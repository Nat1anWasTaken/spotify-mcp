//! The `/mcp` endpoint, driven over HTTP against a test database and a mock
//! Spotify ([`testing::MockSpotify`], plus a small player mock here).

use std::sync::{Arc, Mutex, atomic::Ordering};

use axum::{
    Router,
    body::Body,
    http::{Method, Request, StatusCode, header},
};
use serde_json::{Value, json};
use sqlx::PgPool;
use uuid::Uuid;

use super::tools::{self, Kind};
use crate::{
    crypto::{hash_token, random_token},
    links,
    state::AppState,
    testing::{self, MockSpotify, PUBLIC_URL, Response, dashboard_request, send},
};

const HOST: &str = "mcp.example.test";

struct Harness {
    state: AppState,
    app: Router,
    user_id: Uuid,
    cookie: String,
    session_id: Uuid,
    token: String,
}

impl Harness {
    /// A signed-in user with one agent session and a bearer token for it.
    /// Spotify calls go to `spotify_url`; nothing is linked yet.
    async fn new(db: PgPool, spotify_url: &str) -> Self {
        let state = testing::state_with_spotify(db, spotify_url);
        let (user_id, cookie) = testing::sign_in(&state, 7).await;
        let session_id = testing::agent_session(&state, user_id).await;
        let token = random_token();
        sqlx::query(
            "INSERT INTO oauth_access_tokens (token_hash, agent_session_id, family_id, scope,
                                              expires_at)
             VALUES ($1, $2, gen_random_uuid(), 'spotify', now() + interval '1 hour')",
        )
        .bind(hash_token(&token))
        .bind(session_id)
        .execute(&state.db)
        .await
        .unwrap();
        Self {
            app: crate::app(state.clone()),
            state,
            user_id,
            cookie,
            session_id,
            token,
        }
    }

    async fn link(&self, tokens: &spotify::oauth::Tokens) {
        links::store_linked(&self.state, self.user_id, tokens)
            .await
            .unwrap();
    }

    async fn set_permissions(&self, read_only: bool, allowed_tools: Option<&[&str]>) {
        sqlx::query("UPDATE agent_sessions SET read_only = $2, allowed_tools = $3 WHERE id = $1")
            .bind(self.session_id)
            .bind(read_only)
            .bind(
                allowed_tools.map(|tools| tools.iter().map(|t| t.to_string()).collect::<Vec<_>>()),
            )
            .execute(&self.state.db)
            .await
            .unwrap();
    }

    async fn post_mcp(&self, body: Value) -> Response {
        let request = Request::post("/mcp")
            .header(header::HOST, HOST)
            .header(header::AUTHORIZATION, format!("Bearer {}", self.token))
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ACCEPT, "application/json, text/event-stream")
            .body(Body::from(body.to_string()))
            .unwrap();
        send(&self.app, request).await
    }

    /// A JSON-RPC request; returns the whole response message.
    async fn rpc(&self, method: &str, params: Value) -> Value {
        let body = json!({ "jsonrpc": "2.0", "id": 1, "method": method, "params": params });
        let response = self.post_mcp(body).await;
        assert_eq!(response.status, StatusCode::OK, "{method}");
        response.json()
    }

    /// Calls a tool; returns its `CallToolResult`.
    async fn call(&self, tool: &str, arguments: Value) -> Value {
        let response = self
            .rpc(
                "tools/call",
                json!({ "name": tool, "arguments": arguments }),
            )
            .await;
        assert!(response["error"].is_null(), "{response}");
        response["result"].clone()
    }

    async fn listed_tools(&self) -> Vec<String> {
        let listed = self.rpc("tools/list", json!({})).await;
        listed["result"]["tools"]
            .as_array()
            .unwrap()
            .iter()
            .map(|tool| tool["name"].as_str().unwrap().to_owned())
            .collect()
    }

    /// `(tool, status, error)` of the logged calls, newest first.
    async fn logged(&self) -> Vec<(String, String, Value)> {
        let request = dashboard_request(Method::GET, "/api/tool-calls", &self.cookie, None);
        let response = send(&self.app, request).await;
        assert_eq!(response.status, StatusCode::OK);
        response.json()["items"]
            .as_array()
            .unwrap()
            .iter()
            .map(|call| {
                (
                    call["tool"].as_str().unwrap().to_owned(),
                    call["status"].as_str().unwrap().to_owned(),
                    call["error"].clone(),
                )
            })
            .collect()
    }
}

