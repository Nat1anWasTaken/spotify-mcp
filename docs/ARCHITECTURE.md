# Architecture

spotify-mcp lets AI agents act on a user's Spotify account over MCP: search,
playlists, library, follows and playback. Users sign in to the dashboard with
Spotify; that sign-in is also what links their Spotify account. The dashboard
sets each connected agent's permissions and shows a log of every tool call.

The structure mirrors `~/Developments/telegram-mcp` (OAuth server, per-session
permissions, dashboard, deployment). The idea that signing in links the
upstream account comes from `~/Developments/ilearning-mcp`.

Everything is served from one origin (`PUBLIC_URL`):

| Path | Auth | What |
| --- | --- | --- |
| `/mcp` | OAuth bearer | MCP streamable HTTP endpoint (stateless, JSON responses) |
| `/.well-known/*`, `/oauth/*` | none / client auth | OAuth 2.1 authorization server for MCP clients |
| `/auth/spotify/*`, `/auth/logout` | pre-auth cookie / session cookie | "Continue with Spotify" sign-in |
| `/api/*` | `spmcp_session` cookie | Dashboard JSON API |
| everything else | none | Dashboard SPA (`web/dist`) |

## Crates

```
crates/spotify   Spotify Web API client: OAuth tokens + one Account method per tool.
                 No HTTP serving, MCP, or database code.
crates/server    axum app (binary `spotify-mcp`)
  auth/          spotify.rs (sign-in = link), session.rs (CurrentUser, cookie sessions)
  oauth/         authorization server for MCP clients (copied from telegram-mcp)
  mcp/           server.rs (ServerHandler), tools.rs (registry), policy.rs,
                 pipeline.rs, output.rs, log.rs
  api/           dashboard API
  links.rs       sealed Spotify tokens, on-demand refresh, link status
  sessions.rs    agent sessions and their permissions
  config.rs, crypto.rs, error.rs, net.rs, state.rs, web.rs, testing.rs
```

## Spotify sign-in (linking)

1. The SPA sends the browser to `GET /auth/spotify/start?next=/path`. `next`
   must be a same-origin path, for example `/oauth/authorize?...` when the
   user is in the middle of connecting an agent.
2. The server creates a `link_attempts` row: a random `state` (stored hashed),
   a PKCE verifier (stored sealed), `next`, and a 10 minute TTL. It sets the
   pre-auth cookie `spmcp_link` (path `/auth/spotify`, stored hashed) and
   responds `302` to `accounts.spotify.com/authorize`. The redirect asks for
   `response_type=code`, `redirect_uri={PUBLIC_URL}/auth/spotify/callback`,
   every scope in `spotify::oauth::SCOPES`, and an S256 `code_challenge`. It
   adds `show_dialog=true` when the user is already signed in (a re-link or
   an account switch).
3. `GET /auth/spotify/callback?code&state` (or `?error`):
   1. Checks the cookie against the attempt, then deletes the attempt (single use).
   2. Exchanges the code (`Spotify::exchange_code`) and loads `GET /v1/me`.
   3. Upserts `users` by `spotify_id` and stores the tokens sealed (`links::store_linked`).
   4. Starts a dashboard session (`spmcp_session`) and `302`s to `next`.

   If the user is signed in to the dashboard as a different Spotify user, the
   old session is replaced, since Spotify said who they are. Failures redirect
   to `/login?error=<code>`: `denied` (the user declined), `expired` (the
   attempt is gone or the cookie doesn't match) or `spotify` (an upstream
   failure).

## Tokens and renewal (`links.rs`)

- `spotify_links` holds the access and refresh tokens, each sealed with
  AES-256-GCM (`crypto::sealed`, AAD = user ID). It also holds `token_hash`
  (hex SHA-256 of the access token), the granted `scopes`, `expires_at`,
  `status` (`ok | renewing | needs_relink`), `last_ok_at` and `last_error`.
- `links::account` runs on every tool call. If the access token expires
  within 5 minutes, it refreshes it under the user's lock in `RenewLocks`,
  so concurrent calls refresh only once. Spotify may leave out a new
  `refresh_token` on refresh; the old one is kept then.
- A refresh that fails with `invalid_grant` sets `needs_relink`, and the
  agent is told to have the user sign in again. A 401 from the Web API
  before expiry triggers one forced `links::refresh` and a single retry of
  the call.
- There is no background renewal: Spotify refresh tokens don't expire on a
  schedule, so refreshing on demand is enough.

## Tool calls

`mcp/pipeline.rs` runs each call through these steps:

1. Find the tool. An unknown tool is a protocol error.
2. Load the session's permissions fresh from the database. A revoked
   session is a protocol error.
3. Parse the arguments. Invalid arguments are an error result, logged as `error`.
4. `policy::check_tool`: the session must be enabled, the tool must be in
   `allowed_tools`, and a `read_only` session cannot call write tools. A
   failure is logged as `denied`.
