//! A client for the Spotify Web API: the user's OAuth tokens ([`oauth`]) and
//! one [`Account`] method per MCP tool ([`ops`]). Knows nothing of HTTP
//! serving, the MCP or the database.

mod error;
pub mod oauth;
pub(crate) mod ops;
mod rest;
pub mod types;

pub use ops::users::Profile;

use std::{fmt, sync::Arc, time::Duration};

pub use error::{Error, Result};

pub const ACCOUNTS_URL: &str = "https://accounts.spotify.com";
pub const API_URL: &str = "https://api.spotify.com";

/// The app's Spotify client. Cheap to clone.
#[derive(Clone)]
pub struct Spotify {
    inner: Arc<Inner>,
}

struct Inner {
    http: reqwest::Client,
    client_id: String,
    client_secret: String,
    /// Without a trailing slash; overridable so tests can point at a mock.
    accounts_url: String,
    api_url: String,
}

/// The app credentials from the Spotify developer dashboard.
#[derive(Clone)]
pub struct Credentials {
    pub client_id: String,
    pub client_secret: String,
}

impl fmt::Debug for Credentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Credentials")
            .field("client_id", &self.client_id)
            .finish_non_exhaustive()
    }
}

impl Spotify {
    pub fn new(credentials: Credentials) -> Self {
        Self::with_urls(credentials, ACCOUNTS_URL, API_URL)
    }

    /// A client talking to other hosts than Spotify's, for tests.
    pub fn with_urls(credentials: Credentials, accounts_url: &str, api_url: &str) -> Self {
        let http = reqwest::Client::builder()
            .user_agent(concat!("spotify-mcp/", env!("CARGO_PKG_VERSION")))
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()
            .expect("building the HTTP client");
        Self {
            inner: Arc::new(Inner {
                http,
                client_id: credentials.client_id,
                client_secret: credentials.client_secret,
                accounts_url: accounts_url.trim_end_matches('/').to_owned(),
                api_url: api_url.trim_end_matches('/').to_owned(),
            }),
        }
    }

    pub fn client_id(&self) -> &str {
        &self.inner.client_id
    }

    /// The user's account, acting with their access token.
    pub fn account(&self, access_token: &str) -> Account {
        Account {
            spotify: self.clone(),
            token: access_token.into(),
        }
    }
}

/// The Spotify account of one user. Cheap to clone.
#[derive(Clone)]
pub struct Account {
    spotify: Spotify,
    token: Arc<str>,
}

impl fmt::Debug for Account {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Account").finish_non_exhaustive()
    }
}
