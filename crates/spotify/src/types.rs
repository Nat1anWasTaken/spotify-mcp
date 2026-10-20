//! Tool requests. Doc comments become the parameter descriptions agents see.
//!
//! Wherever an ID is asked for, a `spotify:` URI or an `open.spotify.com`
//! link to the item works too.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

/// No parameters.
#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NoParams {}

// ---- search ----

/// A kind of catalog item to search for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SearchType {
    Track,
    Album,
    Artist,
    Playlist,
    Show,
    Episode,
    Audiobook,
}

impl SearchType {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Track => "track",
            Self::Album => "album",
            Self::Artist => "artist",
            Self::Playlist => "playlist",
            Self::Show => "show",
            Self::Episode => "episode",
            Self::Audiobook => "audiobook",
        }
    }
}

/// Search the Spotify catalog.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchRequest {
    /// Search keywords. Field filters narrow results, e.g.
    /// `artist:Radiohead album:"OK Computer"`, `track:Creep year:1990-1999`,
    /// `genre:jazz`, `tag:new` (albums from the last two weeks).
    pub query: String,
    /// The kinds of items to return, e.g. `["track"]` or `["album", "artist"]`.
    pub types: Vec<SearchType>,
    /// ISO 3166-1 alpha-2 country code; defaults to the user's country.
    pub market: Option<String>,
    /// Results per type, 1–10 (default 5).
    pub limit: Option<u32>,
    /// Index of the first result, 0–1000, for paging.
    pub offset: Option<u32>,
}

// ---- catalog ----

/// Several catalog items of one kind, by ID.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct IdsRequest {
    /// 1–10 IDs (or URIs/links) of items of the tool's kind.
    pub ids: Vec<String>,
    /// ISO 3166-1 alpha-2 country code; defaults to the user's country.
    pub market: Option<String>,
}

/// A page of the children of one item (album tracks, show episodes,
/// audiobook chapters).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PagedIdRequest {
    /// ID (or URI/link) of the album, show or audiobook.
    pub id: String,
    /// ISO 3166-1 alpha-2 country code; defaults to the user's country.
    pub market: Option<String>,
    /// Items per page, 1–50 (default 20).
    pub limit: Option<u32>,
    /// Index of the first item, for paging (use `next_offset`).
    pub offset: Option<u32>,
}

/// A kind of release in an artist's discography.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AlbumGroup {
    Album,
    Single,
    AppearsOn,
    Compilation,
}

impl AlbumGroup {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Album => "album",
            Self::Single => "single",
            Self::AppearsOn => "appears_on",
            Self::Compilation => "compilation",
        }
    }
}

/// A page of an artist's albums.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ArtistAlbumsRequest {
    /// Artist ID (or URI/link).
    pub id: String,
    /// Only these kinds of releases; all kinds by default.
    pub include_groups: Option<Vec<AlbumGroup>>,
    /// ISO 3166-1 alpha-2 country code; defaults to the user's country.
    pub market: Option<String>,
    /// Albums per page, 1–10 (default 5).
    pub limit: Option<u32>,
    /// Index of the first album, for paging (use `next_offset`).
    pub offset: Option<u32>,
}

/// A page of a list.
#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PageRequest {
    /// Items per page, 1–50 (default 20).
    pub limit: Option<u32>,
    /// Index of the first item, for paging (use `next_offset`).
    pub offset: Option<u32>,
}

// ---- library ----

/// A kind of item saved in the user's library.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum SavedKind {
    Track,
    Album,
    Episode,
    Show,
    Audiobook,
}

/// A page of the user's saved items of one kind.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SavedRequest {
    /// Which saved items: `track` (Liked Songs), `album`, `episode`, `show`
    /// or `audiobook`. Followed artists: get_followed_artists; playlists:
    /// list_my_playlists.
    pub kind: SavedKind,
    /// ISO 3166-1 alpha-2 country code; defaults to the user's country.
    pub market: Option<String>,
    /// Items per page, 1–50 (default 20).
    pub limit: Option<u32>,
    /// Index of the first item, for paging (use `next_offset`).
    pub offset: Option<u32>,
}

/// A kind of item that can be saved to (or followed in) the library.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum LibraryKind {
    Track,
    Album,
    Episode,
    Show,
    Audiobook,
    Artist,
    User,
    Playlist,
}