fn text(result: &Value) -> &str {
    result["content"][0]["text"].as_str().unwrap()
}

#[sqlx::test(migrations = "../../migrations")]
async fn initialize_and_list_every_tool(db: PgPool) {
    let harness = Harness::new(db, "http://127.0.0.1:9").await;
    let initialize = harness
        .rpc(
            "initialize",
            json!({
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "test", "version": "1" },
            }),
        )
        .await;
    let info = &initialize["result"];
    assert_eq!(info["serverInfo"]["name"], "spotify");
    assert_eq!(
        info["serverInfo"]["icons"][0]["src"],
        format!("{PUBLIC_URL}/spotify.png")
    );
    let instructions = info["instructions"].as_str().unwrap();
    for name in [
        "search",
        "list_my_playlists",
        "create_playlist",
        "add_playlist_items",
        "list_devices",
        "transfer_playback",
        "save_to_library",
        "Premium",
    ] {
        assert!(instructions.contains(name), "instructions mention {name}");
    }

    let listed = harness.rpc("tools/list", json!({})).await;
    let listed = listed["result"]["tools"].as_array().unwrap();
    assert_eq!(listed.len(), tools::all().count());
    let tool = |name: &str| listed.iter().find(|tool| tool["name"] == name).unwrap();
    assert_eq!(tool("get_me")["annotations"]["readOnlyHint"], true);
    assert_eq!(
        tool("replace_playlist_items")["annotations"]["destructiveHint"],
        true
    );
    assert_eq!(
        tool("add_to_queue")["annotations"]["destructiveHint"],
        false
    );
    assert_eq!(tool("search")["inputSchema"]["type"], "object");

    let unknown = harness
        .rpc("tools/call", json!({ "name": "nope", "arguments": {} }))
        .await;
    assert_eq!(unknown["error"]["code"], -32602);
}

#[sqlx::test(migrations = "../../migrations")]
async fn tools_list_follows_the_session_permissions(db: PgPool) {
    let harness = Harness::new(db, "http://127.0.0.1:9").await;

    harness.set_permissions(true, None).await;
    let listed = harness.listed_tools().await;
    let reads: Vec<_> = tools::all()
        .filter(|tool| tool.kind == Kind::Read)
        .map(|tool| tool.name.to_owned())
        .collect();
    assert_eq!(listed, reads);
    assert!(!listed.iter().any(|name| name == "play"));

    harness
        .set_permissions(false, Some(&["get_me", "play", "no_longer_a_tool"]))
        .await;
    assert_eq!(harness.listed_tools().await, ["get_me", "play"]);

    harness
        .set_permissions(true, Some(&["get_me", "play"]))
        .await;
    assert_eq!(harness.listed_tools().await, ["get_me"]);

    sqlx::query("UPDATE agent_sessions SET enabled = false WHERE id = $1")
        .bind(harness.session_id)
        .execute(&harness.state.db)
        .await
        .unwrap();
    assert!(harness.listed_tools().await.is_empty());
}

#[sqlx::test(migrations = "../../migrations")]
async fn denied_and_invalid_calls_are_logged(db: PgPool) {
    let harness = Harness::new(db, "http://127.0.0.1:9").await;
    harness.set_permissions(true, None).await;

    let denied = harness.call("pause", json!({})).await;
    assert_eq!(denied["isError"], true);
    assert!(text(&denied).contains("read-only"), "{denied}");

    harness.set_permissions(false, Some(&["get_me"])).await;
    let not_allowed = harness
        .call("search", json!({ "query": "x", "types": ["track"] }))
        .await;
    assert_eq!(not_allowed["isError"], true);
    assert!(text(&not_allowed).contains("search"), "{not_allowed}");

    let invalid = harness.call("get_tracks", json!({})).await;
    assert_eq!(invalid["isError"], true);
    assert!(
        text(&invalid).starts_with("Invalid arguments: missing field `ids`"),
        "{invalid}"
    );
    let unknown_field = harness.call("get_me", json!({ "bogus": 1 })).await;
    assert!(text(&unknown_field).starts_with("Invalid arguments"));

    let logged: Vec<_> = harness
        .logged()
        .await
        .into_iter()
        .map(|(tool, status, _)| (tool, status))
        .collect();
    let expected = [
        ("get_me", "error"),
        ("get_tracks", "error"),
        ("search", "denied"),
        ("pause", "denied"),
    ];
    assert_eq!(
        logged,
        expected.map(|(tool, status)| (tool.to_owned(), status.to_owned()))
    );
}

