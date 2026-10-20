//! The user's Spotify tokens: stored sealed, refreshed on demand under a
//! per-user lock, and the link status the dashboard shows.
//!
//! - [`store_linked`] after a sign-in;
//! - [`account`] for every tool call, refreshing when the access token
//!   expires within [`REFRESH_MARGIN`];
//! - [`refresh`] after Spotify rejected a token early;
//! - [`mark_ok`] / [`mark_invalid`] with the outcome of a call;
//! - [`status`] for `/api/me`.
//!
//! There is no background renewal: refreshing when a call needs it is
//! enough. Refresh tokens themselves expire [`REFRESH_TOKEN_LIFETIME`] after
//! the user's sign-in, which refreshing does not extend; the dashboard warns
//! ahead of `reauthorize_by`, and after it the refresh fails as
//! `needs_relink`.

use std::sync::Arc;

use anyhow::Context;
use chrono::{DateTime, Months, TimeDelta, Utc};
use dashmap::DashMap;
use serde::Serialize;
use sha2::{Digest, Sha256};
use sqlx::{FromRow, PgPool};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::{config::Config, crypto::sealed, state::AppState};

/// Refresh access tokens expiring sooner than this.
pub const REFRESH_MARGIN: TimeDelta = TimeDelta::minutes(5);

/// Spotify refresh tokens expire this long after the user's authorization;
/// refreshing does not extend it.
pub const REFRESH_TOKEN_LIFETIME: Months = Months::new(6);

/// `last_ok_at` is bumped at most this often.
const OK_THROTTLE_SECS: i32 = 60;

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
    /// The last Spotify sign-in.
    pub authorized_at: Option<DateTime<Utc>>,
    /// When the refresh token expires and the user must sign in again
    /// (`authorized_at` + [`REFRESH_TOKEN_LIFETIME`]).
    pub reauthorize_by: Option<DateTime<Utc>>,
}

