//! [`MockSpotify`], a stand-in for Spotify's accounts service and Web API,
//! and the app state that talks to it. The framework's `testing` module has
//! the rest.

use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicI64, AtomicUsize, Ordering},
    },
    time::Duration,
};

use axum::{
    Router,
    http::{HeaderMap, StatusCode, header},
};
use mcp_backbone::{App, AppState, testing};
use serde_json::Value;
use sqlx::PgPool;

pub use mcp_backbone::testing::{PUBLIC_URL, Response, dashboard_request, send, sign_in};

use crate::{config::SpotifyConfig, service::SpotifyService};

/// A migrated test database's app state whose Spotify accounts and API hosts
/// are both `spotify_url` (a mock server; the default points at a closed
/// port).
pub async fn state_with_spotify(db: PgPool, spotify_url: &str) -> AppState<SpotifyService> {
    testing::migrate(&db, None).await;
    let config = SpotifyConfig {
        credentials: spotify::Credentials {
            client_id: "spotify-client".to_owned(),
            client_secret: "spotify-secret".to_owned(),
        },
        accounts_url: spotify_url.to_owned(),
        api_url: spotify_url.to_owned(),
    };
    testing::app_state(App::new(SpotifyService::new(&config)), db).await
}

pub async fn state(db: PgPool) -> AppState<SpotifyService> {
    state_with_spotify(db, "http://127.0.0.1:9").await
}

/// Spotify's accounts service (`POST /api/token`) and the Web API endpoints
/// the server itself calls, on an ephemeral local port. Tokens it issues are
/// `access-N` / `refresh-N`; the Web API accepts only issued, unrevoked
/// access tokens.
pub struct MockSpotify {
    pub url: String,
    pub mock: Arc<Mock>,
}

/// The mock's switches and what it saw. Lock poisoning is not a concern in
/// tests, hence the `unwrap`s.
pub struct Mock {
    /// Answer every token request with `invalid_grant`.
    pub invalid_grant: AtomicBool,
    /// Leave `refresh_token` out of refresh responses, as Spotify may.
    pub omit_refresh_token: AtomicBool,
    /// Lifetime of issued access tokens, in seconds.
    pub expires_in: AtomicI64,
    /// Answer `/v1/me` with 403, as for a user missing from a
    /// development-mode app's allowlist.
    pub forbid_me: AtomicBool,
    /// Leave `email`, `country` and `product` out of `/v1/me`, as Spotify
    /// does for development-mode apps.
    pub minimal_profile: AtomicBool,
    /// The Spotify user ID `/v1/me` reports.
    pub user_id: Mutex<String>,
    /// The `/v1/me/player` state; `None` answers `204 No Content`.
    pub playback: Mutex<Option<Value>>,
    /// Access tokens answered with `401` before they expire.
    pub rejected: Mutex<HashSet<String>>,
    pub exchanges: AtomicUsize,
    pub refreshes: AtomicUsize,
    /// The form of the last `authorization_code` exchange.
    pub last_exchange: Mutex<HashMap<String, String>>,
    /// The refresh tokens presented, in order.
    pub refresh_tokens_used: Mutex<Vec<String>>,
    issued: AtomicUsize,
    access_tokens: Mutex<HashSet<String>>,
    refresh_tokens: Mutex<HashSet<String>>,
}

impl MockSpotify {
    pub async fn start() -> Self {
        use axum::routing::{get, post};

        let mock = Arc::new(Mock {
            invalid_grant: AtomicBool::new(false),
            omit_refresh_token: AtomicBool::new(false),
            expires_in: AtomicI64::new(3600),
            forbid_me: AtomicBool::new(false),
            minimal_profile: AtomicBool::new(false),
            user_id: Mutex::new("spotify-user".to_owned()),
            playback: Mutex::new(None),
            rejected: Mutex::default(),
            exchanges: AtomicUsize::new(0),
            refreshes: AtomicUsize::new(0),
            last_exchange: Mutex::default(),
            refresh_tokens_used: Mutex::default(),
            issued: AtomicUsize::new(0),
            access_tokens: Mutex::default(),
            refresh_tokens: Mutex::default(),
        });
        let app = Router::new()
            .route("/api/token", post(mock_token))
            .route("/v1/me", get(mock_me))
            .route("/v1/me/player", get(mock_player))
            .route("/v1/me/player/currently-playing", get(mock_player))
            .with_state(mock.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock Spotify");
        let url = format!("http://{}", listener.local_addr().expect("local addr"));
        tokio::spawn(async move { axum::serve(listener, app).await });
        Self { url, mock }
    }

    /// Test state talking to this mock.
    pub async fn state(&self, db: PgPool) -> AppState<SpotifyService> {
        state_with_spotify(db, &self.url).await
    }

    /// Fresh tokens the mock accepts, as an authorization code grant returns.
    pub fn tokens(&self) -> spotify::oauth::Tokens {
        self.mock.issue(true)
    }
}

impl Mock {
    fn issue(&self, with_refresh_token: bool) -> spotify::oauth::Tokens {
        let n = self.issued.fetch_add(1, Ordering::SeqCst) + 1;
        let access_token = format!("access-{n}");
        self.access_tokens
            .lock()
            .unwrap()
            .insert(access_token.clone());
        let refresh_token = with_refresh_token.then(|| {
            let token = format!("refresh-{n}");
            self.refresh_tokens.lock().unwrap().insert(token.clone());
            token
        });
        spotify::oauth::Tokens {
            access_token,
            refresh_token,
            expires_in: self.expires_in.load(Ordering::SeqCst),
            scope: spotify::oauth::SCOPES.join(" "),
        }
    }

