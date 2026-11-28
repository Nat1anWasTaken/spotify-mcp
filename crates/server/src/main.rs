mod api;
mod auth;
mod config;
mod links;
mod service;
#[cfg(test)]
mod testing;
mod tools;

use mcp_backbone::migrate::Adoption;

use crate::{config::SpotifyConfig, service::SpotifyService};

/// Brings a database from before mcp-backbone into the framework's schema.
pub static ADOPTION: Adoption = Adoption {
    legacy_versions: &[1],
    before: include_str!("../../../migrations/adopt/before.sql"),
    after: include_str!("../../../migrations/adopt/after.sql"),
    service_baseline: None,
};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    let config = SpotifyConfig::from_env()?;
    // Sessions may send their write calls to the user's outbox; off for
    // each session until the user turns it on.
    mcp_backbone::App::new(SpotifyService::new(&config))
        .outbox(true)
        .push(true)
        .adopt(&ADOPTION)
        .serve()
        .await
}

#[cfg(test)]
mod tests {
    use mcp_backbone::{
        crypto::sealed,
        migrate,
        sqlx::{self, PgPool, migrate::Migrator},
        testing::{TempDatabase, schema},
    };
    use serde_json::json;
    use uuid::Uuid;

    use super::ADOPTION;
    use crate::{links, testing};

    static LEGACY: Migrator = sqlx::migrate!("../../migrations/legacy");

    const KEY: [u8; 32] = [7; 32];

    /// A database from before mcp-backbone, with a linked user, a session
    /// and a logged call, is adopted with every row, matches a fresh
    /// schema, and keeps working.
    #[sqlx::test(migrations = false)]
    async fn legacy_databases_are_adopted(db: PgPool) {
        LEGACY.run(&db).await.unwrap();
        let user_id = Uuid::new_v4();
        sqlx::query(
            "INSERT INTO users (id, spotify_id, display_name, product, country)
             VALUES ($1, 'legacy-user', 'Legacy', 'premium', 'TW')",
        )
        .bind(user_id)
        .execute(&db)
        .await
        .unwrap();
        let seal =
            |secret: &str| sealed::seal(&KEY, user_id.as_bytes(), secret.as_bytes()).unwrap();
        sqlx::query(
            "INSERT INTO spotify_links (user_id, access_token_sealed, refresh_token_sealed,
                                        token_hash, scopes, expires_at, authorized_at)
             VALUES ($1, $2, $3, $4, $5, now() + interval '1 hour', now() - interval '1 day')",
        )
        .bind(user_id)
        .bind(seal("access-legacy"))
        .bind(seal("refresh-legacy"))
        .bind(mcp_backbone::links::token_hash("access-legacy"))
        .bind(
            spotify::oauth::SCOPES
                .iter()
                .map(|s| s.to_string())
                .collect::<Vec<_>>(),
        )
        .execute(&db)
        .await
        .unwrap();
        sqlx::raw_sql(sqlx::AssertSqlSafe(format!(
            "INSERT INTO oauth_clients (client_id, redirect_uris, grant_types, source)
             VALUES ('client', ARRAY['https://agent.example/cb'], ARRAY['authorization_code'], 'dcr');
             INSERT INTO agent_sessions (id, user_id, client_id, name, read_only)
             VALUES ('00000000-0000-0000-0000-0000000000a1', '{user_id}', 'client', 'Claude', true);
             INSERT INTO tool_calls (user_id, agent_session_id, tool, arguments, status)
             VALUES ('{user_id}', '00000000-0000-0000-0000-0000000000a1', 'get_me', '{{}}', 'ok');
             INSERT INTO link_attempts (state_hash, preauth_hash, verifier_sealed, next, expires_at)
             VALUES ('\\x01', '\\x02', '\\x03', '/', now() + interval '5 minutes');"
        )))
        .execute(&db)
        .await
        .unwrap();

        migrate::run(&db, None, Some(&ADOPTION)).await.unwrap();
        // Idempotent afterwards.
        migrate::run(&db, None, Some(&ADOPTION)).await.unwrap();

        let fresh = TempDatabase::new(&db).await;
        migrate::run(&fresh.pool, None, None).await.unwrap();
        assert_eq!(schema(&db).await, schema(&fresh.pool).await);
        fresh.drop().await;

        let (subject, login, name, profile): (String, String, String, serde_json::Value) =
            sqlx::query_as("SELECT subject, login, name, profile FROM users")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(
            (subject.as_str(), login.as_str(), name.as_str()),
            ("legacy-user", "legacy-user", "Legacy")
        );
        assert_eq!(profile, json!({ "product": "premium", "country": "TW" }));
        let (read_only, scope): (bool, serde_json::Value) =
            sqlx::query_as("SELECT read_only, scope FROM agent_sessions")
                .fetch_one(&db)
                .await
                .unwrap();
        assert!(read_only);
        assert_eq!(scope, json!({}));
        let calls: i64 = sqlx::query_scalar("SELECT count(*) FROM tool_calls")
            .fetch_one(&db)
            .await
            .unwrap();
        assert_eq!(calls, 1);

        // The sealed tokens still open, and the link reads as before.
        let state = testing::state(db.clone()).await;
        let linked = links::account(&state, &state.service.spotify, user_id)
            .await
            .unwrap()
            .expect("a usable link");
        assert_eq!(
            linked.token_hash,
            Some(mcp_backbone::links::token_hash("access-legacy"))
        );
        let status = links::status(&state, user_id).await.unwrap();
        assert!(status.linked && status.missing_scopes.is_empty());
        assert!(status.authorized_at.unwrap() < chrono::Utc::now() - chrono::TimeDelta::hours(23));
        let record = mcp_backbone::links::LinkStore::new(&state)
            .load(user_id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            record.secret(&state, "refresh_token").unwrap(),
            "refresh-legacy"
        );
    }
}
