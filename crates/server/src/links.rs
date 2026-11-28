//! The user's Spotify tokens, in the framework's link store: the access and
//! refresh tokens are its secrets, the granted scopes and the sign-in time
//! its metadata. Access tokens are refreshed on demand under a per-user lock.
//!
//! - [`store_linked`] after a sign-in;
//! - [`account`] for every call, refreshing when the access token expires
//!   within [`REFRESH_MARGIN`];
//! - [`refresh`] after Spotify rejected a token early;
//! - [`status`] for `/api/me`.
//!
//! The framework records each call's outcome on the link. Refresh tokens
//! expire [`REFRESH_TOKEN_LIFETIME`] after the user's sign-in, which
//! refreshing does not extend; the dashboard warns ahead of
//! `reauthorize_by`, and after it the refresh fails as `needs_relink`.

use anyhow::Context;
use chrono::{DateTime, Months, TimeDelta, Utc};
use mcp_backbone::{
    Core, Linked,
    links::{LinkRecord, LinkState, LinkStore, NewLink, token_hash},
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use uuid::Uuid;

/// Refresh access tokens expiring sooner than this.
pub const REFRESH_MARGIN: TimeDelta = TimeDelta::minutes(5);
/// Spotify refresh tokens expire this long after the user's authorization;
/// refreshing does not extend it.
pub const REFRESH_TOKEN_LIFETIME: Months = Months::new(6);

const ACCESS_TOKEN: &str = "access_token";
const REFRESH_TOKEN: &str = "refresh_token";

/// What the link keeps besides its secrets.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Metadata {
    #[serde(default)]
    scopes: Vec<String>,
    /// The last Spotify sign-in.
    authorized_at: Option<DateTime<Utc>>,
}

fn metadata(record: &LinkRecord) -> Metadata {
    serde_json::from_value(record.metadata.0.clone()).unwrap_or_default()
}

