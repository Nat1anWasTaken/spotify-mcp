//! "Continue with Spotify": the dashboard sign-in, which also links the
//! user's Spotify account (Authorization Code with PKCE).

use axum::{
    extract::{Query, State},
    response::Response,
};
use axum_extra::extract::CookieJar;
use serde::Deserialize;

use crate::{error::AppResult, state::AppState};

#[derive(Deserialize)]
pub struct StartQuery {
    /// Same-origin path to return to; `/` by default.
    pub next: Option<String>,
}

/// `GET /auth/spotify/start?next=`: redirects to Spotify's consent screen.
pub async fn start(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<StartQuery>,
) -> AppResult<Response> {
    let _ = (state, jar, query);
    todo!("auth/links agent")
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

/// `GET /auth/spotify/callback`: finishes the sign-in and redirects to `next`.
pub async fn callback(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<CallbackQuery>,
) -> AppResult<Response> {
    let _ = (state, jar, query);
    todo!("auth/links agent")
}
