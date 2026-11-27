# Architecture

spotify-mcp lets AI agents act on a user's Spotify account over MCP: search,
playlists, library, follows and playback. Users sign in to the dashboard with
Spotify; that sign-in is also what links their Spotify account. The dashboard
sets each connected agent's permissions and shows a log of every tool call.

The OAuth server for MCP clients, dashboard sessions, per-session
permissions, the tool pipeline, the call log and the dashboard itself come
from [mcp-backbone](https://github.com/Nat1anWasTaken/mcp-backbone): the
`mcp-backbone` crate and the `@nat1an/mcp-backbone-dashboard`
package. This repository holds what is Spotify's. The framework's own
documentation covers its parts; this one covers Spotify.

Everything is served from one origin (`PUBLIC_URL`):

| Path | Auth | What |
| --- | --- | --- |
| `/mcp` | OAuth bearer | MCP streamable HTTP endpoint (the framework) |
| `/.well-known/*`, `/oauth/*` | none / client auth | OAuth 2.1 authorization server (the framework) |
| `/auth/spotify/*` | pre-auth cookie | "Continue with Spotify" sign-in |
| `/auth/logout`, `/api/*` | `spmcp_session` cookie | Dashboard API (the framework's, plus `/api/now-playing`) |
| everything else | none | Dashboard SPA (`web/dist`) |

## Crates

```
crates/spotify   Spotify Web API client: OAuth tokens + one Account method per tool.
                 No HTTP serving, MCP, or database code.
crates/server    the mcp-backbone service (binary `spotify-mcp`)
  service.rs     the `Service` impl: account, refresh, error classes, routes
  tools.rs       the tool registry
  auth.rs        "Continue with Spotify" (sign-in = link)
  links.rs       Spotify tokens in the framework's link store, on-demand refresh
  api.rs         /api/now-playing
  config.rs      Spotify settings; testing.rs, the Spotify mock
migrations/      legacy/ (the schema before mcp-backbone) and adopt/ (its one-time adoption)
web/             the dashboard: the package, plus Spotify's login, cards and re-link banner
```

## Spotify sign-in (linking)

1. The SPA sends the browser to `GET /auth/spotify/start?next=/path`. `next`
   must be a same-origin path, for example `/oauth/authorize?...` when the
   user is in the middle of connecting an agent.
2. The server starts a sign-in attempt (`mcp_backbone::auth::attempts`): a
   random `state` (stored hashed), the PKCE verifier (stored sealed), `next`,
   and a 10 minute TTL. It sets the
   pre-auth cookie `spmcp_link` (path `/auth/spotify`, stored hashed) and
   responds `302` to `accounts.spotify.com/authorize`. The redirect asks for
   `response_type=code`, `redirect_uri={PUBLIC_URL}/auth/spotify/callback`,
   every scope in `spotify::oauth::SCOPES`, and an S256 `code_challenge`. It
   adds `show_dialog=true` when the user is already signed in (a re-link or
   an account switch).
3. `GET /auth/spotify/callback?code&state` (or `?error`):
   1. Checks the cookie against the attempt, then deletes the attempt (single use).
   2. Exchanges the code (`Spotify::exchange_code`) and loads `GET /v1/me`.
   3. Upserts the user with the Spotify ID as `subject` and `login`, product
      and country in `profile`, and stores the tokens (`links::store_linked`).
   4. Starts a dashboard session (`spmcp_session`) and `302`s to `next`.

   If the user is signed in to the dashboard as a different Spotify user, the
   old session is replaced, since Spotify said who they are. Failures redirect
   to `/login?error=<code>`: `denied` (the user declined), `expired` (the
   attempt is gone or the cookie doesn't match), `not_allowed` (Spotify
   answers `/me` with 403: a development-mode app serves only the up to 5
   users on its allowlist in the Spotify developer dashboard) or `spotify`
   (any other upstream failure).

## Tokens and renewal (`links.rs`)

- The framework's `links` row holds `token_hash` (hex SHA-256 of the access
  token), `expires_at`, `status` (`ok | renewing | needs_relink`),
  `last_ok_at`, `last_error`, and as metadata the granted `scopes` and
  `authorized_at`. The access and refresh tokens are its sealed secrets
  `access_token` and `refresh_token` (AES-256-GCM, AAD = user ID).
- `links::account` runs on every tool call. If the access token expires
  within 5 minutes, it refreshes it under the user's lock
  (`Core::user_lock`), so concurrent calls refresh only once. Spotify may
  leave out a new `refresh_token` on refresh; the old one is kept then.
- A refresh that fails with `invalid_grant` sets `needs_relink`, and the
  agent is told to have the user sign in again. A 401 from the Web API
  before expiry makes the framework call `Service::refresh` once and retry
  the call; `/api/now-playing` goes through `AppState::call`, which does the
  same.
- There is no background renewal; refreshing on demand is enough. But since
  2026-06-18 Spotify refresh tokens expire 6 months after the user's
  original authorization, and refreshing does not extend that. So the link's
  `authorized_at` records each sign-in, and `LinkStatus` exposes
  `reauthorize_by` = `authorized_at` + 6 months. The dashboard warns in the
  last 14 days. After that, `invalid_grant` → `needs_relink` as usual.

## Migration from before mcp-backbone

A database created before the framework (`_sqlx_migrations` at version 1)
is adopted once at startup, in one transaction (`migrations/adopt`): users
keep their IDs (the Spotify ID becomes `subject`), `spotify_links` becomes a
link whose secrets are the same sealed bytes, sessions and the call log keep
every row, and the legacy table is renamed `_legacy_sqlx_migrations`.
Dashboard cookies stay valid. A test adopts a legacy database and compares
its schema with a fresh one.

## `crates/spotify` public API (contract between the crates)

```rust
pub struct Spotify;                     // Clone. App credentials + HTTP client.
pub struct Credentials { pub client_id: String, pub client_secret: String }
pub const ACCOUNTS_URL: &str; pub const API_URL: &str;
impl Spotify {
    pub fn new(Credentials) -> Self;
    pub fn with_urls(Credentials, accounts_url: &str, api_url: &str) -> Self; // tests: mock server
    pub fn client_id(&self) -> &str;
    pub fn account(&self, access_token: &str) -> Account;
    // oauth.rs
    pub fn authorize_url(&self, redirect_uri, state, code_challenge, show_dialog: bool) -> String;
    pub async fn exchange_code(&self, code, redirect_uri, verifier) -> Result<oauth::Tokens>;
    pub async fn refresh(&self, refresh_token) -> Result<oauth::Tokens>;  // InvalidGrant when revoked
}
pub mod oauth {
    pub const SCOPES: &[&str];
    pub struct Tokens { access_token, refresh_token: Option<String>, expires_in: i64, scope: String }
    impl Tokens { pub fn scopes(&self) -> Vec<String> }
    pub fn missing_scopes(granted: &[String]) -> Vec<&'static str>;
    pub fn new_verifier() -> String;          // 43 base64url chars
    pub fn challenge(verifier: &str) -> String; // S256
}
pub struct Account;                     // Clone, Debug hides the token
impl Account {
    pub async fn profile(&self) -> Result<Profile>;   // GET /v1/me, for sign-in
    pub async fn now_playing(&self) -> Result<Value>; // NowPlaying shape or Null, for /api/now-playing
    // + one method per tool below: async fn <tool>(&self, <Request>) -> Result<Value | ()>
}
pub struct Profile { id, display_name, email, avatar_url, product, country: Option<String>.. }
pub enum Error {   // Display texts are written for agents
    Unauthorized, InvalidGrant, RateLimited { retry_after }, PremiumRequired, NoActiveDevice,
    NotFound(String), Forbidden(String), InvalidArgument(String),
    Api { status, message }, Http(reqwest::Error), Decode(String),
}
pub mod types;   // request DTOs: Deserialize + Serialize + JsonSchema, deny_unknown_fields,
                 // doc comments written for agents (they become the input schema)
```

Responses are compacted for agents. Each item carries `uri`, `id`, `name`,
artists as `[{name, uri}]`, one image URL, `duration` as `m:ss` and the
fields that matter for that kind of item. `available_markets` and the
nested `href`/`external_urls` noise are dropped. Paged lists come back as
`{ items, total, offset, next_offset }`, where `next_offset` is null on the
last page.

## Tools

The `group` column is the `spotify::ops` module and the dashboard grouping
(`get_followed_artists`, from `ops/follow.rs`, is grouped with the library).
The registry lists the tools in this order.
The effect column is the `Effect` passed to `Tool::new`: `R` Read,
`A` Additive, `I` Idempotent, `D` Destructive.

| group | tool | request | effect |
| --- | --- | --- | --- |
| users | get_me | NoParams | R |
| search | search | SearchRequest { query, types, market?, limit? (1–10, default 5), offset? (0–1000) } | R |
| catalog | get_tracks, get_albums, get_artists, get_shows, get_episodes, get_audiobooks, get_chapters | IdsRequest { ids (1–10), market? } | R |
| catalog | get_album_tracks, get_show_episodes, get_audiobook_chapters | PagedIdRequest { id, market?, limit? (1–50), offset? } | R |
| catalog | get_artist_albums | ArtistAlbumsRequest { id, include_groups?, market?, limit? (1–10), offset? } | R |
| library | get_saved | SavedRequest { kind: track\|album\|episode\|show\|audiobook, market?, limit?, offset? } | R |
| library | get_followed_artists | FollowedArtistsRequest { after?, limit? (1–50) } | R |
| library | check_saved | LibraryRequest { kind, ids (1–40) } | R |
| library | save_to_library | LibraryRequest | I |
| library | remove_from_library | LibraryRequest | D |
| personalization | get_top_items | TopItemsRequest { kind: artists\|tracks, time_range?, limit?, offset? } | R |
| personalization | get_recently_played | RecentlyPlayedRequest { limit?, after?, before? } | R |
| playlists | list_my_playlists | PageRequest { limit?, offset? } | R |
| playlists | get_playlist | GetPlaylistRequest { playlist_id, market? } | R |
| playlists | get_playlist_items | PlaylistItemsRequest { playlist_id, market?, limit?, offset? } | R |
| playlists | get_playlist_cover | PlaylistRequest { playlist_id } | R |
| playlists | create_playlist | CreatePlaylistRequest { name, description?, public?, collaborative? } | A |
| playlists | update_playlist_details | UpdatePlaylistRequest { playlist_id, name?, description?, public?, collaborative? } | I |
| playlists | add_playlist_items | AddItemsRequest { playlist_id, uris (1–100), position? } | A |
| playlists | reorder_playlist_items | ReorderItemsRequest { playlist_id, range_start, insert_before, range_length?, snapshot_id? } | A |
| playlists | remove_playlist_items | RemoveItemsRequest { playlist_id, uris (1–100), snapshot_id? } | D |
| playlists | replace_playlist_items | ReplaceItemsRequest { playlist_id, uris (0–100) } | D |
| playlists | upload_playlist_cover | UploadCoverRequest { playlist_id, jpeg_base64 } (≤ 256 KB) | D |
| player | get_playback_state, get_currently_playing | MarketRequest { market? } | R |
| player | list_devices, get_queue | NoParams | R |
| player | transfer_playback | TransferRequest { device_id, play? } | I |
| player | play | PlayRequest { device_id?, context_uri?, uris?, offset?, position_ms? } | I |
| player | pause | DeviceRequest { device_id? } | I |
| player | next, previous | DeviceRequest | A |
| player | seek | SeekRequest { position_ms, device_id? } | I |
| player | set_repeat | RepeatRequest { state: track\|context\|off, device_id? } | I |
| player | set_shuffle | ShuffleRequest { state: bool, device_id? } | I |
| player | set_volume | VolumeRequest { volume_percent, device_id? } | I |
| player | add_to_queue | QueueRequest { uri, device_id? } | A |

Spotify has removed or deprecated some endpoints for development-mode apps,
so these features are not offered: recommendations, related artists, audio
features and analysis, featured and category playlists (Nov 2024), and
(Feb 2026) other users' profiles and playlists, artist top tracks, new
releases, browse categories and markets. The `get_*` catalog tools fetch
each ID with the single-item endpoint, because the batch "get several"
endpoints are gone. Following artists, users and playlists is done with the
library tools (`kind: artist | user | playlist`), because Spotify merged
those endpoints into `/me/library`. See "Spotify facts" for details.

## Spotify facts (verified 2026-10-20)

Sources: developer.spotify.com Web API reference, the changelogs for
February, March and May 2026, the "February 2026 Web API Dev Mode Changes"
migration guide, the quota-modes concept page, and the blog post "Introducing
refresh token expiration" (2026-06-18).

**Quota modes.** These changes apply to *development mode* apps: new apps
since 2026-02-11, and existing ones since 2026-03-09. Extended-quota apps are
unaffected, but since 2025-05-15 Spotify grants extended quota only to
organizations with 250k MAU or more, so a self-hosted spotify-mcp runs in
development mode. In development mode:
- at most 5 users, each added to the app's allowlist in the developer
  dashboard. A user who isn't on the allowlist can sign in, but every API
  call they make returns 403.
- the app owner must have Spotify Premium, or the app stops working.
- requests from all of a developer's dev-mode apps share a per-account
  quota, counted in buckets of endpoints. When it's exhausted, Spotify
  returns `429 {"error":{"message":"Too many requests","reason":"QUOTA_EXCEEDED"}}`.

**Removed for dev mode (Feb 2026), and the replacements we use:**
- `POST /users/{id}/playlists` is replaced by `POST /me/playlists`.
- `GET|POST|PUT|DELETE /playlists/{id}/tracks` are replaced by
  `/playlists/{id}/items`. Delete takes the body
  `{"items":[{"uri"}], "snapshot_id"?}` (it was `tracks`). The playlist
  object's `tracks` field is now `items`, and each entry's `track` is now
  `item`. The client accepts both shapes.
- The entity-specific endpoints `PUT|DELETE /me/{tracks,albums,episodes,shows,audiobooks}`,
  `/me/following`, `/playlists/{id}/followers` and their `/contains`
  variants are replaced by `PUT|DELETE /me/library?uris=` and
  `GET /me/library/contains?uris=` (at most 40 URIs, comma-separated).
  The reference lists track, album, episode, show, audiobook, user and
  playlist URIs, and artist URIs for `/contains`. The migration guide's
  save example also uses artist URIs. Required scopes:
  `user-library-*`, `user-follow-*` and `playlist-*`.
- Removed with no replacement: `GET /tracks`, `/albums`, `/artists`,
  `/shows`, `/episodes`, `/audiobooks` and `/chapters` (the "several"
  batches), `GET /artists/{id}/top-tracks`, `GET /markets`,
  `GET /browse/new-releases`, `GET /browse/categories[/{id}]`,
  `GET /users/{id}` and `GET /users/{id}/playlists`.
- Still available: the single-item `GET /{tracks,albums,artists,shows,episodes,audiobooks,chapters}/{id}`,
  album tracks, artist albums, show episodes, audiobook chapters, search,
  `GET /me`, `GET /me/{tracks,albums,episodes,shows,audiobooks}`,
  `GET /me/following?type=artist`, `GET /me/playlists`, `GET /me/top/{type}`,
  `GET /me/player/recently-played`, every `/me/player*` endpoint,
  `GET|PUT /playlists/{id}`, and `GET|PUT /playlists/{id}/images`.

**Changed limits:**
- `GET /search`: `limit` is 0–10 and defaults to 5 (it was 50 and 20).
  `offset` is at most 1000.
- `GET /artists/{id}/albums`: `limit` is 1–10 and defaults to 5.
- Other paged endpoints (album tracks, show episodes, saved items, top
  items, followed artists, playlist items, my playlists) are still 1–50,
  default 20.
- Playlist add, replace and remove: at most 100 items per request.

**Changed visibility:**
- `GET /playlists/{id}` returns `items` only for playlists the user owns or
  collaborates on. `GET /playlists/{id}/items` returns 403 for any other
  playlist.
- Spotify-owned editorial and algorithmic playlists are not available to
  new apps (Nov 2024).

**Removed fields** (we never rely on them):
- album: `album_group`, `label`, `popularity`, `available_markets`
- artist: `followers`, `popularity`
- track: `popularity`, `available_markets`, `linked_from`. `external_ids`
  was restored in March 2026.
- show and audiobook: `publisher`, `available_markets`
- user (`/me`): `country`, `email`, `explicit_content`, `followers`,
  `product`

The user fields matter most. In dev mode, `Profile.email`, `country` and
`product` are usually `None`. May 2026 added `account_id` to `/me`, a
stable pseudonymous ID that Spotify recommends over `id` for linking.

**Tokens.** Since 2026-06-18 for new apps, and 2026-07-20 for existing
ones, refresh tokens expire 6 months after the user's original
authorization. Refreshing does not extend that. On expiry, `/api/token`
returns `400 {"error":"invalid_grant"}`, which we map to
`Error::InvalidGrant`. No expiry field is returned, so the server must
record when the user authorized and ask them to sign in again before 6
months pass.

**Error shapes.**
- Web API errors look like `{"error":{"status","message","reason"?}}`. The
  player uses these `reason`s: `NO_PREV_TRACK`, `NO_NEXT_TRACK`,
  `NO_SPECIFIC_TRACK`, `ALREADY_PAUSED`, `NOT_PAUSED`,
  `NOT_PLAYING_LOCALLY`, `NOT_PLAYING_TRACK`, `NOT_PLAYING_CONTEXT`,
  `ENDLESS_CONTEXT`, `CONTEXT_DISALLOW`, `ALREADY_PLAYING`,
  `RATE_LIMITED`, `REMOTE_CONTROL_DISALLOW`, `DEVICE_NOT_CONTROLLABLE`,
  `VOLUME_CONTROL_DISALLOW`, `NO_ACTIVE_DEVICE`, `PREMIUM_REQUIRED` and
  `UNKNOWN`.
- 429 responses carry `Retry-After` in seconds.
- Token endpoint errors follow RFC 6749: `{"error","error_description"}`.
- `GET /me/player` and `/me/player/currently-playing` return 204 with an
  empty body when nothing is playing.

## Conventions

- Runtime-checked sqlx (`query` / `query_as` with `FromRow`), never the macros.
- Secrets are stored only as hashes: cookies, OAuth codes and tokens, client
  secrets. Spotify tokens are sealed. Never log a token.
- Every module starts with a `//!` doc comment.
- Errors take one of three forms. The dashboard API uses `AppError`, rendered
  as `{error, message}`. OAuth endpoints use RFC 6749 errors. Tools return
  their failures as tool results.
- Mutating `/api` and `/auth` requests must send JSON from our own `Origin`
  (`csrf_guard`).
- Timestamps are RFC 3339. Commits use conventional commit messages.

## Dashboard API contract

The framework's API (`/api/me`, `/api/meta`, sessions, tools, tool calls,
the consent request) is documented with mcp-backbone. What is Spotify's:

- `GET /api/me` → `link` is the `LinkStatus` in `links.rs`: `linked`,
  `status`, `scopes`, `missing_scopes`, `expires_at`, `last_ok_at`,
  `last_error`, `authorized_at`, `reauthorize_by`. `user.profile` may hold
  `product` and `country`.
- `GET /api/now-playing` → the current playback (`NowPlaying` in
  `web/src/lib/types.ts`), or `null` when nothing plays or the link is not
  usable.
- `GET /auth/spotify/start?next=` and `/auth/spotify/callback`: see above.

## Config (env)

| Variable | |
| --- | --- |
| `PUBLIC_URL` | External origin. Development uses `http://127.0.0.1:5173`, because Spotify refuses `localhost` redirect URIs. |
| `BIND_ADDR` | Default `0.0.0.0:8080` |
| `DATABASE_URL` | Postgres 17 (dev: `docker-compose.dev.yml`, port 5445) |
| `SPOTIFY_CLIENT_ID`, `SPOTIFY_CLIENT_SECRET` | From the Spotify developer dashboard. Redirect URI: `$PUBLIC_URL/auth/spotify/callback` |
| `ENCRYPTION_KEY` | 32 bytes base64. Seals the Spotify tokens. |
| `SPOTIFY_ACCOUNTS_URL`, `SPOTIFY_API_URL` | Optional overrides (tests, mocks) |
| `WEB_DIST`, `RUST_LOG` | Optional |