#[sqlx::test(migrations = "../../migrations")]
async fn unlinked_users_are_told_to_sign_in_again(db: PgPool) {
    let harness = Harness::new(db, "http://127.0.0.1:9").await;
    let result = harness.call("get_me", json!({})).await;
    assert_eq!(result["isError"], true);
    assert!(
        text(&result).contains("sign in with Spotify again"),
        "{result}"
    );
    assert!(text(&result).contains(PUBLIC_URL));
    let logged = harness.logged().await;
    assert_eq!(logged[0].1, "error");
}

#[sqlx::test(migrations = "../../migrations")]
async fn get_me_round_trips_to_spotify(db: PgPool) {
    let spotify = MockSpotify::start().await;
    let harness = Harness::new(db, &spotify.url).await;
    harness.link(&spotify.tokens()).await;

    let result = harness.call("get_me", json!({})).await;
    assert_eq!(result["isError"], false, "{result}");
    let me: Value = serde_json::from_str(text(&result)).unwrap();
    assert_eq!(me["id"], "spotify-user");
    assert_eq!(me["uri"], "spotify:user:spotify-user");
    assert_eq!(me["name"], "User spotify-user");
    assert_eq!(me["image"], "https://i.scdn.co/image/avatar");
    assert_eq!(spotify.mock.refreshes.load(Ordering::SeqCst), 0);

    let logged = harness.logged().await;
    assert_eq!(logged[0].0, "get_me");
    assert_eq!(logged[0].1, "ok");
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_rejected_token_is_refreshed_once_and_retried(db: PgPool) {
    let spotify = MockSpotify::start().await;
    let harness = Harness::new(db, &spotify.url).await;
    let tokens = spotify.tokens();
    harness.link(&tokens).await;
    spotify
        .mock
        .rejected
        .lock()
        .unwrap()
        .insert(tokens.access_token.clone());

    let result = harness.call("get_me", json!({})).await;
    assert_eq!(result["isError"], false, "{result}");
    assert_eq!(spotify.mock.refreshes.load(Ordering::SeqCst), 1);
    assert_eq!(
        *spotify.mock.refresh_tokens_used.lock().unwrap(),
        [tokens.refresh_token.clone().unwrap()]
    );

    // The new token is stored: the next call needs no refresh.
    let again = harness.call("get_me", json!({})).await;
    assert_eq!(again["isError"], false);
    assert_eq!(spotify.mock.refreshes.load(Ordering::SeqCst), 1);
    let statuses: Vec<_> = harness.logged().await.into_iter().map(|c| c.1).collect();
    assert_eq!(statuses, ["ok", "ok"]);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_revoked_refresh_token_asks_for_a_new_sign_in(db: PgPool) {
    let spotify = MockSpotify::start().await;
    let harness = Harness::new(db, &spotify.url).await;
    let tokens = spotify.tokens();
    harness.link(&tokens).await;
    spotify
        .mock
        .rejected
        .lock()
        .unwrap()
        .insert(tokens.access_token.clone());
    spotify.mock.invalid_grant.store(true, Ordering::SeqCst);

    let result = harness.call("get_me", json!({})).await;
    assert_eq!(result["isError"], true);
    assert!(
        text(&result).contains("sign in with Spotify again"),
        "{result}"
    );
    // Exactly one refresh attempt, and no retry after it failed.
    assert_eq!(spotify.mock.refresh_tokens_used.lock().unwrap().len(), 1);

    // The link now needs a new sign-in, so later calls stop before Spotify.
    let later = harness.call("get_me", json!({})).await;
    assert!(text(&later).contains("sign in with Spotify again"));
    assert_eq!(spotify.mock.refresh_tokens_used.lock().unwrap().len(), 1);
}

/// What the player mock received: `(method, path and query, body)`.
type Seen = Arc<Mutex<Vec<(String, String, Value)>>>;

/// A Web API that accepts any token and answers player commands: `204` for
/// `device_id=phone`, otherwise `404 NO_ACTIVE_DEVICE`.
async fn player_mock() -> (String, Seen) {
    use axum::{
        extract::{OriginalUri, State},
        response::IntoResponse,
        routing::put,
    };

    async fn command(
        State(seen): State<Seen>,
        method: Method,
        OriginalUri(uri): OriginalUri,
        body: String,
    ) -> axum::response::Response {
        let body = serde_json::from_str(&body).unwrap_or(Value::Null);
        let path = uri.path_and_query().unwrap().to_string();
        let active = path.contains("device_id=phone");
        seen.lock().unwrap().push((method.to_string(), path, body));
        if active {
            StatusCode::NO_CONTENT.into_response()
        } else {
            let error = json!({ "error": {
                "status": 404,
                "message": "Player command failed: No active device found",
                "reason": "NO_ACTIVE_DEVICE",
            }});
            (StatusCode::NOT_FOUND, axum::Json(error)).into_response()
        }
    }

    let seen = Seen::default();
    let app = Router::new()
        .route("/v1/me/player/play", put(command))
        .route("/v1/me/player/pause", put(command))
        .with_state(seen.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await });
    (url, seen)
}

#[sqlx::test(migrations = "../../migrations")]
async fn player_commands_reach_spotify(db: PgPool) {
    let (url, seen) = player_mock().await;
    let harness = Harness::new(db, &url).await;
    // Fresh for an hour, so no refresh reaches the (missing) token endpoint.
    harness
        .link(&spotify::oauth::Tokens {
            access_token: "access".into(),
            refresh_token: Some("refresh".into()),
            expires_in: 3600,
            scope: spotify::oauth::SCOPES.join(" "),
        })
        .await;

    let played = harness
        .call(
            "play",
            json!({ "device_id": "phone", "uris": ["spotify:track:4uLU6hMCjMI75M1A2tKUQC"] }),
        )
        .await;
    assert_eq!(played["isError"], false, "{played}");
    assert_eq!(text(&played), "Done.");

    let paused = harness.call("pause", json!({})).await;
    assert_eq!(paused["isError"], true);
    assert!(
        text(&paused).contains("No active Spotify device"),
        "{paused}"
    );

    assert_eq!(
        *seen.lock().unwrap(),
        [
            (
                "PUT".to_owned(),
                "/v1/me/player/play?device_id=phone".to_owned(),
                json!({ "uris": ["spotify:track:4uLU6hMCjMI75M1A2tKUQC"] })
            ),
            (
                "PUT".to_owned(),
                "/v1/me/player/pause".to_owned(),
                Value::Null
            ),
        ]
    );
    let statuses: Vec<_> = harness.logged().await.into_iter().map(|c| c.1).collect();
    assert_eq!(statuses, ["error", "ok"]);
}

#[sqlx::test(migrations = "../../migrations")]
async fn a_revoked_session_is_refused(db: PgPool) {
    let harness = Harness::new(db, "http://127.0.0.1:9").await;
    assert_eq!(harness.listed_tools().await.len(), tools::all().count());

    let request = dashboard_request(
        Method::DELETE,
        &format!("/api/sessions/{}", harness.session_id),
        &harness.cookie,
        None,
    );
    assert_eq!(
        send(&harness.app, request).await.status,
        StatusCode::NO_CONTENT
    );
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" });
    let response = harness.post_mcp(body).await;
    assert_eq!(response.status, StatusCode::UNAUTHORIZED);
    assert!(
        response.headers[header::WWW_AUTHENTICATE]
            .to_str()
            .unwrap()
            .contains("invalid_token")
    );
}
