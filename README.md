# telegram-mcp

A remote [MCP](https://modelcontextprotocol.io) server that lets AI agents
(claude.ai, ChatGPT, Gemini, Claude Code, …) use **your own Telegram account**
over MTProto, under permissions you control from a web dashboard.

- **Sign in** to the dashboard with GitHub and link Telegram by scanning a QR code.
- **Connect** an AI client by adding `https://<host>/mcp` as a custom connector.
  It authenticates with OAuth 2.1 (dynamic client registration or client ID
  metadata documents, PKCE), and you approve it on a consent screen.
- **Control** every connected agent separately:
  - enable or disable it;
  - turn on the read-only kill switch;
  - send its write actions to your outbox, where you approve or discard them
    before they expire (after 1 minute to 1 week, your choice);
  - choose the allowed tools;
  - limit which chats it can see.
- **Audit** every tool call, and approve or discard what is in your outbox.
- **Get notified** of actions landing in your outbox, through push notifications
  from the installable dashboard app. Tap one to review the request, edit it
  (message text, target chat, …), and approve or discard it. Message text is
  edited as highlighted markdown or HTML beside a preview of how Telegram will
  show it, with a preview of every link in it.

See [docs/ARCHITECTURE.md](docs/ARCHITECTURE.md) for the design and the
dashboard API contract.

## Development

Requirements: Rust (the pinned toolchain in `rust-toolchain.toml` installs
itself), Node 22 with pnpm, Docker.

```sh
docker compose -f docker-compose.dev.yml up -d   # Postgres on localhost:5442
cp .env.example .env                             # then fill it in, see below
cargo run -p telegram-mcp                        # API + MCP on :8080, migrations run at startup
cd web && pnpm install && pnpm dev               # dashboard on :5173, proxies to :8080
```

Set `PUBLIC_URL=http://localhost:5173` while using the Vite dev server, so that
OAuth redirects and cookies stay on one origin.

Tests (the integration tests use the dev database from `DATABASE_URL`):

```sh
cargo test --workspace
cargo clippy --workspace --all-targets -- -D warnings
cd web && pnpm lint && pnpm build
```

### Configuration

| Variable | Purpose |
| --- | --- |
| `PUBLIC_URL` | External origin. OAuth metadata and the MCP resource URL derive from it. |
| `DATABASE_URL` | Postgres connection string. |
| `GITHUB_CLIENT_ID`, `GITHUB_CLIENT_SECRET` | GitHub OAuth app for dashboard login. The callback is `$PUBLIC_URL/auth/github/callback`. |
| `ALLOWED_GITHUB_LOGINS` | Comma-separated GitHub logins allowed to sign in. Leave it empty to allow anyone. |
| `TELEGRAM_API_ID`, `TELEGRAM_API_HASH` | Telegram application credentials from <https://my.telegram.org/apps>. |
| `ENCRYPTION_KEY` | 32 bytes, base64 (`openssl rand -base64 32`). It encrypts Telegram sessions at rest. |
| `VAPID_SUBJECT` | Optional contact (`mailto:` or `https:` URL) that Web Push services see. It defaults to `PUBLIC_URL`. The VAPID key itself is generated on first start and stored encrypted. |
| `BIND_ADDR`, `WEB_DIST` | Listen address (default `0.0.0.0:8080`) and dashboard build directory (default `web/dist`). |

## Deployment

The production stack is `docker-compose.yml`: the app image (the multi-stage
`Dockerfile` builds both the dashboard and the server) plus Postgres 17. It is
deployed as a Dokploy compose service built from this repository.

1. Put the variables above, plus `POSTGRES_PASSWORD`, in the service's
   environment. `DATABASE_URL` is derived from `POSTGRES_PASSWORD` in the
   compose file.
2. Point a domain at service `app`, port `8080`. TLS terminates at the
   Cloudflare tunnel in front of Traefik.
3. In claude.ai, open Settings, then Connectors, then Add custom connector.
   Enter `https://<host>/mcp` and leave the OAuth fields empty.

### Outbox notifications

The dashboard can be installed as an app, and it then receives push
notifications for actions landing in your outbox.

- **Desktop or Android:** open the dashboard in Chrome, Edge or Firefox, then
  click Enable notifications on the Overview or Outbox page.
- **iPhone or iPad (iOS 16.4 or later):** open the dashboard in Safari, tap
  Share, then Add to Home Screen. Open the app from the Home Screen, sign in,
  and enable notifications there. Safari only delivers push notifications to
  apps installed this way.
