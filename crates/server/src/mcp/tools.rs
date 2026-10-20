//! The tool registry: one MCP tool per [`spotify::Account`] operation.

use std::{future::Future, sync::LazyLock};

use futures::future::BoxFuture;
use rmcp::model::{JsonObject, Tool as Definition, ToolAnnotations};
use schemars::JsonSchema;
use serde::{Serialize, de::DeserializeOwned};
use serde_json::Value;
use spotify::{Account, types::*};

use super::output::{IntoOutput, Output};

/// Whether a tool only reads or also changes something. Clients use the MCP
/// annotations derived from it to decide what to confirm with the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Read,
    Write,
}

/// What a tool does to the account; determines its [`Kind`] and MCP annotations.
#[derive(Clone, Copy)]
enum Effect {
    Read,
    /// Adds or changes something again on every call (a new playlist, a
    /// queued track, a skip).
    Additive,
    /// Changes state; repeating the call changes nothing more.
    Idempotent,
    /// Replaces or finalizes something the user cannot take back.
    Destructive,
}

/// A tool. Serializes as the dashboard's `ToolInfo`.
#[derive(Serialize)]
pub struct Tool {
    pub name: &'static str,
    pub title: &'static str,
    pub description: &'static str,
    pub kind: Kind,
    /// A write that removes or replaces something the user cannot get back.
    pub destructive: bool,
    /// The Web API area it belongs to (`spotify::ops` module), for grouping
    /// in the dashboard.
    pub group: &'static str,
    #[serde(skip)]
    definition: Definition,
    #[serde(skip)]
    parse: Box<Parse>,
}

/// Parses tool arguments into a request.
type Parse = dyn Fn(&Value) -> serde_json::Result<Box<dyn Call>> + Send + Sync;

/// A tool request with valid arguments.
pub trait Call: Send + Sync {
    /// The request as the call log shows it (before the log bounds its size).
    fn log_arguments(&self) -> Value;

    fn run(self: Box<Self>, account: Account) -> BoxFuture<'static, spotify::Result<Output>>;
}

struct Prepared<R, F> {
    request: R,
    run: F,
}

impl<R, F, Fut, T> Call for Prepared<R, F>
where
    R: Serialize + Send + Sync + 'static,
    F: FnOnce(Account, R) -> Fut + Send + Sync + 'static,
    Fut: Future<Output = spotify::Result<T>> + Send + 'static,
    T: IntoOutput,
{
    fn log_arguments(&self) -> Value {
        serde_json::to_value(&self.request).unwrap_or_default()
    }

    fn run(self: Box<Self>, account: Account) -> BoxFuture<'static, spotify::Result<Output>> {
        let Self { request, run } = *self;
        Box::pin(async move { Ok(run(account, request).await?.into_output()) })
    }
}

impl Tool {
    fn new<R, F, Fut, T>(
        group: &'static str,
        name: &'static str,
        effect: Effect,
        title: &'static str,
        description: &'static str,
        run: F,
    ) -> Self
    where
        R: DeserializeOwned + Serialize + JsonSchema + Send + Sync + 'static,
        F: Fn(Account, R) -> Fut + Copy + Send + Sync + 'static,
        Fut: Future<Output = spotify::Result<T>> + Send + 'static,
        T: IntoOutput,
    {
        Self {
            name,
            title,
            description,
            group,
            kind: match effect {
                Effect::Read => Kind::Read,
                _ => Kind::Write,
            },
            destructive: matches!(effect, Effect::Destructive),
            definition: definition::<R>(name, title, description, effect.annotations(title)),
            parse: Box::new(move |args| {
                let request = R::deserialize(args)?;
                Ok(Box::new(Prepared { request, run }))
            }),
        }
    }

    pub fn definition(&self) -> Definition {
        self.definition.clone()
    }

    pub fn parse(&self, args: &Value) -> serde_json::Result<Box<dyn Call>> {
        (self.parse)(args)
    }
}

impl Effect {
    fn annotations(self, title: &str) -> ToolAnnotations {
        let annotations = ToolAnnotations::with_title(title).open_world(true);
        match self {
            Self::Read => annotations.read_only(true),
            write => annotations
                .read_only(false)
                .destructive(matches!(write, Self::Destructive))
                .idempotent(matches!(write, Self::Idempotent)),
        }
    }
}