5. `links::account`. When the user has no usable link, the agent gets the
   re-link message.
6. Run the call in a detached `tokio::spawn`, so it finishes and is logged
   even if the client disconnects. On `Unauthorized`, refresh and retry once.
7. Log the outcome (`tool_calls`, bounded copies of arguments and result).

`tools/list` shows only the tools `policy::check_tool` allows.

Output: an operation returns `serde_json::Value`, rendered as pretty JSON in
one text block, or `()` / `Value::Null`, rendered as `"Done."`. Every error
is a tool result whose text is the `spotify::Error` `Display` message, which
is written for agents.

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

The `group` column is the `spotify::ops` module and the dashboard grouping.
The effect column is the `Effect` passed to `Tool::new`: `R` Read,
`A` Additive, `I` Idempotent, `D` Destructive.

| group | tool | request | effect |
| --- | --- | --- | --- |
| users | get_me | NoParams | R |
| users | get_user_profile | UserRequest { user_id } | R |
| search | search | SearchRequest { query, types, market?, limit?, offset? } | R |
| catalog | get_tracks, get_albums, get_artists, get_shows, get_episodes, get_audiobooks, get_chapters | IdsRequest { ids, market? } | R |
| catalog | get_album_tracks, get_show_episodes, get_audiobook_chapters | PagedIdRequest { id, market?, limit?, offset? } | R |
| catalog | get_artist_albums | ArtistAlbumsRequest { id, include_groups?, market?, limit?, offset? } | R |
| catalog | get_artist_top_tracks | ArtistTopTracksRequest { id, market? } | R |
| catalog | list_new_releases | PageRequest { limit?, offset? } | R |
| catalog | list_categories | CategoriesRequest { locale?, limit?, offset? } | R |
| catalog | get_category | CategoryRequest { id, locale? } | R |
| catalog | list_markets | NoParams | R |
| library | get_saved | SavedRequest { kind, market?, limit?, offset? } | R |
| library | check_saved | LibraryRequest { kind, ids } | R |
| library | save_to_library | LibraryRequest | I |
| library | remove_from_library | LibraryRequest | D |
| follow | get_followed_artists | FollowedArtistsRequest { after?, limit? } | R |
| follow | check_following | FollowRequest { kind: artist\|user, ids } | R |
| follow | follow / unfollow | FollowRequest | I |
| follow | follow_playlist | FollowPlaylistRequest { playlist_id, public? } | I |
| follow | unfollow_playlist | PlaylistRequest { playlist_id } | D |
| personalization | get_top_items | TopItemsRequest { kind: artists\|tracks, time_range?, limit?, offset? } | R |
| personalization | get_recently_played | RecentlyPlayedRequest { limit?, after?, before? } | R |
| playlists | list_my_playlists | PageRequest | R |
| playlists | list_user_playlists | UserPlaylistsRequest { user_id, limit?, offset? } | R |
| playlists | get_playlist | GetPlaylistRequest { playlist_id, market? } | R |
| playlists | get_playlist_items | PlaylistItemsRequest { playlist_id, market?, limit?, offset? } | R |
| playlists | get_playlist_cover | PlaylistRequest | R |
| playlists | create_playlist | CreatePlaylistRequest { name, description?, public?, collaborative? } | A |
| playlists | update_playlist_details | UpdatePlaylistRequest { playlist_id, name?, description?, public?, collaborative? } | I |
| playlists | add_playlist_items | AddItemsRequest { playlist_id, uris, position? } | A |
| playlists | reorder_playlist_items | ReorderItemsRequest { playlist_id, range_start, insert_before, range_length?, snapshot_id? } | A |
| playlists | remove_playlist_items | RemoveItemsRequest { playlist_id, uris, snapshot_id? } | D |
| playlists | replace_playlist_items | ReplaceItemsRequest { playlist_id, uris } | D |
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

Endpoints Spotify has deprecated for new apps are not offered:
recommendations, related artists, audio features and analysis, and featured
or category playlists. See "Spotify facts" for what was verified, and when.

## Spotify facts (verified YYYY-MM-DD)

_To be filled in by whoever verifies the current Web API reference and
changelog: endpoint paths that changed, batch limits, dev-mode restrictions,
error shapes (`{error: {status, message, reason?}}`, the player `reason`
values), and `Retry-After`._

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

All JSON, snake_case. `401 {"error":"unauthorized"}` when not signed in.

### Shapes

