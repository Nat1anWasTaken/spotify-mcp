//! The rmcp [`ServerHandler`]: server info, `tools/list` and `tools/call`.

use rmcp::{
    ErrorData, RoleServer, ServerHandler,
    model::{
        CallToolRequestParams, CallToolResponse, Icon, Implementation, ListToolsResult,
        PaginatedRequestParams, ServerCapabilities, ServerConfig, Tool as Definition,
    },
    service::RequestContext,
};

use super::{pipeline, policy, tools};
use crate::{oauth::AgentPrincipal, state::AppState};

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

/// The MCP server for one request. Stateless: every request is authenticated
/// on its own and reads its session's permissions fresh.
pub struct SpotifyServer {
    state: AppState,
}

impl SpotifyServer {
    pub fn new(state: AppState) -> Self {
        Self { state }
    }
}

impl ServerHandler for SpotifyServer {
    fn get_info(&self) -> ServerConfig {
        ServerConfig::new(ServerCapabilities::builder().enable_tools().build())
            .with_server_info(
                Implementation::new("spotify", env!("CARGO_PKG_VERSION"))
                    .with_title("Spotify")
                    .with_icons(vec![
                        // Served from the dashboard's static files (web/public).
                        Icon::new(format!("{}/spotify.png", self.state.config.public_url))
                            .with_mime_type("image/png")
                            .with_sizes(vec!["512x512".into()]),
                    ]),
            )
            .with_instructions(INSTRUCTIONS)
    }

    async fn list_tools(
        &self,
        _: Option<PaginatedRequestParams>,
        context: RequestContext<RoleServer>,
    ) -> Result<ListToolsResult, ErrorData> {
        let session = pipeline::load_session(&self.state, principal(&context)?).await?;
        let listed = tools::all()
            .filter(|tool| policy::check_tool(&session, tool).is_ok())
            .map(tools::Tool::definition)
            .collect();
        Ok(ListToolsResult::with_all_items(listed))
    }

    async fn call_tool(
        &self,
        request: CallToolRequestParams,
        context: RequestContext<RoleServer>,
    ) -> Result<CallToolResponse, ErrorData> {
        let principal = principal(&context)?;
        let args = request.arguments.unwrap_or_default();
        pipeline::call(&self.state, principal, &request.name, args)
            .await
            .map(Into::into)
    }

    fn get_tool(&self, name: &str) -> Option<Definition> {
        tools::find(name).map(tools::Tool::definition)
    }
}

/// The agent the bearer middleware authenticated; rmcp passes the HTTP
/// request parts through.
fn principal(context: &RequestContext<RoleServer>) -> Result<AgentPrincipal, ErrorData> {
    context
        .extensions
        .get::<axum::http::request::Parts>()
        .and_then(|parts| parts.extensions.get::<AgentPrincipal>())
        .copied()
        .ok_or_else(|| ErrorData::internal_error("request is not authenticated", None))
}
