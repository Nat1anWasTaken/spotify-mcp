//! Spotify sign-in for the dashboard and the cookie sessions it creates.

mod session;
mod spotify;

use axum::{
    Router,
    routing::{get, post},
};
use axum_extra::extract::cookie::{Cookie, SameSite};

use crate::{config::Config, state::AppState};

pub use session::CurrentUser;
#[cfg(test)]
pub use session::start as start_session;

pub fn router() -> Router<AppState> {
    Router::new()
        .route("/auth/spotify/start", get(spotify::start))
        .route("/auth/spotify/callback", get(spotify::callback))
        .route("/auth/logout", post(session::logout))
}

/// An HttpOnly, `SameSite=Lax` cookie, `Secure` when served over https.
fn cookie(
    config: &Config,
    name: &'static str,
    value: String,
    path: &'static str,
    max_age: time::Duration,
) -> Cookie<'static> {
    Cookie::build((name, value))
        .path(path)
        .max_age(max_age)
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(config.secure_cookies())
        .build()
}
