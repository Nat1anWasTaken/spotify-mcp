//! The user's Spotify tokens: stored sealed, refreshed on demand under a
//! per-user lock, and the link status the dashboard shows.
//!
//! Contract (implemented by the auth/links work):
//! - [`store_linked`] after a sign-in;
//! - [`account`] for every tool call, refreshing when the access token
//!   expires within [`REFRESH_MARGIN`];
//! - [`refresh`] after Spotify rejected a token early;
//! - [`mark_ok`] / [`mark_invalid`] with the outcome of a call;
//! - [`status`] for `/api/me`.

use std::sync::Arc;

use chrono::{DateTime, TimeDelta, Utc};
use dashmap::DashMap;
use serde::Serialize;
use sqlx::PgPool;
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::state::AppState;

/// Refresh access tokens expiring sooner than this.
pub const REFRESH_MARGIN: TimeDelta = TimeDelta::minutes(5);

/// One lock per user, so concurrent calls refresh a token once.
pub type RenewLocks = Arc<DashMap<Uuid, Arc<Mutex<()>>>>;

/// A usable Spotify account and the hash of the access token it acts with,
/// which [`mark_invalid`] compares so a stale failure cannot condemn a
/// fresher token.
pub struct Linked {
    pub account: spotify::Account,
    pub token_hash: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, sqlx::Type)]
#[serde(rename_all = "snake_case")]
#[sqlx(type_name = "text", rename_all = "snake_case")]
pub enum State {
    Ok,
    Renewing,
    NeedsRelink,
}

/// The link as the dashboard shows it (`LinkStatus`).
#[derive(Debug, Serialize)]
pub struct LinkStatus {
    pub linked: bool,
    pub status: Option<State>,
    pub scopes: Vec<String>,
    /// Scopes the tools need that the user's grant lacks; non-empty means
    /// the user must sign in again.
    pub missing_scopes: Vec<&'static str>,
    pub expires_at: Option<DateTime<Utc>>,
    pub last_ok_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

/// Stores freshly granted tokens for `user_id`, replacing any before.
pub async fn store_linked(
    state: &AppState,
    user_id: Uuid,
    tokens: &spotify::oauth::Tokens,
) -> anyhow::Result<()> {
    let _ = (state, user_id, tokens);
    todo!("auth/links agent")
}

/// The user's account, refreshing the access token first when it is about to
/// expire. `None` when the user has no usable link (never linked, or it needs
/// a re-link).
pub async fn account(state: &AppState, user_id: Uuid) -> anyhow::Result<Option<Linked>> {
    let _ = (state, user_id);
    todo!("auth/links agent")
}

/// Refreshes the access token now, unless another call already replaced the
/// one hashed `stale_hash`. `None` when the refresh token was revoked (the
/// link is then marked `needs_relink`).
pub async fn refresh(
    state: &AppState,
    user_id: Uuid,
    stale_hash: &str,
) -> anyhow::Result<Option<Linked>> {
    let _ = (state, user_id, stale_hash);
    todo!("auth/links agent")
}

/// Records a successful call (throttled to a write a minute).
pub async fn mark_ok(db: &PgPool, user_id: Uuid) {
    let _ = (db, user_id);
    todo!("auth/links agent")
}

/// Marks the link `needs_relink`, if it still holds the token hashed `token_hash`.
pub async fn mark_invalid(db: &PgPool, user_id: Uuid, token_hash: &str) {
    let _ = (db, user_id, token_hash);
    todo!("auth/links agent")
}

pub async fn status(db: &PgPool, user_id: Uuid) -> sqlx::Result<LinkStatus> {
    let _ = (db, user_id);
    todo!("auth/links agent")
}
