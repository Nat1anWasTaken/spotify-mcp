//! The client against a local mock of the accounts service and Web API:
//! token requests, empty responses, rate-limit retries and error mapping.

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use axum::{
    Form, Json, Router,
    extract::State,
    http::{HeaderMap, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post, put},
};
use serde_json::{Value, json};
use spotify::{Credentials, Error, Spotify, types::NoParams};

async fn serve(app: Router) -> Spotify {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    Spotify::with_urls(
        Credentials {
            client_id: "cid".into(),
            client_secret: "csecret".into(),
        },
        &base,
        &base,
    )
}

async fn token(headers: HeaderMap, Form(form): Form<Vec<(String, String)>>) -> Response {
    // Basic base64("cid:csecret").
    assert_eq!(headers[header::AUTHORIZATION], "Basic Y2lkOmNzZWNyZXQ=");
    let get = |k: &str| {
        form.iter()
            .find(|(key, _)| key == k)
            .map(|(_, v)| v.as_str())
    };
    match (get("grant_type"), get("code"), get("refresh_token")) {
        (Some("authorization_code"), Some("good"), _) => {
            assert_eq!(get("code_verifier"), Some("verifier"));
            assert_eq!(get("redirect_uri"), Some("http://127.0.0.1/cb"));
            Json(
                json!({"access_token": "at", "token_type": "Bearer", "expires_in": 3600,
                        "refresh_token": "rt", "scope": "user-read-private user-read-email"}),
            )
            .into_response()
        }
        (Some("refresh_token"), _, Some("rt")) => {
            Json(json!({"access_token": "at2", "expires_in": 3600, "scope": ""})).into_response()
        }
        _ => (
            StatusCode::BAD_REQUEST,
            Json(json!({"error": "invalid_grant", "error_description": "Invalid refresh token"})),
        )
            .into_response(),
    }
}

#[tokio::test]
async fn token_endpoint() {
    let spotify = serve(Router::new().route("/api/token", post(token))).await;
    let tokens = spotify
        .exchange_code("good", "http://127.0.0.1/cb", "verifier")
        .await
        .unwrap();
    assert_eq!(tokens.access_token, "at");
    assert_eq!(tokens.refresh_token.as_deref(), Some("rt"));
    assert_eq!(tokens.scopes(), ["user-read-private", "user-read-email"]);
    let refreshed = spotify.refresh("rt").await.unwrap();
    assert_eq!(refreshed.access_token, "at2");
    assert!(refreshed.refresh_token.is_none());
    assert!(matches!(
        spotify.refresh("revoked").await,
        Err(Error::InvalidGrant)
    ));
}

#[tokio::test]
async fn web_api() {
    let hits = Arc::new(AtomicUsize::new(0));
    let app = Router::new()
        .route(
            "/v1/me",
            get(|headers: HeaderMap| async move {
                if headers[header::AUTHORIZATION] != "Bearer good" {
                    return (
                        StatusCode::UNAUTHORIZED,
                        Json(json!({"error": {"status": 401, "message": "Invalid access token"}})),
                    )
                        .into_response();
                }
                Json(
                    json!({"id": "u1", "display_name": "U", "uri": "spotify:user:u1",
                            "images": [{"url": "avatar"}]}),
                )
                .into_response()
            }),
        )
        .route("/v1/me/player", get(|| async { StatusCode::NO_CONTENT }))
        .route(
            "/v1/me/player/devices",
            get(|State(hits): State<Arc<AtomicUsize>>| async move {
                if hits.fetch_add(1, Ordering::SeqCst) == 0 {
                    return (StatusCode::TOO_MANY_REQUESTS, [(header::RETRY_AFTER, "0")])
                        .into_response();
                }
                Json(
                    json!({"devices": [{"id": "d", "name": "Mac", "type": "Computer",
                    "is_active": true, "volume_percent": 50}]}),
                )
                .into_response()
            }),
        )
        .route(
            "/v1/me/player/pause",
            put(|headers: HeaderMap| async move {
                assert_eq!(headers[header::CONTENT_LENGTH], "0");
                (
                    StatusCode::NOT_FOUND,
                    Json(json!({"error": {"status": 404,
                        "message": "Player command failed: No active device found",
                        "reason": "NO_ACTIVE_DEVICE"}})),
                )
            }),
        )
        .with_state(hits.clone());
    let spotify = serve(app).await;
    let account = spotify.account("good");

    let profile = account.profile().await.unwrap();
    assert_eq!(profile.id, "u1");
    assert_eq!(profile.avatar_url.as_deref(), Some("avatar"));
    assert!(profile.email.is_none());

    assert_eq!(account.now_playing().await.unwrap(), Value::Null);

    let devices = account.list_devices(NoParams {}).await.unwrap();
    assert_eq!(devices[0]["name"], "Mac");
    assert_eq!(hits.load(Ordering::SeqCst), 2, "retried once after 429");

    let paused = account.pause(Default::default()).await;
    assert!(matches!(paused, Err(Error::NoActiveDevice)));

    let bad = spotify.account("bad").get_me(NoParams {}).await;
    assert!(matches!(bad, Err(Error::Unauthorized)));
}