/// Hex SHA-256 of a token, to tell tokens apart without unsealing them.
pub fn token_hash(token: &str) -> String {
    Sha256::digest(token.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn seal(config: &Config, user_id: Uuid, secret: &str) -> anyhow::Result<Vec<u8>> {
    sealed::seal(
        &config.encryption_key,
        user_id.as_bytes(),
        secret.as_bytes(),
    )
}

fn open(config: &Config, user_id: Uuid, sealed_value: &[u8]) -> anyhow::Result<String> {
    let plain = sealed::open(&config.encryption_key, user_id.as_bytes(), sealed_value)
        .context("unsealing a Spotify token")?;
    Ok(String::from_utf8(plain)?)
}

fn expiry(tokens: &spotify::oauth::Tokens) -> DateTime<Utc> {
    Utc::now() + TimeDelta::seconds(tokens.expires_in.max(0))
}

/// Stores freshly granted tokens for `user_id`, replacing any before. This
/// is a new authorization, so it restarts the refresh token's lifetime.
pub async fn store_linked(
    state: &AppState,
    user_id: Uuid,
    tokens: &spotify::oauth::Tokens,
) -> anyhow::Result<()> {
    let refresh_token = tokens
        .refresh_token
        .as_deref()
        .context("Spotify granted no refresh token")?;
    sqlx::query(
        "INSERT INTO spotify_links (user_id, access_token_sealed, refresh_token_sealed,
                                    token_hash, scopes, expires_at, authorized_at, status,
                                    last_ok_at)
         VALUES ($1, $2, $3, $4, $5, $6, now(), 'ok', now())
         ON CONFLICT (user_id) DO UPDATE SET
             access_token_sealed = EXCLUDED.access_token_sealed,
             refresh_token_sealed = EXCLUDED.refresh_token_sealed,
             token_hash = EXCLUDED.token_hash,
             scopes = EXCLUDED.scopes,
             expires_at = EXCLUDED.expires_at,
             authorized_at = now(),
             status = 'ok',
             last_ok_at = now(),
             last_error = NULL,
             updated_at = now()",
    )
    .bind(user_id)
    .bind(seal(&state.config, user_id, &tokens.access_token)?)
    .bind(seal(&state.config, user_id, refresh_token)?)
    .bind(token_hash(&tokens.access_token))
    .bind(tokens.scopes())
    .bind(expiry(tokens))
    .execute(&state.db)
    .await?;
    Ok(())
}

#[derive(FromRow)]
struct Row {
    access_token_sealed: Vec<u8>,
    refresh_token_sealed: Vec<u8>,
    token_hash: String,
    scopes: Vec<String>,
    expires_at: DateTime<Utc>,
    status: State,
}

impl Row {
    fn is_fresh(&self) -> bool {
        self.expires_at > Utc::now() + REFRESH_MARGIN
    }

    /// Whether tools may act with this link at all.
    fn is_usable(&self) -> bool {
        self.status != State::NeedsRelink && spotify::oauth::missing_scopes(&self.scopes).is_empty()
    }

    fn linked(&self, state: &AppState, user_id: Uuid) -> anyhow::Result<Linked> {
        let token = open(&state.config, user_id, &self.access_token_sealed)?;
        Ok(Linked {
            account: state.spotify.account(&token),
            token_hash: self.token_hash.clone(),
        })
    }
}

async fn load(db: &PgPool, user_id: Uuid) -> sqlx::Result<Option<Row>> {
    sqlx::query_as(
        "SELECT access_token_sealed, refresh_token_sealed, token_hash, scopes, expires_at, status
         FROM spotify_links WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await
}

/// The user's account, refreshing the access token first when it is about to
/// expire. `None` when the user has no usable link (never linked, it needs
/// a re-link, or it lacks scopes the tools need).
pub async fn account(state: &AppState, user_id: Uuid) -> anyhow::Result<Option<Linked>> {
    match load(&state.db, user_id).await? {
        Some(row) if !row.is_usable() => Ok(None),
        Some(row) if row.is_fresh() => row.linked(state, user_id).map(Some),
        Some(_) => renew(state, user_id, None).await,
        None => Ok(None),
    }
}

/// Refreshes the access token now, unless another call already replaced the
/// one hashed `stale_hash`. `None` when the refresh token was revoked (the
/// link is then marked `needs_relink`).
pub async fn refresh(
    state: &AppState,
    user_id: Uuid,
    stale_hash: &str,
) -> anyhow::Result<Option<Linked>> {
    renew(state, user_id, Some(stale_hash)).await
}

/// Refreshes under the user's lock. With `stale_hash`, refreshes unless the
/// stored token is a different one already; without, unless it is fresh by
/// now. Either way a caller that waited for the lock reuses the token the
/// caller before it obtained.
///
/// Detached, so a caller that goes away (a disconnected client) cannot leave
/// the link `renewing` or drop the tokens Spotify just issued.
async fn renew(
    state: &AppState,
    user_id: Uuid,
    stale_hash: Option<&str>,
) -> anyhow::Result<Option<Linked>> {
    let state = state.clone();
    let stale_hash = stale_hash.map(str::to_owned);
    tokio::spawn(async move { renew_locked(&state, user_id, stale_hash.as_deref()).await })
        .await
        .context("refresh task failed")?
}

async fn renew_locked(
    state: &AppState,
    user_id: Uuid,
    stale_hash: Option<&str>,
) -> anyhow::Result<Option<Linked>> {
    let lock = state.renew_locks.entry(user_id).or_default().clone();
    let _guard = lock.lock().await;

    let Some(row) = load(&state.db, user_id).await? else {
        return Ok(None);
    };
    if !row.is_usable() {
        return Ok(None);
    }
    let renewed_already = match stale_hash {
        Some(stale) => row.token_hash != stale,
        None => row.is_fresh(),
    };
    if renewed_already {
        return row.linked(state, user_id).map(Some);
    }

    let refresh_token = open(&state.config, user_id, &row.refresh_token_sealed)?;
    set_status(&state.db, user_id, &row.token_hash, State::Renewing, None).await?;
    match state.spotify.refresh(&refresh_token).await {
        Ok(tokens) => {
            store_refreshed(state, user_id, &row.token_hash, &tokens).await?;
            tracing::debug!(%user_id, "refreshed the Spotify access token");
            let scopes = match tokens.scopes() {
                scopes if scopes.is_empty() => row.scopes,
                scopes => scopes,
            };
            if !spotify::oauth::missing_scopes(&scopes).is_empty() {
                return Ok(None);
            }
            Ok(Some(Linked {
                account: state.spotify.account(&tokens.access_token),
                token_hash: token_hash(&tokens.access_token),
            }))
        }
        Err(spotify::Error::InvalidGrant) => {
            tracing::info!(%user_id, "Spotify refresh token revoked; needs re-link");
            let message = "Spotify revoked the authorization. Sign in with Spotify again.";
            set_status(
                &state.db,
                user_id,
                &row.token_hash,
                State::NeedsRelink,
                Some(message),
            )
            .await?;
            Ok(None)
        }
        Err(err) => {
            set_status(&state.db, user_id, &row.token_hash, State::Ok, None).await?;
            Err(anyhow::Error::new(err).context("refreshing the Spotify access token"))
        }
    }
}

/// Sets the status of the link while it still holds the token `token_hash`.
/// `last_error` is replaced only when given.
async fn set_status(
    db: &PgPool,
    user_id: Uuid,
    token_hash: &str,
    status: State,
    last_error: Option<&str>,
) -> sqlx::Result<()> {
    sqlx::query(
        "UPDATE spotify_links
         SET status = $3, last_error = COALESCE($4, last_error), updated_at = now()
         WHERE user_id = $1 AND token_hash = $2",
    )
    .bind(user_id)
    .bind(token_hash)
    .bind(status)
    .bind(last_error)
    .execute(db)
    .await?;
    Ok(())
}

/// Replaces the token `old_hash` with refreshed ones. Spotify may leave out
/// a new refresh token (and the scopes), in which case the old ones stay.
async fn store_refreshed(
    state: &AppState,
    user_id: Uuid,
    old_hash: &str,
    tokens: &spotify::oauth::Tokens,
) -> anyhow::Result<()> {
    let refresh_token = tokens
        .refresh_token
        .as_deref()
        .map(|token| seal(&state.config, user_id, token))
        .transpose()?;
    let scopes = Some(tokens.scopes()).filter(|scopes| !scopes.is_empty());
    sqlx::query(
        "UPDATE spotify_links SET
             access_token_sealed = $3,
             refresh_token_sealed = COALESCE($4, refresh_token_sealed),
             token_hash = $5,
             scopes = COALESCE($6, scopes),
             expires_at = $7,
             status = 'ok',
             last_error = NULL,
             updated_at = now()
         WHERE user_id = $1 AND token_hash = $2",
    )
    .bind(user_id)
    .bind(old_hash)
    .bind(seal(&state.config, user_id, &tokens.access_token)?)
    .bind(refresh_token)
    .bind(token_hash(&tokens.access_token))
    .bind(scopes)
    .bind(expiry(tokens))
    .execute(&state.db)
    .await?;
    Ok(())
}

/// Records a successful call (throttled to a write a minute).
pub async fn mark_ok(db: &PgPool, user_id: Uuid) {
    let result = sqlx::query(
        "UPDATE spotify_links SET last_ok_at = now()
         WHERE user_id = $1
           AND (last_ok_at IS NULL OR last_ok_at < now() - make_interval(secs => $2))",
    )
    .bind(user_id)
    .bind(OK_THROTTLE_SECS)
    .execute(db)
    .await;
    if let Err(err) = result {
        tracing::warn!(%user_id, error = ?err, "recording a successful Spotify call");
    }
}

/// Marks the link `needs_relink`, if it still holds the token hashed `token_hash`.
pub async fn mark_invalid(db: &PgPool, user_id: Uuid, token_hash: &str) {
    let result = sqlx::query(
        "UPDATE spotify_links
         SET status = 'needs_relink', last_error = 'Spotify rejected the access token.',
             updated_at = now()
         WHERE user_id = $1 AND token_hash = $2",
    )
    .bind(user_id)
    .bind(token_hash)
    .execute(db)
    .await;
    match result {
        Ok(done) if done.rows_affected() > 0 => {
            tracing::info!(%user_id, "Spotify token rejected; needs re-link");
        }
        Ok(_) => {}
        Err(err) => tracing::warn!(%user_id, error = ?err, "marking a Spotify link invalid"),
    }
}

pub async fn status(db: &PgPool, user_id: Uuid) -> sqlx::Result<LinkStatus> {
    #[derive(FromRow)]
    struct StatusRow {
        status: State,
        scopes: Vec<String>,
        expires_at: DateTime<Utc>,
        last_ok_at: Option<DateTime<Utc>>,
        last_error: Option<String>,
        authorized_at: DateTime<Utc>,
    }
    let row: Option<StatusRow> = sqlx::query_as(
        "SELECT status, scopes, expires_at, last_ok_at, last_error, authorized_at
         FROM spotify_links WHERE user_id = $1",
    )
    .bind(user_id)
    .fetch_optional(db)
    .await?;
    Ok(match row {
        Some(row) => LinkStatus {
            linked: true,
            status: Some(row.status),
            missing_scopes: spotify::oauth::missing_scopes(&row.scopes),
            scopes: row.scopes,
            expires_at: Some(row.expires_at),
            last_ok_at: row.last_ok_at,
            last_error: row.last_error,
            authorized_at: Some(row.authorized_at),
            reauthorize_by: row.authorized_at.checked_add_months(REFRESH_TOKEN_LIFETIME),
        },
        None => LinkStatus {
            linked: false,
            status: None,
            scopes: Vec::new(),
            missing_scopes: Vec::new(),
            expires_at: None,
            last_ok_at: None,
            last_error: None,
            authorized_at: None,
            reauthorize_by: None,
        },
    })
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::Ordering;

    use sqlx::PgPool;

    use super::*;
    use crate::testing::{self, MockSpotify};

    /// A signed-in user linked with tokens the mock accepts.
    async fn linked_user(spotify: &MockSpotify, state: &AppState) -> Uuid {
        let (user_id, _) = testing::sign_in(state, 1).await;
        store_linked(state, user_id, &spotify.tokens())
            .await
            .unwrap();
        user_id
    }

    async fn expire_soon(db: &PgPool, user_id: Uuid) {
        sqlx::query(
            "UPDATE spotify_links SET expires_at = now() + interval '1 minute' WHERE user_id = $1",
        )
        .bind(user_id)
        .execute(db)
        .await
        .unwrap();
    }

    #[test]
    fn token_hash_is_hex_sha256() {
        assert_eq!(
            token_hash("abc"),
            "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad"
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn tokens_are_sealed_and_fresh_ones_used_as_is(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let state = spotify.state(db.clone());
        let user_id = linked_user(&spotify, &state).await;

        let (access, refresh): (Vec<u8>, Vec<u8>) =
            sqlx::query_as("SELECT access_token_sealed, refresh_token_sealed FROM spotify_links")
                .fetch_one(&db)
                .await
                .unwrap();
        assert!(!access.windows(8).any(|w| w == b"access-1"));
        assert_eq!(open(&state.config, user_id, &refresh).unwrap(), "refresh-1");
        // Bound to the user: another user's ID cannot open it.
        assert!(open(&state.config, Uuid::new_v4(), &access).is_err());

        let linked = account(&state, user_id).await.unwrap().unwrap();
        assert_eq!(linked.token_hash, token_hash("access-1"));
        assert_eq!(spotify.mock.refreshes.load(Ordering::SeqCst), 0);
        assert!(account(&state, Uuid::new_v4()).await.unwrap().is_none());
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn refreshes_near_expiry_and_keeps_an_omitted_refresh_token(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let state = spotify.state(db.clone());
        let user_id = linked_user(&spotify, &state).await;
        spotify
            .mock
            .omit_refresh_token
            .store(true, Ordering::SeqCst);

        expire_soon(&db, user_id).await;
        let linked = account(&state, user_id).await.unwrap().unwrap();
        assert_eq!(linked.token_hash, token_hash("access-2"));
        let status = status(&db, user_id).await.unwrap();
        assert_eq!(status.status, Some(State::Ok));
        assert!(status.expires_at.unwrap() > Utc::now() + TimeDelta::minutes(55));
        // The profile works with the new token.
        assert!(linked.account.profile().await.is_ok());

        sqlx::query("UPDATE spotify_links SET authorized_at = now() - interval '30 days'")
            .execute(&db)
            .await
            .unwrap();
        let authorized_at = super::status(&db, user_id).await.unwrap().authorized_at;
        expire_soon(&db, user_id).await;
        let linked = account(&state, user_id).await.unwrap().unwrap();
        assert_eq!(linked.token_hash, token_hash("access-3"));
        assert_eq!(
            super::status(&db, user_id).await.unwrap().authorized_at,
            authorized_at,
            "refreshing does not extend the authorization"
        );
        // Signing in again does.
        store_linked(&state, user_id, &spotify.tokens())
            .await
            .unwrap();
        let relinked = super::status(&db, user_id)
            .await
            .unwrap()
            .authorized_at
            .unwrap();
        assert!(relinked > Utc::now() - TimeDelta::minutes(1));
        assert_eq!(
            *spotify.mock.refresh_tokens_used.lock().unwrap(),
            ["refresh-1", "refresh-1"],
            "the original refresh token is kept"
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn concurrent_calls_refresh_once(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let state = spotify.state(db.clone());
        let user_id = linked_user(&spotify, &state).await;
        expire_soon(&db, user_id).await;

        let calls = (0..8).map(|_| {
            let state = state.clone();
            tokio::spawn(async move { account(&state, user_id).await.unwrap().unwrap().token_hash })
        });
        let hashes = futures::future::join_all(calls).await;
        assert_eq!(spotify.mock.refreshes.load(Ordering::SeqCst), 1);
        for hash in hashes {
            assert_eq!(hash.unwrap(), token_hash("access-2"));
        }

        // A forced refresh of a token already replaced reuses the new one.
        let stale = token_hash("access-1");
        let linked = refresh(&state, user_id, &stale).await.unwrap().unwrap();
        assert_eq!(linked.token_hash, token_hash("access-2"));
        assert_eq!(spotify.mock.refreshes.load(Ordering::SeqCst), 1);
        // Of the current one, refreshes.
        let current = token_hash("access-2");
        let linked = refresh(&state, user_id, &current).await.unwrap().unwrap();
        assert_eq!(linked.token_hash, token_hash("access-3"));
        assert_eq!(spotify.mock.refreshes.load(Ordering::SeqCst), 2);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn a_revoked_refresh_token_needs_a_relink(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let state = spotify.state(db.clone());
        let user_id = linked_user(&spotify, &state).await;
        expire_soon(&db, user_id).await;
        spotify.mock.invalid_grant.store(true, Ordering::SeqCst);

        assert!(account(&state, user_id).await.unwrap().is_none());
        let status = status(&db, user_id).await.unwrap();
        assert_eq!(status.status, Some(State::NeedsRelink));
        assert!(
            status
                .last_error
                .unwrap()
                .contains("Sign in with Spotify again")
        );
        // Not retried while it needs a re-link.
        spotify.mock.invalid_grant.store(false, Ordering::SeqCst);
        assert!(account(&state, user_id).await.unwrap().is_none());
        assert_eq!(spotify.mock.refresh_tokens_used.lock().unwrap().len(), 1);

        // Signing in again restores it.
        store_linked(&state, user_id, &spotify.tokens())
            .await
            .unwrap();
        assert!(account(&state, user_id).await.unwrap().is_some());
        assert_eq!(super::status(&db, user_id).await.unwrap().last_error, None);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn other_refresh_failures_are_errors_and_leave_the_link_ok(db: PgPool) {
        // Nothing listens on the default mock URL.
        let spotify = MockSpotify::start().await;
        let state = testing::state(db.clone());
        let (user_id, _) = testing::sign_in(&state, 1).await;
        store_linked(&state, user_id, &spotify.tokens())
            .await
            .unwrap();
        expire_soon(&db, user_id).await;

        assert!(account(&state, user_id).await.is_err());
        assert_eq!(status(&db, user_id).await.unwrap().status, Some(State::Ok));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn missing_scopes_make_the_link_unusable(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let state = spotify.state(db.clone());
        let (user_id, _) = testing::sign_in(&state, 1).await;
        let mut tokens = spotify.tokens();
        tokens.scope = "user-read-private".into();
        store_linked(&state, user_id, &tokens).await.unwrap();

        assert!(account(&state, user_id).await.unwrap().is_none());
        let status = status(&db, user_id).await.unwrap();
        assert!(status.linked);
        assert!(
            status
                .missing_scopes
                .contains(&"user-modify-playback-state")
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn mark_invalid_ignores_stale_tokens_and_mark_ok_is_throttled(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let state = spotify.state(db.clone());
        let user_id = linked_user(&spotify, &state).await;

        mark_invalid(&db, user_id, &token_hash("an-older-token")).await;
        assert_eq!(status(&db, user_id).await.unwrap().status, Some(State::Ok));

        sqlx::query("UPDATE spotify_links SET last_ok_at = now() - interval '30 seconds'")
            .execute(&db)
            .await
            .unwrap();
        let before = status(&db, user_id).await.unwrap().last_ok_at;
        mark_ok(&db, user_id).await;
        assert_eq!(status(&db, user_id).await.unwrap().last_ok_at, before);
        sqlx::query("UPDATE spotify_links SET last_ok_at = now() - interval '2 minutes'")
            .execute(&db)
            .await
            .unwrap();
        mark_ok(&db, user_id).await;
        let after = status(&db, user_id).await.unwrap().last_ok_at.unwrap();
        assert!(after > Utc::now() - TimeDelta::seconds(5));

        mark_invalid(&db, user_id, &token_hash("access-1")).await;
        let status = status(&db, user_id).await.unwrap();
        assert_eq!(status.status, Some(State::NeedsRelink));
        assert!(status.last_error.is_some());
        assert!(account(&state, user_id).await.unwrap().is_none());
    }
}
