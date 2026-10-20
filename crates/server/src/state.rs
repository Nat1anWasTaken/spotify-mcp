use std::sync::Arc;

use sqlx::PgPool;

use crate::{config::Config, links::RenewLocks, oauth::ClientAssertions};

/// Shared application state. Every field is cheap to clone.
#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub db: PgPool,
    pub spotify: spotify::Spotify,
    pub client_assertions: Arc<ClientAssertions>,
    pub renew_locks: RenewLocks,
}

impl AppState {
    pub fn new(config: Arc<Config>, db: PgPool) -> anyhow::Result<Self> {
        Ok(Self {
            spotify: spotify::Spotify::with_urls(
                config.spotify.clone(),
                &config.spotify_accounts_url,
                &config.spotify_api_url,
            ),
            db,
            config,
            client_assertions: Default::default(),
            renew_locks: Default::default(),
        })
    }
}
