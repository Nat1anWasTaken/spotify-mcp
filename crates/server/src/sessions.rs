//! Agent sessions: one connected MCP client per user, and the permissions the
//! user granted it. Shared by the dashboard API and the MCP layer.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Deserializer, Serialize};
use sqlx::{FromRow, PgPool};
use uuid::Uuid;

use crate::error::{AppError, AppResult};

#[derive(Debug, Clone, Serialize, FromRow)]
pub struct AgentSession {
    pub id: Uuid,
    pub name: String,
    pub client_id: String,
    pub client_name: Option<String>,
    pub origin: Option<String>,
    pub enabled: bool,
    pub read_only: bool,
    /// `None` means every tool.
    pub allowed_tools: Option<Vec<String>>,
    pub created_at: DateTime<Utc>,
    pub last_used_at: Option<DateTime<Utc>>,
}

/// What a session may do, read on every MCP request so dashboard changes
/// apply immediately.
#[derive(Debug, Clone, FromRow)]
pub struct Permissions {
    pub enabled: bool,
    pub read_only: bool,
    /// `None` means every tool.
    pub allowed_tools: Option<Vec<String>>,
}

/// A partial update from `PATCH /api/sessions/:id`; absent fields are left alone.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SessionPatch {
    pub name: Option<String>,
    pub enabled: Option<bool>,
    pub read_only: Option<bool>,
    /// `Some(None)` (JSON `null`) grants every tool.
    #[serde(default, deserialize_with = "present")]
    pub allowed_tools: Option<Option<Vec<String>>>,
}

/// Maps a present field to `Some`, so `null` becomes `Some(None)` and only an
/// absent field (via `#[serde(default)]`) stays `None`.
fn present<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    T::deserialize(deserializer).map(Some)
}

/// Trims a user-supplied session name and checks it is reasonable.
pub fn normalize_name(name: &str) -> AppResult<String> {
    const MAX_CHARS: usize = 100;
    let name = name.trim();
    if name.is_empty() || name.chars().count() > MAX_CHARS {
        return Err(AppError::BadRequest(format!(
            "name must be 1 to {MAX_CHARS} characters"
        )));
    }
    Ok(name.to_owned())
}

const SELECT_ACTIVE: &str = "
    SELECT s.id, s.name, s.client_id, c.client_name, s.origin, s.enabled, s.read_only,
           s.allowed_tools, s.created_at, s.last_used_at
    FROM agent_sessions s
    JOIN oauth_clients c ON c.client_id = s.client_id
    WHERE s.user_id = $1 AND s.revoked_at IS NULL AND ($2::uuid IS NULL OR s.id = $2)
    ORDER BY s.created_at DESC";

/// The user's non-revoked sessions, newest first.
pub async fn list_active(db: &PgPool, user_id: Uuid) -> sqlx::Result<Vec<AgentSession>> {
    sqlx::query_as(SELECT_ACTIVE)
        .bind(user_id)
        .bind(None::<Uuid>)
        .fetch_all(db)
        .await
}

/// A non-revoked session owned by `user_id`.
pub async fn find(db: &PgPool, user_id: Uuid, id: Uuid) -> sqlx::Result<Option<AgentSession>> {
    sqlx::query_as(SELECT_ACTIVE)
        .bind(user_id)
        .bind(id)
        .fetch_optional(db)
        .await
}

/// The permissions of a non-revoked session owned by `user_id`.
pub async fn permissions(
    db: &PgPool,
    user_id: Uuid,
    id: Uuid,
) -> sqlx::Result<Option<Permissions>> {
    sqlx::query_as(
        "SELECT enabled, read_only, allowed_tools
         FROM agent_sessions WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL",
    )
    .bind(id)
    .bind(user_id)
    .fetch_optional(db)
    .await
}

/// Applies `patch` to a non-revoked session. Returns whether it exists.
pub async fn update(
    db: &PgPool,
    user_id: Uuid,
    id: Uuid,
    patch: &SessionPatch,
) -> sqlx::Result<bool> {
    let result = sqlx::query(
        "UPDATE agent_sessions SET
            name = COALESCE($3, name),
            enabled = COALESCE($4, enabled),
            read_only = COALESCE($5, read_only),
            allowed_tools = CASE WHEN $6 THEN $7 ELSE allowed_tools END
         WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL",
    )
    .bind(id)
    .bind(user_id)
    .bind(&patch.name)
    .bind(patch.enabled)
    .bind(patch.read_only)
    .bind(patch.allowed_tools.is_some())
    .bind(patch.allowed_tools.as_ref().and_then(Option::as_ref))
    .execute(db)
    .await?;
    Ok(result.rows_affected() == 1)
}

/// Revokes a session and every credential issued to it. Its tool-call log is
/// kept. Returns whether an active session was revoked.
pub async fn revoke(db: &PgPool, user_id: Uuid, id: Uuid) -> sqlx::Result<bool> {
    let mut tx = db.begin().await?;
    let revoked = sqlx::query(
        "UPDATE agent_sessions SET revoked_at = now()
         WHERE id = $1 AND user_id = $2 AND revoked_at IS NULL",
    )
    .bind(id)
    .bind(user_id)
    .execute(&mut *tx)
    .await?
    .rows_affected()
        == 1;
    if revoked {
        for sql in [
            "DELETE FROM oauth_access_tokens WHERE agent_session_id = $1",
            "DELETE FROM oauth_refresh_tokens WHERE agent_session_id = $1",
            "DELETE FROM oauth_codes WHERE agent_session_id = $1",
        ] {
            sqlx::query(sql).bind(id).execute(&mut *tx).await?;
        }
    }
    tx.commit().await?;
    Ok(revoked)
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn patch_distinguishes_absent_from_null() {
        let patch: SessionPatch = serde_json::from_value(json!({"read_only": false})).unwrap();
        assert_eq!(patch.allowed_tools, None);
        let patch: SessionPatch = serde_json::from_value(json!({"allowed_tools": null})).unwrap();
        assert_eq!(patch.allowed_tools, Some(None));
        let patch: SessionPatch =
            serde_json::from_value(json!({"allowed_tools": ["play"]})).unwrap();
        assert_eq!(patch.allowed_tools, Some(Some(vec!["play".into()])));
        assert!(serde_json::from_value::<SessionPatch>(json!({"bogus": 1})).is_err());
    }
}