/// The link as the dashboard shows it (`LinkStatus`).
#[derive(Debug, Serialize)]
pub struct LinkStatus {
    pub linked: bool,
    pub status: Option<LinkState>,
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

fn expiry(tokens: &spotify::oauth::Tokens) -> DateTime<Utc> {
    Utc::now() + TimeDelta::seconds(tokens.expires_in.max(0))
}

/// Stores freshly granted tokens for `user_id`, replacing any before. This
/// is a new authorization, so it restarts the refresh token's lifetime.
pub async fn store_linked(
    core: &Core,
    user_id: Uuid,
    tokens: &spotify::oauth::Tokens,
) -> anyhow::Result<()> {
    let refresh_token = tokens
        .refresh_token
        .as_deref()
        .context("Spotify granted no refresh token")?;
    LinkStore::new(core)
        .put(
            user_id,
            &NewLink {
                secrets: &[
                    (ACCESS_TOKEN, &tokens.access_token),
                    (REFRESH_TOKEN, refresh_token),
                ],
                token_hash: token_hash(&tokens.access_token),
                expires_at: Some(expiry(tokens)),
                metadata: json!({ "scopes": tokens.scopes(), "authorized_at": Utc::now() }),
            },
        )
        .await
}

fn is_fresh(record: &LinkRecord) -> bool {
    record
        .expires_at
        .is_some_and(|expires_at| expires_at > Utc::now() + REFRESH_MARGIN)
}

/// Whether tools may act with this link at all.
fn is_usable(record: &LinkRecord) -> bool {
    record.is_usable() && spotify::oauth::missing_scopes(&metadata(record).scopes).is_empty()
}

fn linked(
    core: &Core,
    spotify: &spotify::Spotify,
    record: &LinkRecord,
) -> anyhow::Result<Linked<spotify::Account>> {
    let token = record.secret(core, ACCESS_TOKEN)?;
    Ok(Linked::new(
        spotify.account(&token),
        record.token_hash.clone(),
    ))
}

/// The user's account, refreshing the access token first when it is about to
/// expire. `None` when the user has no usable link (never linked, it needs
/// a re-link, or it lacks scopes the tools need).
pub async fn account(
    core: &Core,
    spotify: &spotify::Spotify,
    user_id: Uuid,
) -> anyhow::Result<Option<Linked<spotify::Account>>> {
    match LinkStore::new(core).load(user_id).await? {
        Some(record) if !is_usable(&record) => Ok(None),
        Some(record) if is_fresh(&record) => linked(core, spotify, &record).map(Some),
        Some(_) => renew(core, spotify, user_id, None).await,
        None => Ok(None),
    }
}

/// Refreshes the access token now, unless another call already replaced the
/// one hashed `stale_hash`. `None` when the refresh token was revoked (the
/// link is then marked `needs_relink`).
pub async fn refresh(
    core: &Core,
    spotify: &spotify::Spotify,
    user_id: Uuid,
    stale_hash: &str,
) -> anyhow::Result<Option<Linked<spotify::Account>>> {
    renew(core, spotify, user_id, Some(stale_hash)).await
}

/// Refreshes under the user's lock. With `stale_hash`, refreshes unless the
/// stored token is a different one already; without, unless it is fresh by
/// now. Either way a caller that waited for the lock reuses the token the
/// caller before it obtained.
///
/// Detached, so a caller that goes away (a disconnected client) cannot leave
/// the link `renewing` or drop the tokens Spotify just issued.
async fn renew(
    core: &Core,
    spotify: &spotify::Spotify,
    user_id: Uuid,
    stale_hash: Option<&str>,
) -> anyhow::Result<Option<Linked<spotify::Account>>> {
    let core = core.clone();
    let spotify = spotify.clone();
    let stale_hash = stale_hash.map(str::to_owned);
    tokio::spawn(async move { renew_locked(&core, &spotify, user_id, stale_hash.as_deref()).await })
        .await
        .context("refresh task failed")?
}

async fn renew_locked(
    core: &Core,
    spotify: &spotify::Spotify,
    user_id: Uuid,
    stale_hash: Option<&str>,
) -> anyhow::Result<Option<Linked<spotify::Account>>> {
    let lock = core.user_lock(user_id);
    let _guard = lock.lock().await;
    let store = LinkStore::new(core);
    let Some(record) = store.load(user_id).await? else {
        return Ok(None);
    };
    if !is_usable(&record) {
        return Ok(None);
    }
    let renewed_already = match stale_hash {
        Some(stale) => record.token_hash != stale,
        None => is_fresh(&record),
    };
    if renewed_already {
        return linked(core, spotify, &record).map(Some);
    }
    let refresh_token = record.secret(core, REFRESH_TOKEN)?;
    let old_hash = &record.token_hash;
    store
        .set_status(user_id, old_hash, LinkState::Renewing, None)
        .await?;
    match spotify.refresh(&refresh_token).await {
        Ok(tokens) => {
            // Spotify may leave out a new refresh token and the scopes, in
            // which case the old ones stay.
            let scopes = match tokens.scopes() {
                scopes if scopes.is_empty() => metadata(&record).scopes,
                scopes => scopes,
            };
            let mut secrets = vec![(ACCESS_TOKEN, tokens.access_token.as_str())];
            if let Some(refresh_token) = tokens.refresh_token.as_deref() {
                secrets.push((REFRESH_TOKEN, refresh_token));
            }
            let renewed = NewLink {
                secrets: &secrets,
                token_hash: token_hash(&tokens.access_token),
                expires_at: Some(expiry(&tokens)),
                metadata: json!({ "scopes": scopes }),
            };
            store.replace(user_id, old_hash, &renewed).await?;
            tracing::debug!(%user_id, "refreshed the Spotify access token");
            if !spotify::oauth::missing_scopes(&scopes).is_empty() {
                return Ok(None);
            }
            Ok(Some(Linked::new(
                spotify.account(&tokens.access_token),
                renewed.token_hash,
            )))
        }
        Err(spotify::Error::InvalidGrant) => {
            tracing::info!(%user_id, "Spotify refresh token revoked; needs re-link");
            let message = "Spotify revoked the authorization. Sign in with Spotify again.";
            store
                .set_status(user_id, old_hash, LinkState::NeedsRelink, Some(message))
                .await?;
            Ok(None)
        }
        Err(err) => {
            store
                .set_status(user_id, old_hash, LinkState::Ok, None)
                .await?;
            Err(anyhow::Error::new(err).context("refreshing the Spotify access token"))
        }
    }
}

pub async fn status(core: &Core, user_id: Uuid) -> anyhow::Result<LinkStatus> {
    let record = LinkStore::new(core).load(user_id).await?;
    Ok(match record {
        Some(record) => {
            let metadata = metadata(&record);
            LinkStatus {
                linked: true,
                status: Some(record.status),
                missing_scopes: spotify::oauth::missing_scopes(&metadata.scopes),
                scopes: metadata.scopes,
                expires_at: record.expires_at,
                last_ok_at: record.last_ok_at,
                last_error: record.last_error,
                authorized_at: metadata.authorized_at,
                reauthorize_by: metadata
                    .authorized_at
                    .and_then(|at| at.checked_add_months(REFRESH_TOKEN_LIFETIME)),
            }
        }
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

    type State_ = mcp_backbone::AppState<crate::service::SpotifyService>;

    fn client(state: &State_) -> &spotify::Spotify {
        &state.service.spotify
    }

    async fn account(
        state: &State_,
        user_id: Uuid,
    ) -> anyhow::Result<Option<Linked<spotify::Account>>> {
        super::account(state, client(state), user_id).await
    }

    async fn refresh(
        state: &State_,
        user_id: Uuid,
        stale: &str,
    ) -> anyhow::Result<Option<Linked<spotify::Account>>> {
        super::refresh(state, client(state), user_id, stale).await
    }

    /// A signed-in user linked with tokens the mock accepts.
    async fn linked_user(spotify: &MockSpotify, state: &State_) -> Uuid {
        let (user_id, _) = testing::sign_in(state, 1).await;
        store_linked(state, user_id, &spotify.tokens())
            .await
            .unwrap();
        user_id
    }

    async fn expire_soon(db: &PgPool, user_id: Uuid) {
        sqlx::query("UPDATE links SET expires_at = now() + interval '1 minute' WHERE user_id = $1")
            .bind(user_id)
            .execute(db)
            .await
            .unwrap();
    }

    #[sqlx::test(migrations = false)]
    async fn tokens_are_sealed_and_fresh_ones_used_as_is(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let state = spotify.state(db.clone()).await;
        let user_id = linked_user(&spotify, &state).await;

        let sealed: Vec<Vec<u8>> = sqlx::query_scalar("SELECT sealed FROM link_secrets")
            .fetch_all(&db)
            .await
            .unwrap();
        assert_eq!(sealed.len(), 2);
        assert!(
            sealed
                .iter()
                .all(|s| !s.windows(8).any(|w| w == b"access-1"))
        );
        let record = LinkStore::new(&state).load(user_id).await.unwrap().unwrap();
        assert_eq!(record.secret(&state, REFRESH_TOKEN).unwrap(), "refresh-1");

        let linked = account(&state, user_id).await.unwrap().unwrap();
        assert_eq!(linked.token_hash, Some(token_hash("access-1")));
        assert_eq!(spotify.mock.refreshes.load(Ordering::SeqCst), 0);
        assert!(account(&state, Uuid::new_v4()).await.unwrap().is_none());
    }

    #[sqlx::test(migrations = false)]
    async fn refreshes_near_expiry_and_keeps_an_omitted_refresh_token(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let state = spotify.state(db.clone()).await;
        let user_id = linked_user(&spotify, &state).await;
        spotify
            .mock
            .omit_refresh_token
            .store(true, Ordering::SeqCst);

        expire_soon(&db, user_id).await;
        let linked = account(&state, user_id).await.unwrap().unwrap();
        assert_eq!(linked.token_hash, Some(token_hash("access-2")));
        let status = status(&state, user_id).await.unwrap();
        assert_eq!(status.status, Some(LinkState::Ok));
        assert!(status.expires_at.unwrap() > Utc::now() + TimeDelta::minutes(55));
        // The profile works with the new token.
        assert!(linked.account.profile().await.is_ok());

        sqlx::query(
            "UPDATE links SET metadata = jsonb_set(metadata, '{authorized_at}',
                                                 to_jsonb(now() - interval '30 days'))",
        )
        .execute(&db)
        .await
        .unwrap();
        let authorized_at = super::status(&state, user_id).await.unwrap().authorized_at;
        expire_soon(&db, user_id).await;
        let linked = account(&state, user_id).await.unwrap().unwrap();
        assert_eq!(linked.token_hash, Some(token_hash("access-3")));
        assert_eq!(
            super::status(&state, user_id).await.unwrap().authorized_at,
            authorized_at,
            "refreshing does not extend the authorization"
        );
        // Signing in again does.
        store_linked(&state, user_id, &spotify.tokens())
            .await
            .unwrap();
        let relinked = super::status(&state, user_id)
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

    #[sqlx::test(migrations = false)]
    async fn concurrent_calls_refresh_once(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let state = spotify.state(db.clone()).await;
        let user_id = linked_user(&spotify, &state).await;
        expire_soon(&db, user_id).await;

        let calls = (0..8).map(|_| {
            let state = state.clone();
            tokio::spawn(async move { account(&state, user_id).await.unwrap().unwrap().token_hash })
        });
        let hashes = futures::future::join_all(calls).await;
        assert_eq!(spotify.mock.refreshes.load(Ordering::SeqCst), 1);
        for hash in hashes {
            assert_eq!(hash.unwrap(), Some(token_hash("access-2")));
        }

        // A forced refresh of a token already replaced reuses the new one.
        let stale = token_hash("access-1");
        let linked = refresh(&state, user_id, &stale).await.unwrap().unwrap();
        assert_eq!(linked.token_hash, Some(token_hash("access-2")));
        assert_eq!(spotify.mock.refreshes.load(Ordering::SeqCst), 1);
        // Of the current one, refreshes.
        let current = token_hash("access-2");
        let linked = refresh(&state, user_id, &current).await.unwrap().unwrap();
        assert_eq!(linked.token_hash, Some(token_hash("access-3")));
        assert_eq!(spotify.mock.refreshes.load(Ordering::SeqCst), 2);
    }

    #[sqlx::test(migrations = false)]
    async fn a_revoked_refresh_token_needs_a_relink(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let state = spotify.state(db.clone()).await;
        let user_id = linked_user(&spotify, &state).await;
        expire_soon(&db, user_id).await;
        spotify.mock.invalid_grant.store(true, Ordering::SeqCst);

        assert!(account(&state, user_id).await.unwrap().is_none());
        let status = status(&state, user_id).await.unwrap();
        assert_eq!(status.status, Some(LinkState::NeedsRelink));
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
        assert_eq!(
            super::status(&state, user_id).await.unwrap().last_error,
            None
        );
    }

    #[sqlx::test(migrations = false)]
    async fn other_refresh_failures_are_errors_and_leave_the_link_ok(db: PgPool) {
        // Nothing listens on the default mock URL.
        let spotify = MockSpotify::start().await;
        let state = testing::state(db.clone()).await;
        let (user_id, _) = testing::sign_in(&state, 1).await;
        store_linked(&state, user_id, &spotify.tokens())
            .await
            .unwrap();
        expire_soon(&db, user_id).await;

        assert!(account(&state, user_id).await.is_err());
        assert_eq!(
            status(&state, user_id).await.unwrap().status,
            Some(LinkState::Ok)
        );
    }

    #[sqlx::test(migrations = false)]
    async fn missing_scopes_make_the_link_unusable(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let state = spotify.state(db.clone()).await;
        let (user_id, _) = testing::sign_in(&state, 1).await;
        let mut tokens = spotify.tokens();
        tokens.scope = "user-read-private".into();
        store_linked(&state, user_id, &tokens).await.unwrap();

        assert!(account(&state, user_id).await.unwrap().is_none());
        let status = status(&state, user_id).await.unwrap();
        assert!(status.linked);
        assert!(
            status
                .missing_scopes
                .contains(&"user-modify-playback-state")
        );
    }
}
