//! Spotify as an mcp-backbone service.

use axum::{Router, routing::get};
use mcp_backbone::{Access, AppState, Core, Failure, Info, Linked, NoScope, Service, Tool};
use serde_json::Value;
use uuid::Uuid;

use crate::{api, auth, config::SpotifyConfig, links, tools};

pub static INFO: Info = Info {
    slug: "spotify",
    name: "Spotify",
    version: env!("CARGO_PKG_VERSION"),
    // Served from the dashboard's static files (web/public).
    icon: Some("/spotify.png"),
    // Kept from before the framework, so signed-in browsers stay signed in.
    cookie_prefix: Some("spmcp"),
    instructions: INSTRUCTIONS,
    // A development-mode Spotify app only serves the users on its own
    // allowlist, so there is no other.
    allowlist_env: &[],
    max_request_bytes: Info::DEFAULT_MAX_REQUEST_BYTES,
};

const INSTRUCTIONS: &str = "\
These tools act as the user's own Spotify account. Use them only as the user asks.

Finding things: refer to tracks, albums, artists, playlists, shows and episodes by the \
Spotify URIs (spotify:track:…) or IDs that other tools return; get them with search \
(at most 10 results per type and page) instead of guessing. The user's playlists come from \
list_my_playlists; get_playlist_items only works for playlists the user owns or \
collaborates on.

Playlists: create_playlist returns the new playlist's URI; then add_playlist_items with up to \
100 track or episode URIs per call. Edits return a snapshot_id; pass the one you read \
positions from to reorder_playlist_items or remove_playlist_items.

Library and follows: save_to_library, remove_from_library and check_saved cover saved \
tracks (Liked Songs), albums, episodes, shows and audiobooks, and following artists, users \
and playlists (kind artist, user or playlist), up to 40 IDs per call.

Playback: control needs Spotify Premium and a device with Spotify open. Call list_devices \
first; when no device is active, pass a device_id to play or call transfer_playback. A \
\"No active device\" error means the user has to open Spotify on some device.

Write tools change the user's real account: their library, follows, playlists and what is \
playing. Do what the user asked and no more, and confirm before removing or replacing \
anything.

The user sets this connection's permissions in the dashboard; some tools may be denied or \
the connection may be read-only. If a tool says the Spotify sign-in expired, ask the user to \
sign in with Spotify again at the link it gives, then retry.";

pub struct SpotifyService {
    pub spotify: spotify::Spotify,
}

impl SpotifyService {
    pub fn new(config: &SpotifyConfig) -> Self {
        Self {
            spotify: spotify::Spotify::with_urls(
                config.credentials.clone(),
                &config.accounts_url,
                &config.api_url,
            ),
        }
    }
}

impl Service for SpotifyService {
    type Account = spotify::Account;
    type Error = spotify::Error;
    type Scope = NoScope;
    type Target = ();

    fn info(&self) -> &'static Info {
        &INFO
    }

    fn tools(&self) -> Vec<Tool<Self>> {
        tools::all()
    }

    async fn account(
        &self,
        core: &Core,
        user_id: Uuid,
    ) -> anyhow::Result<Access<spotify::Account>> {
        // Never linked, needing a re-link or lacking scopes all read the
        // same to the agent: the user must sign in with Spotify again.
        Ok(match links::account(core, &self.spotify, user_id).await? {
            Some(linked) => Access::Ready(linked),
            None => Access::NeedsRelink,
        })
    }

    fn can_refresh(&self) -> bool {
        true
    }

    async fn refresh(
        &self,
        core: &Core,
        user_id: Uuid,
        stale_hash: Option<&str>,
    ) -> anyhow::Result<Option<Linked<spotify::Account>>> {
        links::refresh(core, &self.spotify, user_id, stale_hash.unwrap_or_default()).await
    }

    fn classify(&self, error: &spotify::Error) -> Failure {
        match error {
            spotify::Error::Unauthorized | spotify::Error::InvalidGrant => Failure::Unauthorized,
            _ => Failure::Other,
        }
    }

    async fn link_status(&self, core: &Core, user_id: Uuid) -> anyhow::Result<Value> {
        Ok(serde_json::to_value(links::status(core, user_id).await?)?)
    }

    fn relink_message(&self, core: &Core) -> String {
        format!(
            "The user's Spotify sign-in has expired or was revoked, or lacks permissions these \
             tools need. Ask the user to sign in with Spotify again at {}/ and then try again.",
            core.config.public_url
        )
    }

    fn not_linked_message(&self, core: &Core) -> String {
        self.relink_message(core)
    }

    fn routes(&self) -> Router<AppState<Self>> {
        Router::new()
            .route("/auth/spotify/start", get(auth::start))
            .route("/auth/spotify/callback", get(auth::callback))
            .route("/api/now-playing", get(api::now_playing))
    }
}
