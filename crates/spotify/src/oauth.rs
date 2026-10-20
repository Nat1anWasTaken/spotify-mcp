//! The user's Spotify authorization: Authorization Code with PKCE, as a
//! confidential client (the secret is sent too), and refreshing.

use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use rand::RngCore;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use url::form_urlencoded;

use crate::{Error, Result, Spotify};

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
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    URL_SAFE_NO_PAD.encode(bytes)
}

/// The S256 challenge of `verifier`.
pub fn challenge(verifier: &str) -> String {
    URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))
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
        let mut query = form_urlencoded::Serializer::new(String::new());
        query
            .append_pair("client_id", &self.inner.client_id)
            .append_pair("response_type", "code")
            .append_pair("redirect_uri", redirect_uri)
            .append_pair("state", state)
            .append_pair("scope", &SCOPES.join(" "))
            .append_pair("code_challenge_method", "S256")
            .append_pair("code_challenge", code_challenge);
        if show_dialog {
            query.append_pair("show_dialog", "true");
        }
        format!("{}/authorize?{}", self.inner.accounts_url, query.finish())
    }

    /// Exchanges an authorization code. Fails with [`crate::Error::InvalidGrant`]
    /// for a bad or used code.
    pub async fn exchange_code(
        &self,
        code: &str,
        redirect_uri: &str,
        verifier: &str,
    ) -> Result<Tokens> {
        self.token_request(&[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("redirect_uri", redirect_uri),
            ("code_verifier", verifier),
        ])
        .await
    }

    /// Refreshes an access token. Fails with [`crate::Error::InvalidGrant`]
    /// when the refresh token is revoked.
    pub async fn refresh(&self, refresh_token: &str) -> Result<Tokens> {
        self.token_request(&[
            ("grant_type", "refresh_token"),
            ("refresh_token", refresh_token),
        ])
        .await
    }

    /// `POST {accounts}/api/token` with HTTP Basic client authentication.
    async fn token_request(&self, form: &[(&str, &str)]) -> Result<Tokens> {
        let response = self
            .inner
            .http
            .post(format!("{}/api/token", self.inner.accounts_url))
            .basic_auth(&self.inner.client_id, Some(&self.inner.client_secret))
            .form(form)
            .send()
            .await
            .map_err(|e| Error::Http(e.without_url()))?;
        let status = response.status();
        let body = response
            .text()
            .await
            .map_err(|e| Error::Http(e.without_url()))?;
        if status.is_success() {
            return serde_json::from_str(&body)
                .map_err(|e| Error::Decode(format!("token response: {e}")));
        }
        Err(token_error(status.as_u16(), &body))
    }
}

/// Maps a failed token response (`{error, error_description}`, RFC 6749).
fn token_error(status: u16, body: &str) -> Error {
    #[derive(Deserialize)]
    struct OAuthError {
        error: String,
        error_description: Option<String>,
    }
    match serde_json::from_str::<OAuthError>(body) {
        Ok(e) if e.error == "invalid_grant" => Error::InvalidGrant,
        Ok(e) => Error::Api {
            status,
            message: match e.error_description {
                Some(description) => format!("{}: {description}", e.error),
                None => e.error,
            },
        },
        Err(_) if status == 429 => Error::RateLimited {
            retry_after: std::time::Duration::from_secs(1),
        },
        Err(_) => Error::Api {
            status,
            message: "token endpoint failed".into(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Credentials;

    #[test]
    fn verifier_is_43_base64url_chars() {
        let v = new_verifier();
        assert_eq!(v.len(), 43);
        assert!(
            v.chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
        );
        assert_ne!(v, new_verifier());
    }

    #[test]
    fn challenge_matches_rfc7636_example() {
        assert_eq!(
            challenge("dBjftJeZ4CVP-mB92K27uhbUJU1p1r_wW1gFWFOEjXk"),
            "E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM"
        );
    }

    #[test]
    fn authorize_url_is_encoded() {
        let spotify = Spotify::with_urls(
            Credentials {
                client_id: "cid".into(),
                client_secret: "secret".into(),
            },
            "https://accounts.example/",
            "https://api.example",
        );
        let url = spotify.authorize_url("http://127.0.0.1:5173/cb?x=1&y", "s t", "ch", true);
        let parsed = url::Url::parse(&url).unwrap();
        assert_eq!(parsed.path(), "/authorize");
        assert!(url.starts_with("https://accounts.example/authorize?"));
        let pairs: std::collections::HashMap<_, _> = parsed.query_pairs().into_owned().collect();
        assert_eq!(pairs["redirect_uri"], "http://127.0.0.1:5173/cb?x=1&y");
        assert_eq!(pairs["state"], "s t");
        assert_eq!(pairs["scope"], SCOPES.join(" "));
        assert_eq!(pairs["code_challenge_method"], "S256");
        assert_eq!(pairs["show_dialog"], "true");
        assert!(!url.contains("secret"));
        let quiet = spotify.authorize_url("http://x/cb", "s", "ch", false);
        assert!(!quiet.contains("show_dialog"));
    }

    #[test]
    fn token_errors() {
        assert!(matches!(
            token_error(
                400,
                r#"{"error":"invalid_grant","error_description":"Invalid refresh token"}"#
            ),
            Error::InvalidGrant
        ));
        assert!(matches!(
            token_error(400, r#"{"error":"invalid_client"}"#),
            Error::Api { status: 400, .. }
        ));
        assert!(matches!(
            token_error(502, "<html>"),
            Error::Api { status: 502, .. }
        ));
    }
}