fn definition<R: JsonSchema + 'static>(
    name: &'static str,
    title: &str,
    description: &'static str,
    annotations: ToolAnnotations,
) -> Definition {
    Definition::new(name, description, JsonObject::new())
        .with_title(title)
        .with_input_schema::<R>()
        .with_annotations(annotations)
}

static TOOLS: LazyLock<Vec<Tool>> = LazyLock::new(|| {
    use Effect::*;
    vec![
        // ---- users ----
        Tool::new(
            "users",
            "get_me",
            Read,
            "Get my profile",
            "The user's Spotify profile: user URI and ID, display name and image. Email, \
             country and product (premium or free) are usually null for this app.",
            |a: Account, r: NoParams| async move { a.get_me(r).await },
        ),
        // ---- search ----
        Tool::new(
            "search",
            "search",
            Read,
            "Search Spotify",
            "Searches the catalog for tracks, albums, artists, playlists, shows, episodes or \
             audiobooks (`types`) and returns their URIs for the other tools. `query` takes \
             field filters like `artist:`, `album:`, `track:`, `year:1990-1999`. `limit` is \
             1–10 per type (default 5); page with `offset` (at most 1000).",
            |a: Account, r: SearchRequest| async move { a.search(r).await },
        ),
        // ---- catalog ----
        Tool::new(
            "catalog",
            "get_tracks",
            Read,
            "Get tracks",
            "Details of 1–10 tracks by ID, URI or link: name, artists, album, duration, \
             explicit. IDs that don't exist come back as `{id, error}`.",
            |a: Account, r: IdsRequest| async move { a.get_tracks(r).await },
        ),
        Tool::new(
            "catalog",
            "get_albums",
            Read,
            "Get albums",
            "Details of 1–10 albums by ID, URI or link: name, artists, release date, type and \
             track count. For the track list use get_album_tracks.",
            |a: Account, r: IdsRequest| async move { a.get_albums(r).await },
        ),
        Tool::new(
            "catalog",
            "get_artists",
            Read,
            "Get artists",
            "Details of 1–10 artists by ID, URI or link: name, genres and image. For their \
             releases use get_artist_albums.",
            |a: Account, r: IdsRequest| async move { a.get_artists(r).await },
        ),
        Tool::new(
            "catalog",
            "get_shows",
            Read,
            "Get podcast shows",
            "Details of 1–10 podcast shows by ID, URI or link. For the episodes use \
             get_show_episodes.",
            |a: Account, r: IdsRequest| async move { a.get_shows(r).await },
        ),
        Tool::new(
            "catalog",
            "get_episodes",
            Read,
            "Get podcast episodes",
            "Details of 1–10 podcast episodes by ID, URI or link: name, show, release date, \
             duration and description.",
            |a: Account, r: IdsRequest| async move { a.get_episodes(r).await },
        ),
        Tool::new(
            "catalog",
            "get_audiobooks",
            Read,
            "Get audiobooks",
            "Details of 1–10 audiobooks by ID, URI or link: name, authors, narrators and \
             chapter count. For the chapters use get_audiobook_chapters.",
            |a: Account, r: IdsRequest| async move { a.get_audiobooks(r).await },
        ),
        Tool::new(
            "catalog",
            "get_chapters",
            Read,
            "Get audiobook chapters",
            "Details of 1–10 audiobook chapters by ID, URI or link.",
            |a: Account, r: IdsRequest| async move { a.get_chapters(r).await },
        ),
        Tool::new(
            "catalog",
            "get_album_tracks",
            Read,
            "Get an album's tracks",
            "A page of an album's tracks in order (`limit` 1–50, default 20). Page with \
             `offset` = the previous page's `next_offset`.",
            |a: Account, r: PagedIdRequest| async move { a.get_album_tracks(r).await },
        ),
        Tool::new(
            "catalog",
            "get_show_episodes",
            Read,
            "Get a show's episodes",
            "A page of a podcast show's episodes, newest first (`limit` 1–50, default 20). \
             Page with `offset` = `next_offset`.",
            |a: Account, r: PagedIdRequest| async move { a.get_show_episodes(r).await },
        ),
        Tool::new(
            "catalog",
            "get_audiobook_chapters",
            Read,
            "Get an audiobook's chapters",
            "A page of an audiobook's chapters in order (`limit` 1–50, default 20). Page with \
             `offset` = `next_offset`.",
            |a: Account, r: PagedIdRequest| async move { a.get_audiobook_chapters(r).await },
        ),
        Tool::new(
            "catalog",
            "get_artist_albums",
            Read,
            "Get an artist's albums",
            "A page of an artist's releases, optionally only some `include_groups` (album, \
             single, appears_on, compilation). `limit` is 1–10 (default 5); page with \
             `offset` = `next_offset`. Spotify no longer offers an artist's top tracks.",
            |a: Account, r: ArtistAlbumsRequest| async move { a.get_artist_albums(r).await },
        ),
        // ---- library ----
        Tool::new(
            "library",
            "get_saved",
            Read,
            "Get my saved items",
            "A page of the user's saved items of one `kind`: track (Liked Songs), album, \
             episode, show or audiobook, newest first with `added_at` (`limit` 1–50). Followed \
             artists: get_followed_artists; playlists: list_my_playlists.",
            |a: Account, r: SavedRequest| async move { a.get_saved(r).await },
        ),
        Tool::new(
            "library",
            "get_followed_artists",
            Read,
            "Get my followed artists",
            "A page of the artists the user follows (`limit` 1–50). Page with `after` = the \
             previous page's `next_after`.",
            |a: Account, r: FollowedArtistsRequest| async move { a.get_followed_artists(r).await },
        ),
        Tool::new(
            "library",
            "check_saved",
            Read,
            "Check my library",
            "Whether each of 1–40 items of one `kind` is in the user's library: saved tracks \
             (Liked Songs), albums, episodes, shows or audiobooks, or followed artists, users \
             or playlists. Returns `[{uri, saved}]` in the order given.",
            |a: Account, r: LibraryRequest| async move { a.check_saved(r).await },
        ),
        Tool::new(
            "library",
            "save_to_library",
            Idempotent,
            "Save to my library or follow",
            "Saves 1–40 items of one `kind` to the user's library: track (Liked Songs), album, \
             episode, show or audiobook. With `kind` artist, user or playlist it follows them \
             instead. Saving something already saved changes nothing.",
            |a: Account, r: LibraryRequest| async move { a.save_to_library(r).await },
        ),
        Tool::new(
            "library",
            "remove_from_library",
            Destructive,
            "Remove from my library or unfollow",
            "Removes 1–40 items of one `kind` from the user's library: track (Liked Songs), \
             album, episode, show or audiobook. With `kind` artist, user or playlist it \
             unfollows them; unfollowing a playlist the user owns removes it from their \
             playlists.",
            |a: Account, r: LibraryRequest| async move { a.remove_from_library(r).await },
        ),
        // ---- personalization ----
        Tool::new(
            "personalization",
            "get_top_items",
            Read,
            "Get my top artists or tracks",
            "The user's most played `artists` or `tracks` over `time_range`: short_term \
             (~4 weeks), medium_term (~6 months, default) or long_term (~1 year). `limit` \
             1–50, page with `offset`.",
            |a: Account, r: TopItemsRequest| async move { a.get_top_items(r).await },
        ),
        Tool::new(
            "personalization",
            "get_recently_played",
            Read,
            "Get recently played tracks",
            "Tracks the user played recently (at most the last 50), newest first, with \
             `played_at` and the context they played from. Page back with `before` = \
             `next_before`. Podcast episodes are not included.",
            |a: Account, r: RecentlyPlayedRequest| async move { a.get_recently_played(r).await },
        ),
        // ---- playlists ----
        Tool::new(
            "playlists",
            "list_my_playlists",
            Read,
            "List my playlists",
            "A page of the playlists the user owns or follows (`limit` 1–50), with URI, \
             owner, item count and `snapshot_id`. Start here to find the user's playlists.",
            |a: Account, r: PageRequest| async move { a.list_my_playlists(r).await },
        ),
        Tool::new(
            "playlists",
            "get_playlist",
            Read,
            "Get a playlist",
            "A playlist's details (name, description, owner, public, collaborative, \
             `snapshot_id`), plus its first page of items when the user owns or collaborates \
             on it.",
            |a: Account, r: GetPlaylistRequest| async move { a.get_playlist(r).await },
        ),
        Tool::new(
            "playlists",
            "get_playlist_items",
            Read,
            "Get a playlist's items",
            "A page of a playlist's tracks and episodes with `added_at` (`limit` 1–50, page \
             with `offset`). Works only for playlists the user owns or collaborates on; \
             Spotify refuses other playlists' items.",
            |a: Account, r: PlaylistItemsRequest| async move { a.get_playlist_items(r).await },
        ),
        Tool::new(
            "playlists",
            "get_playlist_cover",
            Read,
            "Get a playlist's cover",
            "The URLs of a playlist's cover image in its available sizes, largest first.",
            |a: Account, r: PlaylistRequest| async move { a.get_playlist_cover(r).await },
        ),
        Tool::new(
            "playlists",
            "create_playlist",
            Additive,
            "Create a playlist",
            "Creates a new, empty playlist owned by the user and returns it with its URI. \
             Public by default; `collaborative: true` needs `public: false`. Then add tracks \
             with add_playlist_items.",
            |a: Account, r: CreatePlaylistRequest| async move { a.create_playlist(r).await },
        ),
        Tool::new(
            "playlists",
            "update_playlist_details",
            Idempotent,
            "Update playlist details",
            "Changes the name, description, public or collaborative flag of a playlist the \
             user owns. Omitted fields stay as they are.",
            |a: Account, r: UpdatePlaylistRequest| async move { a.update_playlist_details(r).await },
        ),
        Tool::new(
            "playlists",
            "add_playlist_items",
            Additive,
            "Add items to a playlist",
            "Adds 1–100 track or episode URIs to a playlist the user owns or collaborates on, \
             in order, at `position` or at the end. Adding twice adds duplicates. Returns the \
             new `snapshot_id`.",
            |a: Account, r: AddItemsRequest| async move { a.add_playlist_items(r).await },
        ),
        Tool::new(
            "playlists",
            "reorder_playlist_items",
            Additive,
            "Reorder playlist items",
            "Moves `range_length` (default 1) items starting at `range_start` to before \
             `insert_before` (0-based, counted before the move). Pass the `snapshot_id` the \
             positions were read from so Spotify applies them to that version. Returns the new \
             `snapshot_id`.",
            |a: Account, r: ReorderItemsRequest| async move { a.reorder_playlist_items(r).await },
        ),
        Tool::new(
            "playlists",
            "remove_playlist_items",
            Destructive,
            "Remove items from a playlist",
            "Removes every occurrence of 1–100 track or episode URIs from a playlist the user \
             owns or collaborates on. Pass `snapshot_id` to remove from the version you read. \
             Returns the new `snapshot_id`.",
            |a: Account, r: RemoveItemsRequest| async move { a.remove_playlist_items(r).await },
        ),
        Tool::new(
            "playlists",
            "replace_playlist_items",
            Destructive,
            "Replace a playlist's items",
            "Replaces all of a playlist's items with 0–100 track or episode URIs, in order; an \
             empty list clears it. The previous contents are lost. Returns the new \
             `snapshot_id`.",
            |a: Account, r: ReplaceItemsRequest| async move { a.replace_playlist_items(r).await },
        ),
        Tool::new(
            "playlists",
            "upload_playlist_cover",
            Destructive,
            "Upload a playlist cover",
            "Replaces the cover image of a playlist the user owns with a base64 JPEG (at most \
             256 KB encoded; a `data:` URL prefix is accepted).",
            |a: Account, r: UploadCoverRequest| async move { a.upload_playlist_cover(r).await },
        ),
        // ---- player ----
        Tool::new(
            "player",
            "get_playback_state",
            Read,
            "Get playback state",
            "What is playing and where: device, progress, shuffle and repeat state, context \
             and item. The result is just \"Done.\" when nothing is playing.",
            |a: Account, r: MarketRequest| async move { a.get_playback_state(r).await },
        ),
        Tool::new(
            "player",
            "get_currently_playing",
            Read,
            "Get the current track",
            "The track or episode playing now and its progress. The result is just \"Done.\" \
             when nothing is playing.",
            |a: Account, r: MarketRequest| async move { a.get_currently_playing(r).await },
        ),
        Tool::new(
            "player",
            "list_devices",
            Read,
            "List playback devices",
            "The user's devices with Spotify open: ID, name, type, whether active, and volume. \
             Use a `device_id` from here with play or transfer_playback.",
            |a: Account, r: NoParams| async move { a.list_devices(r).await },
        ),
        Tool::new(
            "player",
            "get_queue",
            Read,
            "Get the playback queue",
            "The item playing now and the upcoming queue.",
            |a: Account, r: NoParams| async move { a.get_queue(r).await },
        ),
        Tool::new(
            "player",
            "transfer_playback",
            Idempotent,
            "Transfer playback",
            "Moves playback to `device_id` (from list_devices); `play: true` also starts \
             playing there. Use it when no device is active. Needs Premium.",
            |a: Account, r: TransferRequest| async move { a.transfer_playback(r).await },
        ),
        Tool::new(
            "player",
            "play",
            Idempotent,
            "Start or resume playback",
            "Starts or resumes playback on the active device or `device_id`: an album, artist, \
             playlist or show by `context_uri` (optionally from `offset`), or a list of track \
             and episode `uris`. Without either it resumes what was paused. Needs Premium and \
             an active device (see list_devices).",
            |a: Account, r: PlayRequest| async move { a.play(r).await },
        ),
        Tool::new(
            "player",
            "pause",
            Idempotent,
            "Pause playback",
            "Pauses playback on the active device or `device_id`. Needs Premium and an active \
             device.",
            |a: Account, r: DeviceRequest| async move { a.pause(r).await },
        ),
        Tool::new(
            "player",
            "next",
            Additive,
            "Skip to next",
            "Skips to the next item in the queue. Each call skips once. Needs Premium and an \
             active device.",
            |a: Account, r: DeviceRequest| async move { a.next(r).await },
        ),
        Tool::new(
            "player",
            "previous",
            Additive,
            "Skip to previous",
            "Goes back to the previous item. Each call skips once. Needs Premium and an active \
             device.",
            |a: Account, r: DeviceRequest| async move { a.previous(r).await },
        ),
        Tool::new(
            "player",
            "seek",
            Idempotent,
            "Seek",
            "Jumps to `position_ms` in the current item; past the end skips to the next one. \
             Needs Premium and an active device.",
            |a: Account, r: SeekRequest| async move { a.seek(r).await },
        ),
        Tool::new(
            "player",
            "set_repeat",
            Idempotent,
            "Set repeat mode",
            "Sets repeat to `track`, `context` (the album, playlist, ...) or `off`. Needs \
             Premium and an active device.",
            |a: Account, r: RepeatRequest| async move { a.set_repeat(r).await },
        ),
        Tool::new(
            "player",
            "set_shuffle",
            Idempotent,
            "Set shuffle",
            "Turns shuffle on (`state: true`) or off. Needs Premium and an active device.",
            |a: Account, r: ShuffleRequest| async move { a.set_shuffle(r).await },
        ),
        Tool::new(
            "player",
            "set_volume",
            Idempotent,
            "Set volume",
            "Sets the volume to `volume_percent` (0–100). Needs Premium and an active device \
             that allows volume control (most phones don't).",
            |a: Account, r: VolumeRequest| async move { a.set_volume(r).await },
        ),
        Tool::new(
            "player",
            "add_to_queue",
            Additive,
            "Add to queue",
            "Adds a track or episode URI to the end of the queue. Adding twice queues it \
             twice. Needs Premium and an active device.",
            |a: Account, r: QueueRequest| async move { a.add_to_queue(r).await },
        ),
    ]
});

