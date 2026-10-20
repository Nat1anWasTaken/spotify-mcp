//! The dashboard JSON API, mounted at `/api`.

mod sessions;
mod tool_calls;
mod tools;

use axum::{
    Json, Router,
    body::HttpBody,
    extract::{Request, State},
    http::{HeaderMap, header},
    middleware::{self, Next},
    response::Response,
    routing::get,
};
use serde_json::{Value, json};

use crate::{
    auth::CurrentUser,
    error::{AppError, AppResult},
    links,
    oauth::consent,
    state::AppState,
};

pub fn router(state: AppState) -> Router<AppState> {
    Router::new()
        .route("/me", get(me))
        .route("/now-playing", get(now_playing))
        .route("/tools", get(tools::list))
        .route("/tool-calls", get(tool_calls::list))
        .route("/tool-calls/{id}", get(tool_calls::show))
        .route("/sessions", get(sessions::list))
        .route(
            "/sessions/{id}",
            get(sessions::show)
                .patch(sessions::update)
                .delete(sessions::revoke),
        )
        .route(
            "/oauth/requests/{id}",
            get(consent::show).post(consent::decide),
        )
        .fallback(|| async { AppError::NotFound })
        .layer(middleware::from_fn_with_state(state, csrf_guard))
}

async fn me(State(state): State<AppState>, user: CurrentUser) -> AppResult<Json<Value>> {
    let link = links::status(&state.db, user.id).await?;
    Ok(Json(json!({ "user": user, "link": link })))
}

/// `GET /api/now-playing`: the user's current playback, for the overview.
/// `null` when nothing is playing or the user has no usable link. Like a
/// tool call, a token Spotify rejects early is refreshed once.
async fn now_playing(State(state): State<AppState>, user: CurrentUser) -> AppResult<Json<Value>> {
    let Some(linked) = links::account(&state, user.id).await? else {
        return Ok(Json(Value::Null));
    };
    let mut token_hash = linked.token_hash;
    let mut outcome = linked.account.now_playing().await;
    if matches!(outcome, Err(spotify::Error::Unauthorized)) {
        let Some(fresh) = links::refresh(&state, user.id, &token_hash).await? else {
            return Ok(Json(Value::Null));
        };
        token_hash = fresh.token_hash;
        outcome = fresh.account.now_playing().await;
    }
    match outcome {
        Ok(playing) => {
            links::mark_ok(&state.db, user.id).await;
            Ok(Json(playing))
        }
        Err(err @ (spotify::Error::Unauthorized | spotify::Error::InvalidGrant)) => {
            links::mark_invalid(&state.db, user.id, &token_hash).await;
            Err(err.into())
        }
        Err(err) => Err(err.into()),
    }
}

/// CSRF defence alongside `SameSite=Lax` cookies: mutating requests must come
/// from our own origin (when the browser says) and carry JSON, which a
/// cross-site form cannot send.
async fn csrf_guard(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> AppResult<Response> {
    if !request.method().is_safe() {
        let has_body = !request.body().is_end_stream();
        let headers = request.headers();
        if headers
            .get(header::ORIGIN)
            .is_some_and(|origin| !origin.to_str().is_ok_and(|o| state.config.is_own_origin(o)))
        {
            return Err(AppError::Forbidden("cross-origin request rejected".into()));
        }
        if has_body && !is_json(headers) {
            return Err(AppError::BadRequest(
                "request body must be application/json".into(),
            ));
        }
    }
    Ok(next.run(request).await)
}

fn is_json(headers: &HeaderMap) -> bool {
    headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"))
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

    #[sqlx::test(migrations = "../../migrations")]
    async fn me_requires_a_session(db: PgPool) {
        let state = testing::state(db);
        let (user_id, cookie) = testing::sign_in(&state, 7).await;
        let app = crate::app(state);

        let response = send(
            &app,
            dashboard_request(Method::GET, "/api/me", &cookie, None),
        )
        .await;
        assert_eq!(
            response.json(),
            json!({
                "user": {
                    "id": user_id, "spotify_id": "user7", "display_name": null, "email": null,
                    "avatar_url": null, "product": null, "country": null,
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

    #[sqlx::test(migrations = "../../migrations")]
    async fn csrf_guard_rejects_cross_origin_and_non_json(db: PgPool) {
        let state = testing::state(db);
        let (user_id, cookie) = testing::sign_in(&state, 7).await;
        let session = testing::agent_session(&state, user_id).await;
        let app = crate::app(state);
        let uri = format!("/api/sessions/{session}");
        let body = json!({ "read_only": true });

        let mut cross_origin = dashboard_request(Method::PATCH, &uri, &cookie, Some(&body));
        cross_origin
            .headers_mut()
            .insert("origin", "https://evil.example".parse().unwrap());
        assert_eq!(send(&app, cross_origin).await.status, StatusCode::FORBIDDEN);

        let form = Request::patch(&uri)
            .header("cookie", &cookie)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(Body::from("read_only=true"))
            .unwrap();
        assert_eq!(send(&app, form).await.status, StatusCode::BAD_REQUEST);

        // Bodyless deletes and same-origin JSON pass.
        let ok = send(
            &app,
            dashboard_request(Method::PATCH, &uri, &cookie, Some(&body)),
        )
        .await;
        assert_eq!(ok.status, StatusCode::OK);
        let delete = send(&app, dashboard_request(Method::DELETE, &uri, &cookie, None)).await;
        assert_eq!(delete.status, StatusCode::NO_CONTENT);
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn me_shows_the_link_status(db: PgPool) {
        let spotify = testing::MockSpotify::start().await;
        let state = spotify.state(db.clone());
        let (user_id, cookie) = testing::sign_in(&state, 7).await;
        crate::links::store_linked(&state, user_id, &spotify.tokens())
            .await
            .unwrap();
        let app = crate::app(state);

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
            "UPDATE spotify_links SET status = 'needs_relink', last_error = 'revoked',
                    scopes = ARRAY['user-read-private']",
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

    #[sqlx::test(migrations = "../../migrations")]
    async fn now_playing(db: PgPool) {
        use std::sync::atomic::Ordering;

        let spotify = testing::MockSpotify::start().await;
        let state = spotify.state(db.clone());
        let (user_id, cookie) = testing::sign_in(&state, 7).await;
        let app = crate::app(state.clone());
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
        assert_eq!(failed.json()["error"], "spotify_error");
        let link = crate::links::status(&db, user_id).await.unwrap();
        assert_eq!(link.status, Some(crate::links::State::NeedsRelink));
        assert_eq!(now_playing().await.json(), json!(null));
    }
}
