use std::time::Duration;

pub type Result<T, E = Error> = std::result::Result<T, E>;

/// A failed Spotify request. `Display` texts are written for the agent that
/// made the call: they say what went wrong and what to do about it.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// The access token is invalid or expired (HTTP 401). The caller refreshes
    /// it and retries once.
    #[error("Spotify rejected the access token.")]
    Unauthorized,
    /// The refresh token was revoked or is invalid (`invalid_grant`): the user
    /// must sign in with Spotify again.
    #[error("The user's Spotify authorization was revoked.")]
    InvalidGrant,
    #[error("Spotify is rate limiting requests; try again in {} seconds.", retry_after.as_secs().max(1))]
    RateLimited { retry_after: Duration },
    #[error(
        "Spotify Premium is required for this. The user's account is not Premium, so playback \
         cannot be controlled."
    )]
    PremiumRequired,
    #[error(
        "No active Spotify device. Call list_devices and pass a device_id, or transfer_playback \
         to a device first; the user may need to open Spotify on one."
    )]
    NoActiveDevice,
    #[error("Not found: {0}")]
    NotFound(String),
    #[error("Spotify refused the request: {0}")]
    Forbidden(String),
    #[error("Invalid request: {0}")]
    InvalidArgument(String),
    #[error("Spotify error {status}: {message}")]
    Api { status: u16, message: String },
    #[error("Could not reach Spotify: {0}")]
    Http(#[from] reqwest::Error),
    #[error("Unexpected response from Spotify: {0}")]
    Decode(String),
}
