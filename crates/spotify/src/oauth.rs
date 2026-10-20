//! The user's Spotify authorization: Authorization Code with PKCE, as a
//! confidential client (the secret is sent too), and refreshing.

use serde::Deserialize;

use crate::{Result, Spotify};

/// Every scope the tools need. A grant missing any of them is re-requested.
pub const SCOPES: &[&str] = &[
    "user-read-private",
    "user-read-email",
    "playlist-read-private",
    "playlist-read-collaborative",
    "playlist-modify-public",
    "playlist-modify-private",
    "ugc-image-upload",
    "user-library-read",
    "user-library-modify",
    "user-follow-read",
    "user-follow-modify",
    "user-top-read",
    "user-read-recently-played",
    "user-read-playback-position",
    "user-read-playback-state",
    "user-modify-playback-state",
    "user-read-currently-playing",
];

/// Tokens from the token endpoint. Spotify may omit `refresh_token` on
/// refresh, in which case the previous one stays valid.
#[derive(Clone, Deserialize)]
pub struct Tokens {
    pub access_token: String,
    pub refresh_token: Option<String>,
    /// Seconds.
    pub expires_in: i64,
    /// Space-separated granted scopes.
    #[serde(default)]
    pub scope: String,
}

impl std::fmt::Debug for Tokens {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Tokens")
            .field("expires_in", &self.expires_in)
            .field("scope", &self.scope)
            .finish_non_exhaustive()
    }
}

impl Tokens {
    pub fn scopes(&self) -> Vec<String> {
        self.scope.split_whitespace().map(str::to_owned).collect()
    }
}

/// The scopes of [`SCOPES`] missing from `granted`.
pub fn missing_scopes(granted: &[String]) -> Vec<&'static str> {
    SCOPES
        .iter()
        .copied()
        .filter(|scope| !granted.iter().any(|g| g == scope))
        .collect()
}

/// A new PKCE verifier (43 base64url characters).
pub fn new_verifier() -> String {
    todo!("crates/spotify agent")
}

/// The S256 challenge of `verifier`.
pub fn challenge(verifier: &str) -> String {
    let _ = verifier;
    todo!("crates/spotify agent")
}

impl Spotify {
    /// Where to send the user's browser to authorize the app. `show_dialog`
    /// forces Spotify's consent screen, so the user can switch accounts.
    pub fn authorize_url(
        &self,
        redirect_uri: &str,
        state: &str,
        code_challenge: &str,
        show_dialog: bool,
    ) -> String {
        let _ = (redirect_uri, state, code_challenge, show_dialog);
        todo!("crates/spotify agent")
    }

    /// Exchanges an authorization code. Fails with [`crate::Error::InvalidGrant`]
    /// for a bad or used code.
    pub async fn exchange_code(
        &self,
        code: &str,
        redirect_uri: &str,
        verifier: &str,
    ) -> Result<Tokens> {
        let _ = (code, redirect_uri, verifier);
        todo!("crates/spotify agent")
    }

    /// Refreshes an access token. Fails with [`crate::Error::InvalidGrant`]
    /// when the refresh token is revoked.
    pub async fn refresh(&self, refresh_token: &str) -> Result<Tokens> {
        let _ = refresh_token;
        todo!("crates/spotify agent")
    }
}