```ts
type User = {
  id: string
  spotify_id: string
  display_name: string | null
  email: string | null
  avatar_url: string | null
  product: string | null   // "premium" | "free" | ...
  country: string | null
}

type LinkStatus = {
  linked: boolean
  status: "ok" | "renewing" | "needs_relink" | null
  scopes: string[]
  missing_scopes: string[] // non-empty: sign in again to grant them
  expires_at: string | null
  last_ok_at: string | null
  last_error: string | null
}

type AgentSession = {
  id: string
  name: string
  client_id: string
  client_name: string | null
  origin: string | null            // e.g. "claude.ai", "chatgpt.com", "localhost"
  enabled: boolean
  read_only: boolean
  allowed_tools: string[] | null   // null = all tools
  created_at: string
  last_used_at: string | null
}

type ToolInfo = {
  name: string
  title: string
  description: string
  kind: "read" | "write"
  group: "users" | "search" | "catalog" | "library" | "follow" | "personalization" | "playlists" | "player"
}

type ToolCallStatus = "running" | "ok" | "error" | "denied"
type ToolCall = {
  id: string
  session_id: string
  session_name: string
  tool: string
  arguments: Record<string, unknown> // long strings truncated
  status: ToolCallStatus
  result: unknown | null
  error: string | null
  duration_ms: number | null
  created_at: string
}

type OAuthRequest = {
  client_id: string
  client_name: string | null
  client_uri: string | null
  logo_uri: string | null
  origin: string | null
  redirect_uri: string
  scope: string
  // Set when re-authorizing a client that already has an active session.
  existing_session: { name: string; read_only: boolean } | null
}

// What GET /api/now-playing returns while something plays; null otherwise.
type NowPlaying = {
  is_playing: boolean
  progress: string | null         // "m:ss"
  device: { name: string; type: string; volume_percent: number | null } | null
  item: { uri: string; name: string; artists: { name: string; uri: string }[];
          album: string | null; image: string | null; duration: string } | null
}
```

### Endpoints

| Method & path | Body | Response |
| --- | --- | --- |
| `GET /auth/spotify/start?next=/path` | | 302 to Spotify (browser navigation) |
| `GET /auth/spotify/callback` | | sets cookie, 302 to `next` (or `/login?error=`) |
| `POST /auth/logout` | | 204 |
| `GET /api/me` | | `{ user: User, link: LinkStatus }` |
| `GET /api/now-playing` | | `NowPlaying \| null` (502 `spotify_error` on upstream failure) |
| `GET /api/tools` | | `ToolInfo[]` |
| `GET /api/sessions` | | `AgentSession[]` (active only) |
| `GET /api/sessions/:id` | | `AgentSession` |
| `PATCH /api/sessions/:id` | any of `name, enabled, read_only, allowed_tools` | `AgentSession` |
| `DELETE /api/sessions/:id` | | 204 (revokes tokens, keeps the log) |
| `GET /api/tool-calls?session_id=&status=&tool=&before=&limit=` | | `{ items: ToolCall[], next_before: string \| null }` (newest first) |
| `GET /api/tool-calls/:id` | | `ToolCall` |
| `GET /api/oauth/requests/:id` | | `OAuthRequest` |
| `POST /api/oauth/requests/:id` | `{ approve: boolean, name?, read_only? }` | `{ redirect_to: string }` |

### OAuth flow (MCP client side)

1. Client hits `/mcp` → `401` with `WWW-Authenticate: Bearer resource_metadata="<PUBLIC_URL>/.well-known/oauth-protected-resource/mcp"`.
2. The client reads the protected-resource and authorization-server metadata.
   It registers via `POST /oauth/register`, or uses a CIMD URL as its `client_id`.
3. Browser → `GET /oauth/authorize?...` (PKCE S256 required). The server
   stores an `oauth_requests` row and redirects to `/consent/<id>`. If the
   user isn't signed in, it first redirects to
   `/login?next=<authorize URL>`, whose "Continue with Spotify" button
   returns there afterwards.
4. The consent page shows `GET /api/oauth/requests/:id`. On approve it sends
   a `POST` and navigates to `redirect_to`.
5. Client exchanges the code at `POST /oauth/token`, then refreshes with
   rotating refresh tokens.

### SPA routes

These sit outside the layout:
- `/login` ("Continue with Spotify", shows `?error=`)
- `/consent/:id`

These sit inside `AppLayout`, and redirect to `/login?next=` when signed out:
- `/`: overview with profile, link status / re-link and now playing
- `/sessions` and `/sessions/:id`: permissions and allowed tools by group
- `/activity`
- `/connect`

## Config (env)

| Variable | |
| --- | --- |
| `PUBLIC_URL` | External origin. Development uses `http://127.0.0.1:5173`, because Spotify refuses `localhost` redirect URIs. |
| `BIND_ADDR` | Default `0.0.0.0:8080` |
| `DATABASE_URL` | Postgres 17 (dev: `docker-compose.dev.yml`, port 5444) |
| `SPOTIFY_CLIENT_ID`, `SPOTIFY_CLIENT_SECRET` | From the Spotify developer dashboard. Redirect URI: `$PUBLIC_URL/auth/spotify/callback` |
| `ENCRYPTION_KEY` | 32 bytes base64. Seals the Spotify tokens. |
| `SPOTIFY_ACCOUNTS_URL`, `SPOTIFY_API_URL` | Optional overrides (tests, mocks) |
| `WEB_DIST`, `RUST_LOG` | Optional |
