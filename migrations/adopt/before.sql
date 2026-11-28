-- Adopts a spotify-mcp database from before mcp-backbone: reshapes the
-- tables the framework shares with it, keeping every row. Runs once, in one
-- transaction, before the framework's baseline creates the tables missing.

-- Users: the Spotify ID is the identity's subject (and the login, as Spotify
-- has no handle); product and country move into the profile.
ALTER TABLE users RENAME COLUMN spotify_id TO subject;
ALTER TABLE users RENAME CONSTRAINT users_spotify_id_key TO users_subject_key;
ALTER TABLE users RENAME COLUMN display_name TO name;
ALTER TABLE users ADD COLUMN login text;
ALTER TABLE users ADD COLUMN profile jsonb NOT NULL DEFAULT '{}';
UPDATE users SET
    login = subject,
    profile = jsonb_strip_nulls(jsonb_build_object('product', product, 'country', country));
ALTER TABLE users DROP COLUMN product, DROP COLUMN country;

-- Sign-in attempts live ten minutes; one in flight now has to start over.
DROP TABLE link_attempts;

-- Sessions gain a scope (Spotify has none) and the outbox settings.
ALTER TABLE agent_sessions ADD COLUMN scope jsonb NOT NULL DEFAULT '{}';
ALTER TABLE agent_sessions ADD COLUMN use_outbox boolean NOT NULL DEFAULT false;
ALTER TABLE agent_sessions ADD COLUMN outbox_ttl_minutes integer NOT NULL DEFAULT 60
    CHECK (outbox_ttl_minutes BETWEEN 1 AND 10080);

-- The call log gains the outbox's statuses and columns.
ALTER TABLE tool_calls ADD COLUMN original_arguments jsonb;
ALTER TABLE tool_calls ADD COLUMN outbox_expires_at timestamptz;
ALTER TABLE tool_calls ADD COLUMN decided_at timestamptz;
ALTER TABLE tool_calls DROP CONSTRAINT tool_calls_status_check;
ALTER TABLE tool_calls ADD CONSTRAINT tool_calls_status_check CHECK (status IN
    ('running', 'ok', 'error', 'denied', 'queued', 'discarded', 'expired'));