impl LibraryKind {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Track => "track",
            Self::Album => "album",
            Self::Episode => "episode",
            Self::Show => "show",
            Self::Audiobook => "audiobook",
            Self::Artist => "artist",
            Self::User => "user",
            Self::Playlist => "playlist",
        }
    }
}

/// Items of one kind to check, save or remove in the user's library.
/// Saving an artist, user or playlist follows it; removing unfollows.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct LibraryRequest {
    /// The kind of the items: `track` (Liked Songs), `album`, `episode`,
    /// `show`, `audiobook`, or `artist` / `user` / `playlist` to follow.
    pub kind: LibraryKind,
    /// 1–40 IDs (or URIs/links) of items of that kind.
    pub ids: Vec<String>,
}

// ---- follow ----

/// A page of the artists the user follows.
#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct FollowedArtistsRequest {
    /// Cursor from the previous page's `next_after`.
    pub after: Option<String>,
    /// Artists per page, 1–50 (default 20).
    pub limit: Option<u32>,
}

// ---- personalization ----

/// What to rank.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum TopKind {
    Artists,
    Tracks,
}

/// Over which period to rank.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimeRange {
    /// About the last 4 weeks.
    ShortTerm,
    /// About the last 6 months (the default).
    MediumTerm,
    /// About the last year.
    LongTerm,
}

impl TimeRange {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ShortTerm => "short_term",
            Self::MediumTerm => "medium_term",
            Self::LongTerm => "long_term",
        }
    }
}

/// The user's most listened artists or tracks.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TopItemsRequest {
    /// `artists` or `tracks`.
    pub kind: TopKind,
    /// `short_term` (~4 weeks), `medium_term` (~6 months, default) or
    /// `long_term` (~1 year).
    pub time_range: Option<TimeRange>,
    /// Items per page, 1–50 (default 20).
    pub limit: Option<u32>,
    /// Index of the first item, for paging (use `next_offset`).
    pub offset: Option<u32>,
}

/// Tracks the user played recently (at most the last 50).
#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecentlyPlayedRequest {
    /// Tracks to return, 1–50 (default 20).
    pub limit: Option<u32>,
    /// Only plays after this Unix time in milliseconds. Not with `before`.
    pub after: Option<i64>,
    /// Only plays before this Unix time in milliseconds (use the previous
    /// page's `next_before`). Not with `after`.
    pub before: Option<i64>,
}

// ---- playlists ----

/// One playlist.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaylistRequest {
    /// Playlist ID (or URI/link).
    pub playlist_id: String,
}

/// One playlist's details.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GetPlaylistRequest {
    /// Playlist ID (or URI/link).
    pub playlist_id: String,
    /// ISO 3166-1 alpha-2 country code; defaults to the user's country.
    pub market: Option<String>,
}

/// A page of a playlist's tracks and episodes. Only works for playlists
/// the user owns or collaborates on.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlaylistItemsRequest {
    /// Playlist ID (or URI/link).
    pub playlist_id: String,
    /// ISO 3166-1 alpha-2 country code; defaults to the user's country.
    pub market: Option<String>,
    /// Items per page, 1–50 (default 20).
    pub limit: Option<u32>,
    /// Index of the first item, for paging (use `next_offset`).
    pub offset: Option<u32>,
}

/// A new, empty playlist owned by the user.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CreatePlaylistRequest {
    /// Playlist name.
    pub name: String,
    /// Playlist description.
    pub description: Option<String>,
    /// Show it on the user's profile (default true).
    pub public: Option<bool>,
    /// Let others edit it (default false); requires `public: false`.
    pub collaborative: Option<bool>,
}

/// Changes to a playlist the user owns. Omitted fields stay as they are.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UpdatePlaylistRequest {
    /// Playlist ID (or URI/link).
    pub playlist_id: String,
    /// New name.
    pub name: Option<String>,
    /// New description.
    pub description: Option<String>,
    /// Show it on the user's profile.
    pub public: Option<bool>,
    /// Let others edit it; only for non-public playlists.
    pub collaborative: Option<bool>,
}

/// Tracks or episodes to add to a playlist.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AddItemsRequest {
    /// Playlist ID (or URI/link).
    pub playlist_id: String,
    /// 1–100 track or episode URIs (or links; bare IDs are taken as tracks),
    /// added in this order.
    pub uris: Vec<String>,
    /// 0-based position to insert at; appended by default.
    pub position: Option<u32>,
}

