//! The path every tool call takes: argument parsing, the session's
//! permissions, the user's Spotify account, execution and the call log.

use std::time::Instant;

use rmcp::{
    ErrorData,
    model::{CallToolResult, JsonObject},
};
use serde_json::Value;

use super::{
    log::{self, NewCall, Status},
    output::failure,
    policy,
    tools::{self, Tool},
};
use crate::{
    links,
    oauth::AgentPrincipal,
    sessions::{self, Permissions},
    state::AppState,
};

pub const REVOKED: &str = "This connection was disconnected in the dashboard.";

/// Handles `tools/call`. Only an unknown tool, a revoked session or an
/// internal failure is a protocol error; everything else is a tool result the
/// agent can read.
pub async fn call(
    state: &AppState,
    principal: AgentPrincipal,
    name: &str,
    args: JsonObject,
) -> Result<CallToolResult, ErrorData> {
    let tool = tools::find(name)
        .ok_or_else(|| ErrorData::invalid_params(format!("unknown tool: {name}"), None))?;
    let session = load_session(state, principal).await?;
    run(state, principal, &session, tool, Value::Object(args))
        .await
        .map_err(internal_error)
}

/// The session's permissions, read fresh on every request so dashboard
/// changes apply immediately.
pub async fn load_session(
    state: &AppState,
    principal: AgentPrincipal,
) -> Result<Permissions, ErrorData> {
    sessions::permissions(&state.db, principal.user_id, principal.session_id)
        .await
        .map_err(internal_error)?
        .ok_or_else(|| ErrorData::invalid_request(REVOKED, None))
}

fn internal_error(err: sqlx::Error) -> ErrorData {
    tracing::error!(error = ?err, "serving an MCP request");
    ErrorData::internal_error("internal error", None)
}

/// What the agent is told when Spotify no longer accepts the user's tokens.
pub fn relink_message(state: &AppState) -> String {
    format!(
        "The user's Spotify sign-in has expired or was revoked, or lacks permissions these \
         tools need. Ask the user to sign in with Spotify again at {}/ and then try again.",
        state.config.public_url
    )
}

async fn run(
    state: &AppState,
    principal: AgentPrincipal,
    session: &Permissions,
    tool: &'static Tool,
    args: Value,
) -> sqlx::Result<CallToolResult> {
    let started = Instant::now();
    let request = match tool.parse(&args) {
        Ok(request) => request,
        Err(err) => {
            let entry = NewCall::unparsed(principal, tool.name, &args);
            let message = format!("Invalid arguments: {err}");
            return refuse(state, &entry, Status::Error, message, started).await;
        }
    };
    let entry = NewCall::new(principal, tool.name, &request.log_arguments());
    if let Err(denial) = policy::check_tool(session, tool) {
        return refuse(state, &entry, Status::Denied, denial.to_string(), started).await;
    }
    let linked = match links::account(state, principal.user_id).await {
        Ok(Some(linked)) => linked,
        Ok(None) => {
            let message = relink_message(state);
            return refuse(state, &entry, Status::Error, message, started).await;
        }
        Err(err) => {
            tracing::error!(error = ?err, "loading the Spotify account");
            let message = "internal error".to_owned();
            return refuse(state, &entry, Status::Error, message, started).await;
        }
    };
    // Detached: rmcp drops this request's future when the client goes away,
    // but an action that started must run to its end and be logged.
    let state = state.clone();
    let task = tokio::spawn(async move {
        let id = log::start(&state.db, &entry).await?;
        let user_id = principal.user_id;
        let mut outcome = request.run(linked.account).await;
        let mut token_hash = linked.token_hash;
        // Spotify revoked the access token before it expired: refresh it once
        // and run the call again with the new one.
        if matches!(outcome, Err(spotify::Error::Unauthorized)) {
            match links::refresh(&state, user_id, &token_hash).await {
                Ok(Some(fresh)) => {
                    token_hash = fresh.token_hash;
                    let retry = tool.parse(&args).expect("arguments parsed before");
                    outcome = retry.run(fresh.account).await;
                }
                Ok(None) => {}
                Err(err) => tracing::error!(error = ?err, "refreshing the Spotify token"),
            }
        }
        let (outcome, status, result, error) = match outcome {
            Ok(output) => {
                links::mark_ok(&state.db, user_id).await;
                let (outcome, result) = output.into_parts();
                (outcome, Status::Ok, result, None)
            }
            Err(spotify::Error::Unauthorized | spotify::Error::InvalidGrant) => {
                links::mark_invalid(&state.db, user_id, &token_hash).await;
                let message = relink_message(&state);
                (failure(message.clone()), Status::Error, None, Some(message))
            }
            Err(err) => {
                let message = err.to_string();
                (failure(message.clone()), Status::Error, None, Some(message))
            }
        };
        let logged = log::finish(
            &state.db,
            id,
            status,
            result,
            error.as_deref(),
            started.elapsed(),
        )
        .await;
        if let Err(err) = logged {
            // The agent still gets the outcome.
            tracing::error!(%id, error = ?err, "logging the outcome of a tool call");
        }
        Ok(outcome)
    });
    task.await.unwrap_or_else(|err| {
        tracing::error!(tool = tool.name, error = ?err, "tool call task failed");
        Ok(failure("internal error"))
    })
}

/// Logs a call that ends without running.
async fn refuse(
    state: &AppState,
    entry: &NewCall,
    status: Status,
    message: String,
    started: Instant,
) -> sqlx::Result<CallToolResult> {
    log::record(&state.db, entry, status, &message, started.elapsed()).await?;
    Ok(failure(message))
}
