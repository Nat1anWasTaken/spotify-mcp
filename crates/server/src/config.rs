//! Spotify settings, read from the environment; the framework reads its own.

use anyhow::Result;
use mcp_backbone::config::{optional, required};

#[derive(Clone, Debug)]
pub struct SpotifyConfig {
    pub credentials: spotify::Credentials,
    /// Spotify hosts, overridable so tests can point at a mock.
    pub accounts_url: String,
    pub api_url: String,
}

impl SpotifyConfig {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            credentials: spotify::Credentials {
                client_id: required("SPOTIFY_CLIENT_ID")?,
                client_secret: required("SPOTIFY_CLIENT_SECRET")?,
            },
            accounts_url: optional("SPOTIFY_ACCOUNTS_URL")
                .unwrap_or_else(|| spotify::ACCOUNTS_URL.into()),
            api_url: optional("SPOTIFY_API_URL").unwrap_or_else(|| spotify::API_URL.into()),
        })
    }
}
