# spotify-mcp

A remote [MCP](https://modelcontextprotocol.io) server that lets AI agents
(claude.ai, ChatGPT, Claude Code, …) use **your own Spotify account** through
the Spotify Web API, under permissions you control from a web dashboard.

- **Sign in** to the dashboard with Spotify. Signing in also links your Spotify
  account: the tokens are stored encrypted and refreshed on demand. Spotify
  ends app access six months after each sign-in, so the dashboard asks you to
  sign in again before then. Sign in again also after revoking access in your
  Spotify account settings or when a new version asks for more scopes.
- **Connect** an AI client by adding `https://<host>/mcp` as a custom connector.
  It authenticates with OAuth 2.1 (dynamic client registration or client ID
  metadata documents, PKCE), and you approve it on a consent screen.
- **Use** search, the catalog (tracks, albums, artists, shows, episodes,
  audiobooks), playlists, your library (saved items, and followed artists,
  users and playlists), top items and recently played, and playback (play, pause, skip, seek, volume, shuffle, repeat, queue,
  devices). Playback control needs Spotify Premium.
- **Control** every connected agent separately: enable or disable it, turn on
  the read-only kill switch, and choose the allowed tools, grouped by area.
- **Audit** every tool call, including denied ones.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the design and the
dashboard API contract.

## Spotify app

Create an app at <https://developer.spotify.com/dashboard>:

1. Choose **Web API** as the API used.
2. Add the redirect URIs, `$PUBLIC_URL/auth/spotify/callback` for each
   environment:
   - development: `http://127.0.0.1:5173/auth/spotify/callback`
   - production: `https://<host>/auth/spotify/callback`

   Spotify rejects `localhost` redirect URIs; loopback addresses must be
   written as `127.0.0.1` (or `[::1]`). Other URIs must use HTTPS.
3. Copy the client ID and client secret into `SPOTIFY_CLIENT_ID` and
   `SPOTIFY_CLIENT_SECRET`.
4. New apps start in **development mode**, with these limits:
   - Only users you add can sign in, at most 5. Add each one (name and
     Spotify account email) under **User Management** in the app's settings.
     Anyone else is turned away, and the dashboard tells them to ask you.
   - The app owner's Spotify account needs Premium.
   - Refresh tokens expire six months after each sign-in, whatever their
     use. Users then sign in again; the dashboard warns them two weeks ahead.
   - Spotify no longer returns email, country or subscription level from
     `/me`, so the dashboard can't tell whether a user has Premium (needed
     for playback control).

## Development

Requirements: Rust (the pinned toolchain in `rust-toolchain.toml` installs
itself), Node 22 with pnpm, Docker.

```sh
docker compose -f docker-compose.dev.yml up -d   # Postgres on localhost:5445
cp .env.example .env                             # then fill it in, see below
cargo run -p spotify-mcp                         # API + MCP on :8080, migrations run at startup
cd web && pnpm install && pnpm dev               # dashboard on http://127.0.0.1:5173, proxies to :8080
```

Open the dashboard at `http://127.0.0.1:5173`, not `localhost`, and keep
`PUBLIC_URL=http://127.0.0.1:5173` while using the Vite dev server, so that
the Spotify redirect, OAuth redirects and cookies all stay on one origin.

Tests (the integration tests use the dev database from `DATABASE_URL`; none of
them contact Spotify):

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd web && pnpm lint && pnpm format:check && pnpm build
```

### Configuration

| Variable | Purpose |
| --- | --- |
| `PUBLIC_URL` | External origin. OAuth metadata, the MCP resource URL and the Spotify redirect URI (`$PUBLIC_URL/auth/spotify/callback`) derive from it. Use `http://127.0.0.1:5173` in development. |
| `DATABASE_URL` | Postgres connection string. |
| `SPOTIFY_CLIENT_ID`, `SPOTIFY_CLIENT_SECRET` | The Spotify app's credentials, see [Spotify app](#spotify-app). |
| `ENCRYPTION_KEY` | 32 bytes, base64 (`openssl rand -base64 32`). It encrypts Spotify tokens at rest. |
| `BIND_ADDR`, `WEB_DIST` | Listen address (default `0.0.0.0:8080`) and dashboard build directory (default `web/dist`). |
| `RUST_LOG` | Log filter, for example `info,spotify_mcp=debug`. |
| `SPOTIFY_ACCOUNTS_URL`, `SPOTIFY_API_URL` | Optional overrides of `https://accounts.spotify.com` and `https://api.spotify.com`, for tests and mocks. |

## Deployment

The production stack is `docker-compose.yml`: the app image (the multi-stage
`Dockerfile` builds both the dashboard and the server) plus Postgres 17. It is
deployed as a Dokploy compose service built from this repository.

1. Put the variables above, plus `POSTGRES_PASSWORD`, in the service's
   environment. `DATABASE_URL` is derived from `POSTGRES_PASSWORD` in the
   compose file.
2. Point a domain at service `app`, port `8080`. TLS terminates at the
   Cloudflare tunnel in front of Traefik.
3. Add `https://<host>/auth/spotify/callback` to the Spotify app's redirect
   URIs, and add every user to the app while it is in development mode.

## Connecting clients

The dashboard's Connect page shows the MCP URL and these steps.

- **claude.ai** (also the Claude desktop and mobile apps): Settings, then
  Connectors, then Add custom connector. Enter `https://<host>/mcp`, leave the
  OAuth client ID and secret empty, then Connect and approve the request.
- **Claude Code:** run
  `claude mcp add --transport http spotify https://<host>/mcp`, then `/mcp`
  inside Claude Code to authenticate.
- **ChatGPT:** turn on developer mode (Settings, then Apps & Connectors, then
  Advanced settings), create a connector with `https://<host>/mcp` and OAuth
  authentication, then sign in and approve the request.
- **Other clients:** any client that supports remote MCP servers over
  Streamable HTTP with OAuth works; no client ID or secret is needed.

The first connection sends you through "Continue with Spotify" if you aren't
signed in to the dashboard yet, then to the consent screen, where you name the
session and choose whether it is read-only.
