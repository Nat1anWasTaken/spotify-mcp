//! Dashboard endpoints of Spotify's own.

use axum::{Json, extract::State};
use mcp_backbone::{AppResult, AppState, CallError, auth::CurrentUser};
use serde_json::Value;

use crate::service::SpotifyService;

/// `GET /api/now-playing`: the user's current playback, for the overview.
/// `null` when nothing is playing or the user has no usable link. Like a
/// tool call, a token Spotify rejects early is refreshed once.
pub async fn now_playing(
    State(state): State<AppState<SpotifyService>>,
    user: CurrentUser,
) -> AppResult<Json<Value>> {
    let playing = state
        .call(
            user.id,
            |account| async move { account.now_playing().await },
        )
        .await;
    match playing {
        Ok(playing) => Ok(Json(playing)),
        Err(CallError::NotLinked | CallError::NeedsRelink) => Ok(Json(Value::Null)),
        Err(err) => Err(err.into()),
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Method, Request, StatusCode},
    };
    use serde_json::json;
    use sqlx::PgPool;

    use crate::testing::{self, dashboard_request, send};

    #[sqlx::test(migrations = false)]
    async fn me_requires_a_session(db: PgPool) {
        let state = testing::state(db).await;
        let (user_id, cookie) = testing::sign_in(&state, 7).await;
        let app = mcp_backbone::router(state);

        let response = send(
            &app,
            dashboard_request(Method::GET, "/api/me", &cookie, None),
        )
        .await;
        assert_eq!(
            response.json(),
            json!({
                "user": {
                    "id": user_id, "subject": "subject-7", "login": "user7", "name": "User 7",
                    "email": null, "avatar_url": null, "profile": {},
                },
                "link": {
                    "linked": false, "status": null, "scopes": [], "missing_scopes": [],
                    "expires_at": null, "last_ok_at": null, "last_error": null,
                    "authorized_at": null, "reauthorize_by": null,
                },
            })
        );

        let anonymous = send(&app, Request::get("/api/me").body(Body::empty()).unwrap()).await;
        assert_eq!(anonymous.status, StatusCode::UNAUTHORIZED);
        assert_eq!(anonymous.json()["error"], "unauthorized");
        let stale = dashboard_request(Method::GET, "/api/me", "spmcp_session=bogus", None);
        assert_eq!(send(&app, stale).await.status, StatusCode::UNAUTHORIZED);

        let unknown = send(
            &app,
            dashboard_request(Method::GET, "/api/nope", &cookie, None),
        )
        .await;
        assert_eq!(unknown.status, StatusCode::NOT_FOUND);
        assert_eq!(unknown.json()["error"], "not_found");
    }

    #[sqlx::test(migrations = false)]
    async fn me_shows_the_link_status(db: PgPool) {
        let spotify = testing::MockSpotify::start().await;
        let state = spotify.state(db.clone()).await;
        let (user_id, cookie) = testing::sign_in(&state, 7).await;
        crate::links::store_linked(&state, user_id, &spotify.tokens())
            .await
            .unwrap();
        let app = mcp_backbone::router(state);

        let me = send(
            &app,
            dashboard_request(Method::GET, "/api/me", &cookie, None),
        )
        .await
        .json();
        let link = &me["link"];
        assert_eq!(link["linked"], true);
        assert_eq!(link["status"], "ok");
        assert_eq!(
            link["scopes"].as_array().unwrap().len(),
            spotify::oauth::SCOPES.len()
        );
        assert_eq!(link["missing_scopes"], json!([]));
        assert!(link["expires_at"].is_string());
        assert!(link["last_ok_at"].is_string());
        assert_eq!(link["last_error"], json!(null));
        let authorized_at: chrono::DateTime<chrono::Utc> =
            serde_json::from_value(link["authorized_at"].clone()).unwrap();
        let reauthorize_by: chrono::DateTime<chrono::Utc> =
            serde_json::from_value(link["reauthorize_by"].clone()).unwrap();
        assert!(authorized_at > chrono::Utc::now() - chrono::TimeDelta::minutes(1));
        assert_eq!(
            reauthorize_by,
            authorized_at
                .checked_add_months(chrono::Months::new(6))
                .unwrap()
        );

        sqlx::query(
            "UPDATE links SET status = 'needs_relink', last_error = 'revoked',
                    metadata = jsonb_set(metadata, '{scopes}', '[\"user-read-private\"]')",
        )
        .execute(&db)
        .await
        .unwrap();
        let me = send(
            &app,
            dashboard_request(Method::GET, "/api/me", &cookie, None),
        )
        .await
        .json();
        assert_eq!(me["link"]["status"], "needs_relink");
        assert_eq!(me["link"]["last_error"], "revoked");
        assert!(
            me["link"]["missing_scopes"]
                .as_array()
                .unwrap()
                .contains(&json!("user-modify-playback-state"))
        );
    }

    #[sqlx::test(migrations = false)]
    async fn now_playing(db: PgPool) {
        use std::sync::atomic::Ordering;

        let spotify = testing::MockSpotify::start().await;
        let state = spotify.state(db.clone()).await;
        let (user_id, cookie) = testing::sign_in(&state, 7).await;
        let app = mcp_backbone::router(state.clone());
        let now_playing = || async {
            send(
                &app,
                dashboard_request(Method::GET, "/api/now-playing", &cookie, None),
            )
            .await
        };

        // Not linked.
        let response = now_playing().await;
        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.json(), json!(null));

        // Linked, nothing playing.
        crate::links::store_linked(&state, user_id, &spotify.tokens())
            .await
            .unwrap();
        assert_eq!(now_playing().await.json(), json!(null));

        *spotify.mock.playback.lock().unwrap() = Some(json!({
            "is_playing": true,
            "progress_ms": 61_000,
            "device": { "name": "Desk", "type": "Computer", "volume_percent": 40 },
            "item": {
                "type": "track", "uri": "spotify:track:1", "name": "Song", "duration_ms": 200_000,
                "artists": [{ "name": "Band", "uri": "spotify:artist:1" }],
                "album": { "name": "Record", "images": [{ "url": "https://i.scdn.co/image/a" }] },
            },
        }));
        let playing = now_playing().await.json();
        assert_eq!(playing["is_playing"], true);
        assert_eq!(playing["progress"], "1:01");
        assert_eq!(playing["device"]["name"], "Desk");
        assert_eq!(playing["item"]["name"], "Song");
        assert_eq!(playing["item"]["duration"], "3:20");

        // Spotify rejects the token early: refreshed once, then retried.
        spotify
            .mock
            .rejected
            .lock()
            .unwrap()
            .insert("access-1".into());
        let playing = now_playing().await;
        assert_eq!(playing.status, StatusCode::OK);
        assert_eq!(playing.json()["item"]["name"], "Song");
        assert_eq!(spotify.mock.refreshes.load(Ordering::SeqCst), 1);

        // Rejected even after refreshing: an upstream error, and the link
        // (holding the refreshed token) needs a re-link.
        spotify
            .mock
            .rejected
            .lock()
            .unwrap()
            .extend(["access-2".to_owned(), "access-3".to_owned()]);
        let failed = now_playing().await;
        assert_eq!(failed.status, StatusCode::BAD_GATEWAY);
        assert_eq!(failed.json()["error"], "upstream_error");
        let link = crate::links::status(&state, user_id).await.unwrap();
        assert_eq!(
            link.status,
            Some(mcp_backbone::links::LinkState::NeedsRelink)
        );
        assert_eq!(now_playing().await.json(), json!(null));
    }
}
