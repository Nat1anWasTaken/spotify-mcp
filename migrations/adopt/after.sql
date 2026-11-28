-- The Spotify tokens become a link with two secrets. They were sealed the
-- way link secrets are (the user ID as associated data), so they move as is.
INSERT INTO links (user_id, status, token_hash, expires_at, last_ok_at, last_error, metadata,
                   created_at, updated_at)
SELECT user_id, status, token_hash, expires_at, last_ok_at, last_error,
       jsonb_build_object('scopes', to_jsonb(scopes), 'authorized_at', authorized_at),
       created_at, updated_at
FROM spotify_links;
INSERT INTO link_secrets (user_id, name, sealed)
SELECT user_id, 'access_token', access_token_sealed FROM spotify_links
UNION ALL
SELECT user_id, 'refresh_token', refresh_token_sealed FROM spotify_links;
DROP TABLE spotify_links;
