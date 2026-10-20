-- Dashboard users. Signing in with Spotify is the only way to get an account,
-- so a user is a Spotify identity.
CREATE TABLE users (
    id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    spotify_id    text NOT NULL UNIQUE,
    display_name  text,
    email         text,
    avatar_url    text,
    product       text,                          -- 'premium', 'free', ...
    country       text,
    created_at    timestamptz NOT NULL DEFAULT now(),
    last_login_at timestamptz NOT NULL DEFAULT now()
);

-- Browser sessions for the dashboard. Only the SHA-256 of the cookie value is stored.
CREATE TABLE dashboard_sessions (
    token_hash bytea PRIMARY KEY,
    user_id    uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    created_at timestamptz NOT NULL DEFAULT now(),
    expires_at timestamptz NOT NULL
);
CREATE INDEX dashboard_sessions_user_idx ON dashboard_sessions (user_id);

-- The user's Spotify tokens. Both are AES-256-GCM sealed with ENCRYPTION_KEY
-- (nonce || ciphertext, the user ID bound as associated data). `token_hash`
-- (hex SHA-256 of the access token) tells tokens apart without decrypting,
-- so a stale failure cannot mark a fresher token invalid.
CREATE TABLE spotify_links (
    user_id               uuid PRIMARY KEY REFERENCES users (id) ON DELETE CASCADE,
    access_token_sealed   bytea NOT NULL,
    refresh_token_sealed  bytea NOT NULL,
    token_hash            text NOT NULL,
    scopes                text[] NOT NULL,
    expires_at            timestamptz NOT NULL,
    status                text NOT NULL DEFAULT 'ok'
                              CHECK (status IN ('ok', 'renewing', 'needs_relink')),
    last_ok_at            timestamptz,
    last_error            text,
    created_at            timestamptz NOT NULL DEFAULT now(),
    updated_at            timestamptz NOT NULL DEFAULT now()
);

-- In-flight "Continue with Spotify" redirects. `state_hash` is the SHA-256 of
-- the OAuth `state`, `preauth_hash` that of the cookie binding the attempt to
-- the browser that started it; the PKCE verifier is sealed (the attempt ID
-- bound as associated data).
CREATE TABLE link_attempts (
    id              uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    state_hash      bytea NOT NULL UNIQUE,
    preauth_hash    bytea NOT NULL,
    verifier_sealed bytea NOT NULL,
    next            text NOT NULL,
    created_at      timestamptz NOT NULL DEFAULT now(),
    expires_at      timestamptz NOT NULL
);

-- OAuth clients, registered dynamically (RFC 7591) or resolved from a
-- Client ID Metadata Document (client_id is then the document URL).
CREATE TABLE oauth_clients (
    client_id                  text PRIMARY KEY,
    client_secret_hash         bytea,
    client_name                text,
    client_uri                 text,
    logo_uri                   text,
    redirect_uris              text[] NOT NULL,
    grant_types                text[] NOT NULL,
    token_endpoint_auth_method text NOT NULL DEFAULT 'none',
    source                     text NOT NULL CHECK (source IN ('dcr', 'cimd')),
    metadata                   jsonb NOT NULL DEFAULT '{}',
    created_at                 timestamptz NOT NULL DEFAULT now(),
    updated_at                 timestamptz NOT NULL DEFAULT now()
);

-- A connected agent (one per user + OAuth client) and the permissions the
-- user granted it. Revoked sessions are kept so their tool-call log survives.
CREATE TABLE agent_sessions (
    id            uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id       uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    client_id     text NOT NULL REFERENCES oauth_clients (client_id) ON DELETE CASCADE,
    name          text NOT NULL,
    origin        text,
    enabled       boolean NOT NULL DEFAULT true,
    read_only     boolean NOT NULL DEFAULT false,
    allowed_tools text[],                       -- NULL = every tool
    created_at    timestamptz NOT NULL DEFAULT now(),
    last_used_at  timestamptz,
    revoked_at    timestamptz
);
CREATE UNIQUE INDEX agent_sessions_active_uniq
    ON agent_sessions (user_id, client_id) WHERE revoked_at IS NULL;

-- Pending /oauth/authorize requests waiting for the user's consent.
CREATE TABLE oauth_requests (
    id             uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    client_id      text NOT NULL REFERENCES oauth_clients (client_id) ON DELETE CASCADE,
    redirect_uri   text NOT NULL,
    state          text,
    scope          text NOT NULL,
    resource       text,
    code_challenge text NOT NULL,
    created_at     timestamptz NOT NULL DEFAULT now(),
    expires_at     timestamptz NOT NULL
);

-- Single-use authorization codes (hash only).
CREATE TABLE oauth_codes (
    code_hash        bytea PRIMARY KEY,
    client_id        text NOT NULL REFERENCES oauth_clients (client_id) ON DELETE CASCADE,
    agent_session_id uuid NOT NULL REFERENCES agent_sessions (id) ON DELETE CASCADE,
    redirect_uri     text NOT NULL,
    code_challenge   text NOT NULL,
    scope            text NOT NULL,
    resource         text,
    expires_at       timestamptz NOT NULL
);

-- Access tokens share `family_id` with the refresh token they were issued
-- with, so revoking a refresh token family also kills its access tokens.
CREATE TABLE oauth_access_tokens (
    token_hash       bytea PRIMARY KEY,
    agent_session_id uuid NOT NULL REFERENCES agent_sessions (id) ON DELETE CASCADE,
    family_id        uuid NOT NULL,
    scope            text NOT NULL,
    expires_at       timestamptz NOT NULL,
    created_at       timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX oauth_access_tokens_session_idx ON oauth_access_tokens (agent_session_id);
CREATE INDEX oauth_access_tokens_family_idx ON oauth_access_tokens (family_id);

-- Rotating refresh tokens. Presenting a token whose `used_at` is set revokes
-- its whole family (RFC 9700 refresh token reuse detection).
CREATE TABLE oauth_refresh_tokens (
    token_hash       bytea PRIMARY KEY,
    agent_session_id uuid NOT NULL REFERENCES agent_sessions (id) ON DELETE CASCADE,
    family_id        uuid NOT NULL,
    scope            text NOT NULL,
    expires_at       timestamptz NOT NULL,
    used_at          timestamptz,
    created_at       timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX oauth_refresh_tokens_session_idx ON oauth_refresh_tokens (agent_session_id);
CREATE INDEX oauth_refresh_tokens_family_idx ON oauth_refresh_tokens (family_id);

-- Every MCP tool call: 'denied' when the session's permissions refused it.
CREATE TABLE tool_calls (
    id               uuid PRIMARY KEY DEFAULT gen_random_uuid(),
    user_id          uuid NOT NULL REFERENCES users (id) ON DELETE CASCADE,
    agent_session_id uuid NOT NULL REFERENCES agent_sessions (id) ON DELETE CASCADE,
    tool             text NOT NULL,
    arguments        jsonb NOT NULL,
    status           text NOT NULL CHECK (status IN ('running', 'ok', 'error', 'denied')),
    result           jsonb,
    error            text,
    duration_ms      integer,
    created_at       timestamptz NOT NULL DEFAULT now()
);
CREATE INDEX tool_calls_user_idx ON tool_calls (user_id, created_at DESC);
CREATE INDEX tool_calls_session_idx ON tool_calls (agent_session_id, created_at DESC);