/// Move a run of items within a playlist.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReorderItemsRequest {
    /// Playlist ID (or URI/link).
    pub playlist_id: String,
    /// 0-based position of the first item to move.
    pub range_start: u32,
    /// 0-based position to move the items before, counted before the move.
    /// To move to the end, use the playlist's item count.
    pub insert_before: u32,
    /// How many consecutive items to move (default 1).
    pub range_length: Option<u32>,
    /// The playlist version these positions refer to (`snapshot_id`).
    pub snapshot_id: Option<String>,
}

/// Tracks or episodes to remove from a playlist (every occurrence).
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RemoveItemsRequest {
    /// Playlist ID (or URI/link).
    pub playlist_id: String,
    /// 1–100 track or episode URIs (or links; bare IDs are taken as tracks).
    pub uris: Vec<String>,
    /// The playlist version to remove from (`snapshot_id`).
    pub snapshot_id: Option<String>,
}

/// Replace all of a playlist's items.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ReplaceItemsRequest {
    /// Playlist ID (or URI/link).
    pub playlist_id: String,
    /// 0–100 track or episode URIs (or links; bare IDs are taken as
    /// tracks), the playlist's new contents in order. Empty clears it.
    pub uris: Vec<String>,
}

/// A new cover image for a playlist the user owns.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct UploadCoverRequest {
    /// Playlist ID (or URI/link).
    pub playlist_id: String,
    /// The JPEG image, base64-encoded; at most 256 KB encoded.
    pub jpeg_base64: String,
}

// ---- player ----

/// Options for reading playback.
#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct MarketRequest {
    /// ISO 3166-1 alpha-2 country code; defaults to the user's country.
    pub market: Option<String>,
}

/// Move playback to another device.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TransferRequest {
    /// Target device ID (from list_devices).
    pub device_id: String,
    /// Start playing on it (default: keep the current play/pause state).
    pub play: Option<bool>,
}

/// What to play. Give `context_uri` or `uris`, or neither to resume.
#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct PlayRequest {
    /// Device to play on (from list_devices); the active device by default.
    pub device_id: Option<String>,
    /// An album, artist, playlist, show or audiobook URI (or link), e.g.
    /// `spotify:album:…`.
    pub context_uri: Option<String>,
    /// Track or episode URIs (or links) to play, in order.
    pub uris: Option<Vec<String>>,
    /// Where in `context_uri` to start: a 0-based position or an item URI.
    pub offset: Option<PlayOffset>,
    /// Where in the first item to start, in milliseconds.
    pub position_ms: Option<u64>,
}

/// A 0-based position in the context, or the URI of an item in it.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(untagged)]
pub enum PlayOffset {
    Position(u32),
    Uri(String),
}

/// The device a command applies to.
#[derive(Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DeviceRequest {
    /// Device ID (from list_devices); the active device by default.
    pub device_id: Option<String>,
}

/// Jump within the current item.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SeekRequest {
    /// Position in milliseconds; past the end skips to the next item.
    pub position_ms: u64,
    /// Device ID (from list_devices); the active device by default.
    pub device_id: Option<String>,
}

/// Repeat mode.
#[derive(Debug, Clone, Copy, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum RepeatState {
    /// Repeat the current track.
    Track,
    /// Repeat the current album, playlist, etc.
    Context,
    /// No repeat.
    Off,
}

impl RepeatState {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Track => "track",
            Self::Context => "context",
            Self::Off => "off",
        }
    }
}

/// Set the repeat mode.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RepeatRequest {
    /// `track`, `context` or `off`.
    pub state: RepeatState,
    /// Device ID (from list_devices); the active device by default.
    pub device_id: Option<String>,
}

/// Turn shuffle on or off.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ShuffleRequest {
    /// `true` to shuffle.
    pub state: bool,
    /// Device ID (from list_devices); the active device by default.
    pub device_id: Option<String>,
}

/// Set the volume.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct VolumeRequest {
    /// Volume, 0–100.
    pub volume_percent: u8,
    /// Device ID (from list_devices); the active device by default.
    pub device_id: Option<String>,
}

/// Add an item to the end of the queue.
#[derive(Debug, Deserialize, Serialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct QueueRequest {
    /// A track or episode URI (or link; a bare ID is taken as a track).
    pub uri: String,
    /// Device ID (from list_devices); the active device by default.
    pub device_id: Option<String>,
}