pub fn all() -> impl Iterator<Item = &'static Tool> {
    TOOLS.iter()
}

pub fn find(name: &str) -> Option<&'static Tool> {
    TOOLS.iter().find(|tool| tool.name == name)
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    /// The dashboard's tool groups (`spotify::ops` modules), in registry order.
    const GROUPS: [&str; 7] = [
        "users",
        "search",
        "catalog",
        "library",
        "personalization",
        "playlists",
        "player",
    ];

    /// Every tool, so adding or removing one is a deliberate change here.
    const EXPECTED: &[&str] = &[
        "get_me",
        "search",
        "get_tracks",
        "get_albums",
        "get_artists",
        "get_shows",
        "get_episodes",
        "get_audiobooks",
        "get_chapters",
        "get_album_tracks",
        "get_show_episodes",
        "get_audiobook_chapters",
        "get_artist_albums",
        "get_saved",
        "get_followed_artists",
        "check_saved",
        "save_to_library",
        "remove_from_library",
        "get_top_items",
        "get_recently_played",
        "list_my_playlists",
        "get_playlist",
        "get_playlist_items",
        "get_playlist_cover",
        "create_playlist",
        "update_playlist_details",
        "add_playlist_items",
        "reorder_playlist_items",
        "remove_playlist_items",
        "replace_playlist_items",
        "upload_playlist_cover",
        "get_playback_state",
        "get_currently_playing",
        "list_devices",
        "get_queue",
        "transfer_playback",
        "play",
        "pause",
        "next",
        "previous",
        "seek",
        "set_repeat",
        "set_shuffle",
        "set_volume",
        "add_to_queue",
    ];

    #[test]
    fn registry_is_consistent() {
        let names: Vec<_> = all().map(|tool| tool.name).collect();
        let unique: BTreeSet<_> = names.iter().copied().collect();
        assert_eq!(unique.len(), names.len(), "tool names are unique");
        assert_eq!(unique, EXPECTED.iter().copied().collect::<BTreeSet<_>>());
        assert_eq!(names.len(), 45);
    }

    #[test]
    fn tools_are_ordered_by_known_groups() {
        let positions: Vec<usize> = all()
            .map(|tool| {
                GROUPS
                    .iter()
                    .position(|group| *group == tool.group)
                    .unwrap_or_else(|| panic!("{} has unknown group {}", tool.name, tool.group))
            })
            .collect();
        assert!(positions.is_sorted(), "the registry is ordered by group");
        let used: BTreeSet<_> = all().map(|tool| tool.group).collect();
        assert_eq!(used.len(), GROUPS.len(), "every group has tools");
    }

    #[test]
    fn every_tool_has_an_object_schema() {
        for tool in all() {
            let definition = tool.definition();
            assert_eq!(definition.input_schema["type"], "object", "{}", tool.name);
            assert_eq!(definition.title.as_deref(), Some(tool.title));
            assert_eq!(definition.description.as_deref(), Some(tool.description));
            assert!(!tool.description.is_empty() && !tool.title.is_empty());
        }
    }

    #[test]
    fn annotations_match_effects() {
        let effects = [
            ("R", Kind::Read, None, None),
            ("I", Kind::Write, Some(false), Some(true)),
            ("A", Kind::Write, Some(false), Some(false)),
            ("D", Kind::Write, Some(true), Some(false)),
        ];
        let effect_of = |name: &str| match name {
            "save_to_library"
            | "update_playlist_details"
            | "transfer_playback"
            | "play"
            | "pause"
            | "seek"
            | "set_repeat"
            | "set_shuffle"
            | "set_volume" => "I",
            "create_playlist"
            | "add_playlist_items"
            | "reorder_playlist_items"
            | "next"
            | "previous"
            | "add_to_queue" => "A",
            "remove_from_library"
            | "remove_playlist_items"
            | "replace_playlist_items"
            | "upload_playlist_cover" => "D",
            _ => "R",
        };
        for tool in all() {
            let effect = effect_of(tool.name);
            let (_, kind, destructive, idempotent) =
                effects.iter().find(|(e, ..)| *e == effect).unwrap();
            let annotations = tool.definition().annotations.unwrap();
            assert_eq!(tool.kind, *kind, "{}", tool.name);
            assert_eq!(tool.destructive, effect == "D", "{}", tool.name);
            assert_eq!(annotations.title.as_deref(), Some(tool.title));
            assert_eq!(annotations.open_world_hint, Some(true), "{}", tool.name);
            if *kind == Kind::Read {
                assert_eq!(annotations.read_only_hint, Some(true), "{}", tool.name);
                assert!(
                    tool.name.starts_with("get_")
                        || ["search", "check_saved", "list_my_playlists", "list_devices"]
                            .contains(&tool.name),
                    "{}",
                    tool.name
                );
            } else {
                assert_eq!(annotations.read_only_hint, Some(false), "{}", tool.name);
                assert_eq!(annotations.destructive_hint, *destructive, "{}", tool.name);
                assert_eq!(annotations.idempotent_hint, *idempotent, "{}", tool.name);
            }
        }
    }

    #[test]
    fn player_commands_mention_premium() {
        for tool in all().filter(|t| t.group == "player" && t.kind == Kind::Write) {
            assert!(tool.description.contains("Premium"), "{}", tool.name);
        }
    }
}