    fn authorized(&self, headers: &HeaderMap) -> bool {
        let Some(token) = headers
            .get(header::AUTHORIZATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.strip_prefix("Bearer "))
        else {
            return false;
        };
        self.access_tokens.lock().unwrap().contains(token)
            && !self.rejected.lock().unwrap().contains(token)
    }
}

fn tokens_json(tokens: &spotify::oauth::Tokens) -> Value {
    let mut body = serde_json::json!({
        "access_token": tokens.access_token,
        "token_type": "Bearer",
        "expires_in": tokens.expires_in,
        "scope": tokens.scope,
    });
    if let Some(refresh_token) = &tokens.refresh_token {
        body["refresh_token"] = refresh_token.as_str().into();
    }
    body
}

async fn mock_token(
    axum::extract::State(mock): axum::extract::State<Arc<Mock>>,
    axum::Form(form): axum::Form<HashMap<String, String>>,
) -> (StatusCode, axum::Json<Value>) {
    let invalid_grant = || {
        (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({
                "error": "invalid_grant",
                "error_description": "Invalid refresh token",
            })),
        )
    };
    let revoked = mock.invalid_grant.load(Ordering::SeqCst);
    match form.get("grant_type").map(String::as_str) {
        Some(_) if revoked && !form.contains_key("refresh_token") => invalid_grant(),
        Some("authorization_code") => {
            mock.exchanges.fetch_add(1, Ordering::SeqCst);
            *mock.last_exchange.lock().unwrap() = form;
            (StatusCode::OK, axum::Json(tokens_json(&mock.issue(true))))
        }
        Some("refresh_token") => {
            let presented = form.get("refresh_token").cloned().unwrap_or_default();
            mock.refresh_tokens_used
                .lock()
                .unwrap()
                .push(presented.clone());
            if revoked || !mock.refresh_tokens.lock().unwrap().contains(&presented) {
                return invalid_grant();
            }
            mock.refreshes.fetch_add(1, Ordering::SeqCst);
            // Widen the window in which concurrent refreshes would overlap.
            tokio::time::sleep(Duration::from_millis(50)).await;
            let omit = mock.omit_refresh_token.load(Ordering::SeqCst);
            (StatusCode::OK, axum::Json(tokens_json(&mock.issue(!omit))))
        }
        _ => (
            StatusCode::BAD_REQUEST,
            axum::Json(serde_json::json!({ "error": "unsupported_grant_type" })),
        ),
    }
}

fn unauthorized() -> axum::response::Response {
    use axum::response::IntoResponse;
    (
        StatusCode::UNAUTHORIZED,
        axum::Json(serde_json::json!({
            "error": { "status": 401, "message": "The access token expired" },
        })),
    )
        .into_response()
}

async fn mock_me(
    axum::extract::State(mock): axum::extract::State<Arc<Mock>>,
    headers: HeaderMap,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    if !mock.authorized(&headers) {
        return unauthorized();
    }
    if mock.forbid_me.load(Ordering::SeqCst) {
        return (
            StatusCode::FORBIDDEN,
            axum::Json(serde_json::json!({
                "error": {
                    "status": 403,
                    "message": "User not registered in the Developer Dashboard",
                },
            })),
        )
            .into_response();
    }
    let id = mock.user_id.lock().unwrap().clone();
    if mock.minimal_profile.load(Ordering::SeqCst) {
        return axum::Json(serde_json::json!({
            "id": id,
            "uri": format!("spotify:user:{id}"),
            "display_name": format!("User {id}"),
            "images": [],
        }))
        .into_response();
    }
    axum::Json(serde_json::json!({
        "id": id,
        "uri": format!("spotify:user:{id}"),
        "display_name": format!("User {id}"),
        "email": format!("{id}@example.test"),
        "images": [{ "url": "https://i.scdn.co/image/avatar", "height": 300, "width": 300 }],
        "product": "premium",
        "country": "TW",
    }))
    .into_response()
}

async fn mock_player(
    axum::extract::State(mock): axum::extract::State<Arc<Mock>>,
    headers: HeaderMap,
) -> axum::response::Response {
    use axum::response::IntoResponse;
    if !mock.authorized(&headers) {
        return unauthorized();
    }
    match mock.playback.lock().unwrap().clone() {
        Some(state) => axum::Json(state).into_response(),
        None => StatusCode::NO_CONTENT.into_response(),
    }
}

/// The `name=value` of a `Set-Cookie` header setting `name`, if any.
pub fn set_cookie(response: &Response, name: &str) -> Option<String> {
    response
        .headers
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .find(|v| v.starts_with(&format!("{name}=")))
        .map(str::to_owned)
}
