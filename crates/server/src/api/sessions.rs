use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use uuid::Uuid;

use crate::{
    auth::CurrentUser,
    error::{AppError, AppResult},
    mcp,
    sessions::{self, AgentSession, SessionPatch},
    state::AppState,
};

pub async fn list(
    State(state): State<AppState>,
    user: CurrentUser,
) -> AppResult<Json<Vec<AgentSession>>> {
    Ok(Json(sessions::list_active(&state.db, user.id).await?))
}

pub async fn show(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> AppResult<Json<AgentSession>> {
    let session = sessions::find(&state.db, user.id, id).await?;
    session.map(Json).ok_or(AppError::NotFound)
}

pub async fn update(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
    Json(mut patch): Json<SessionPatch>,
) -> AppResult<Json<AgentSession>> {
    patch.name = patch
        .name
        .as_deref()
        .map(sessions::normalize_name)
        .transpose()?;
    if let Some(Some(tools)) = &patch.allowed_tools
        && let Some(unknown) = tools
            .iter()
            .find(|name| !mcp::all_tools().any(|tool| tool.name == *name))
    {
        return Err(AppError::BadRequest(format!("unknown tool {unknown:?}")));
    }
    if !sessions::update(&state.db, user.id, id, &patch).await? {
        return Err(AppError::NotFound);
    }
    let session = sessions::find(&state.db, user.id, id).await?;
    session.map(Json).ok_or(AppError::NotFound)
}

pub async fn revoke(
    State(state): State<AppState>,
    user: CurrentUser,
    Path(id): Path<Uuid>,
) -> AppResult<StatusCode> {
    if !sessions::revoke(&state.db, user.id, id).await? {
        return Err(AppError::NotFound);
    }
    Ok(StatusCode::NO_CONTENT)
}

#[cfg(test)]
mod tests {
    use axum::http::{Method, StatusCode};
    use serde_json::{Value, json};
    use sqlx::PgPool;

    use crate::testing::{self, dashboard_request, send};

    #[sqlx::test(migrations = "../../migrations")]
    async fn patch_is_partial(db: PgPool) {
        let state = testing::state(db);
        let (user_id, cookie) = testing::sign_in(&state, 7).await;
        let session = testing::agent_session(&state, user_id).await;
        let app = crate::app(state.clone());
        let uri = format!("/api/sessions/{session}");
        let patch = |body: Value| {
            let app = app.clone();
            let request = dashboard_request(Method::PATCH, &uri, &cookie, Some(&body));
            async move { send(&app, request).await }
        };

        let updated = patch(json!({ "allowed_tools": ["play"] })).await.json();
        assert_eq!(updated["allowed_tools"], json!(["play"]));
        assert_eq!(updated["client_name"], "Test Agent");

        let updated = patch(json!({ "read_only": true, "name": "  Renamed " }))
            .await
            .json();
        assert_eq!(updated["allowed_tools"], json!(["play"]));
        assert_eq!(updated["read_only"], true);
        assert_eq!(updated["name"], "Renamed");

        let updated = patch(json!({ "allowed_tools": null })).await.json();
        assert_eq!(updated["allowed_tools"], Value::Null);

        assert_eq!(
            patch(json!({ "name": " " })).await.status,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            patch(json!({ "allowed_tools": ["no_such_tool"] }))
                .await
                .status,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            patch(json!({ "use_outbox": true })).await.status,
            StatusCode::UNPROCESSABLE_ENTITY
        );

        // Other users cannot see or change the session.
        let (_, other) = testing::sign_in(&state, 8).await;
        let request = dashboard_request(
            Method::PATCH,
            &uri,
            &other,
            Some(&json!({ "enabled": false })),
        );
        assert_eq!(send(&app, request).await.status, StatusCode::NOT_FOUND);
        let request = dashboard_request(Method::GET, "/api/sessions", &other, None);
        assert_eq!(send(&app, request).await.json(), json!([]));
    }
}
