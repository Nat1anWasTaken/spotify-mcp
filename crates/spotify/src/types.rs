//! Tool requests. Doc comments become the parameter descriptions agents see.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// No parameters.
#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoParams {}

/// What to play. Give `context_uri` or `uris`, or neither to resume.
#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlayRequest {
    /// Device to play on (from list_devices); the active device by default.
    pub device_id: Option<String>,
    /// An album, artist, playlist or show URI, e.g. `spotify:album:…`.
    pub context_uri: Option<String>,
    /// Track or episode URIs to play, in order.
    pub uris: Option<Vec<String>>,
    /// Where in `context_uri` to start: a 0-based position or an item URI.
    pub offset: Option<PlayOffset>,
    /// Where in the first item to start, in milliseconds.
    pub position_ms: Option<u64>,
}

#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum PlayOffset {
    Position(u32),
    Uri(String),
}
