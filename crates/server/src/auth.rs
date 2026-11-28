//! "Continue with Spotify": the dashboard sign-in, which also links the
//! user's Spotify account (Authorization Code with PKCE).
//!
//! `start` records a sign-in attempt (the OAuth `state`, the sealed PKCE
//! verifier, `next`) bound to this browser by the `spmcp_link` pre-auth
//! cookie, and sends the browser to Spotify. `callback` consumes the
//! attempt, exchanges the code, upserts the user by Spotify ID, stores the
//! tokens and starts a dashboard session.

use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::{CookieJar, cookie::Cookie};
use chrono::TimeDelta;
use mcp_backbone::{
    AppResult, AppState,
    auth::{self, CurrentUser, Profile, attempts},
    crypto::random_token,
};
use serde::Deserialize;
use serde_json::json;
use uuid::Uuid;

use crate::{links, service::SpotifyService};

const COOKIE_PATH: &str = "/auth/spotify";
const ATTEMPT_TTL: TimeDelta = TimeDelta::minutes(10);

type State_ = AppState<SpotifyService>;

fn cookie_name(state: &State_) -> String {
    state.info.cookie_name("link")
}

#[derive(Deserialize)]
pub struct StartQuery {
    /// Same-origin path to return to; `/` by default.
    pub next: Option<String>,
}

/// `GET /auth/spotify/start?next=`: redirects to Spotify's consent screen.
pub async fn start(
    State(state): State<State_>,
    jar: CookieJar,
    signed_in: Option<CurrentUser>,
    Query(query): Query<StartQuery>,
) -> AppResult<Response> {
    let next = auth::local_path_or_root(query.next.as_deref());
    let oauth_state = random_token();
    let verifier = spotify::oauth::new_verifier();
    let attempt = attempts::start(
        &state,
        Some(&oauth_state),
        Some(verifier.as_bytes()),
        Some(&next),
        ATTEMPT_TTL,
    )
    .await?;

    // Already signed in: this is a re-link or an account switch, so let the
    // user see (and change) which Spotify account they grant.
    let authorize_url = state.service.spotify.authorize_url(
        &redirect_uri(&state),
        &oauth_state,
        &spotify::oauth::challenge(&verifier),
        signed_in.is_some(),
    );
    let cookie = auth::cookie(
        &state.config,
        cookie_name(&state),
        attempt.preauth,
        COOKIE_PATH,
        time::Duration::minutes(ATTEMPT_TTL.num_minutes()),
    );
    Ok((jar.add(cookie), Redirect::to(&authorize_url)).into_response())
}

#[derive(Deserialize)]
pub struct CallbackQuery {
    pub code: Option<String>,
    pub state: Option<String>,
    pub error: Option<String>,
}

/// Why a sign-in failed, as the `/login?error=` code.
#[derive(Clone, Copy)]
enum Failure {
    /// The user declined on Spotify's consent screen.
    Denied,
    /// The attempt is gone, or this browser did not start it.
    Expired,
    /// Spotify answered `/me` with 403: a development-mode app serves only
    /// the users on its allowlist. Also the server's own allowlist.
    NotAllowed,
    /// Spotify failed.
    Spotify,
}

impl Failure {
    fn redirect(self) -> Redirect {
        let code = match self {
            Self::Denied => "denied",
            Self::Expired => "expired",
            Self::NotAllowed => "not_allowed",
            Self::Spotify => "spotify",
        };
        Redirect::to(&format!("/login?error={code}"))
    }
}

/// `GET /auth/spotify/callback`: finishes the sign-in and redirects to `next`.
pub async fn callback(
    State(state): State<State_>,
    jar: CookieJar,
    Query(query): Query<CallbackQuery>,
) -> AppResult<Response> {
    let name = cookie_name(&state);
    let preauth = jar.get(&name).map(|cookie| cookie.value().to_owned());
    let jar = jar.remove(Cookie::build(name).path(COOKIE_PATH));

    // Consume the attempt first, so a state is single use however this ends.
    let attempt = match (&query.state, &preauth) {
        (Some(oauth_state), Some(preauth)) => {
            attempts::take_by_state(&state, oauth_state, preauth).await?
        }
        _ => None,
    };
    if let Some(error) = &query.error {
        let failure = if error == "access_denied" {
            Failure::Denied
        } else {
            tracing::warn!(error, "Spotify sign-in failed");
            Failure::Spotify
        };
        return Ok((jar, failure.redirect()).into_response());
    }
    let (Some(attempt), Some(code)) = (attempt, &query.code) else {
        return Ok((jar, Failure::Expired.redirect()).into_response());
    };

    let user_id = match sign_in(&state, &attempt, code).await {
        Ok(user_id) => user_id,
        Err(SignInError::NotAllowed) => {
            return Ok((jar, Failure::NotAllowed.redirect()).into_response());
        }
        Err(SignInError::Spotify(spotify::Error::Forbidden(message))) => {
            tracing::warn!(%message, "Spotify refused /me: not on the app's allowlist?");
            return Ok((jar, Failure::NotAllowed.redirect()).into_response());
        }
        Err(SignInError::Spotify(err)) => {
            tracing::warn!(error = %err, "Spotify sign-in failed");
            return Ok((jar, Failure::Spotify.redirect()).into_response());
        }
        Err(SignInError::Internal(err)) => return Err(err.into()),
    };
    // Spotify said who this is: replace whatever session the browser had.
    let session = auth::start_session(&state, user_id).await?;
    let next = auth::local_path_or_root(attempt.next.as_deref());
    Ok((jar.add(session), Redirect::to(&next)).into_response())
}

enum SignInError {
    NotAllowed,
    Spotify(spotify::Error),
    Internal(anyhow::Error),
}

fn internal(err: impl Into<anyhow::Error>) -> SignInError {
    SignInError::Internal(err.into())
}

/// Exchanges the code, loads the profile, upserts the user and stores the
/// tokens. Returns the user's ID. Profile fields `/me` leaves out keep their
/// stored values.
async fn sign_in(
    state: &State_,
    attempt: &attempts::Attempt,
    code: &str,
) -> Result<Uuid, SignInError> {
    let verifier = attempt
        .payload
        .clone()
        .ok_or_else(|| internal(anyhow::anyhow!("the attempt has no verifier")))?;
    let verifier = String::from_utf8(verifier).map_err(internal)?;
    let spotify = &state.service.spotify;
    let tokens = spotify
        .exchange_code(code, &redirect_uri(state), &verifier)
        .await
        .map_err(SignInError::Spotify)?;
    let me = spotify
        .account(&tokens.access_token)
        .profile()
        .await
        .map_err(SignInError::Spotify)?;

    let mut profile = Profile::new(me.id.clone());
    profile.login = Some(me.id.clone());
    profile.name = me.display_name.clone();
    profile.email = me.email.clone();
    profile.avatar_url = me.avatar_url.clone();
    let mut extra = serde_json::Map::new();
    if let Some(product) = &me.product {
        extra.insert("product".into(), json!(product));
    }
    if let Some(country) = &me.country {
        extra.insert("country".into(), json!(country));
    }
    profile.profile = extra.into();
    if !auth::is_allowed(&state.config, &profile) {
        return Err(SignInError::NotAllowed);
    }
    let user_id = auth::upsert_user(&state.db, &profile)
        .await
        .map_err(internal)?;
    links::store_linked(state, user_id, &tokens)
        .await
        .map_err(internal)?;
    tracing::info!(%user_id, spotify_id = %me.id, "signed in with Spotify");
    Ok(user_id)
}

fn redirect_uri(state: &State_) -> String {
    format!("{}/auth/spotify/callback", state.config.public_url)
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode, header},
    };
    use sqlx::PgPool;
    use url::{Url, form_urlencoded};

    use crate::testing::{MockSpotify, PUBLIC_URL, Response, send, set_cookie};

    fn get(uri: &str, cookie: Option<&str>) -> Request<Body> {
        let mut request = Request::get(uri);
        if let Some(cookie) = cookie {
            request = request.header(header::COOKIE, cookie);
        }
        request.body(Body::empty()).unwrap()
    }

    fn query_param(url: &str, name: &str) -> Option<String> {
        let url = Url::parse(&format!("{PUBLIC_URL}{url}"))
            .or_else(|_| Url::parse(url))
            .unwrap();
        url.query_pairs()
            .find(|(k, _)| k == name)
            .map(|(_, v)| v.into_owned())
    }

    fn encode(value: &str) -> String {
        form_urlencoded::byte_serialize(value.as_bytes()).collect()
    }

    /// `GET /auth/spotify/start`; returns the response, the `state` sent to
    /// Spotify and the pre-auth cookie.
    async fn start(
        app: &axum::Router,
        next: &str,
        session: Option<&str>,
    ) -> (Response, String, String) {
        let uri = format!("/auth/spotify/start?next={}", encode(next));
        let response = send(app, get(&uri, session)).await;
        assert_eq!(response.status, StatusCode::SEE_OTHER);
        let state = query_param(response.location(), "state").expect("state");
        let cookie = set_cookie(&response, "spmcp_link").expect("pre-auth cookie");
        let cookie = cookie.split(';').next().unwrap().to_owned();
        (response, state, cookie)
    }

    async fn callback(app: &axum::Router, query: &str, cookie: Option<&str>) -> Response {
        send(app, get(&format!("/auth/spotify/callback?{query}"), cookie)).await
    }

    async fn count(db: &PgPool, table: &str) -> i64 {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT count(*) FROM {table}")))
            .fetch_one(db)
            .await
            .unwrap()
    }

    #[sqlx::test(migrations = false)]
    async fn sign_in_links_the_account_and_lands_on_next(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let state = spotify.state(db.clone()).await;
        let app = mcp_backbone::router(state.clone());

        let (response, oauth_state, cookie) = start(&app, "/sessions?x=1", None).await;
        let location = response.location();
        assert!(
            location.starts_with(&format!("{}/authorize?", spotify.url)),
            "{location}"
        );
        let redirect_uri = format!("{PUBLIC_URL}/auth/spotify/callback");
        assert_eq!(
            query_param(location, "redirect_uri").as_deref(),
            Some(redirect_uri.as_str())
        );
        assert_eq!(
            query_param(location, "response_type").as_deref(),
            Some("code")
        );
        assert_eq!(query_param(location, "show_dialog"), None);
        let challenge = query_param(location, "code_challenge").unwrap();
        let set = set_cookie(&response, "spmcp_link").unwrap();
        assert!(set.contains("Path=/auth/spotify"), "{set}");
        assert!(
            set.contains("HttpOnly") && set.contains("Max-Age=600"),
            "{set}"
        );

        let response = callback(
            &app,
            &format!("code=abc&state={oauth_state}"),
            Some(&cookie),
        )
        .await;
        assert_eq!(response.status, StatusCode::SEE_OTHER);
        assert_eq!(response.location(), "/sessions?x=1");
        // The exchange used the attempt's verifier and our redirect URI.
        let exchange = spotify.mock.last_exchange.lock().unwrap().clone();
        assert_eq!(
            spotify::oauth::challenge(&exchange["code_verifier"]),
            challenge
        );
        assert_eq!(exchange["redirect_uri"], redirect_uri);
        assert_eq!(exchange["code"], "abc");
        // The pre-auth cookie is cleared, a session started.
        let cleared = set_cookie(&response, "spmcp_link").unwrap();
        assert!(cleared.contains("Max-Age=0"), "{cleared}");
        let session = set_cookie(&response, "spmcp_session").unwrap();
        let session = session.split(';').next().unwrap().to_owned();

        let me = send(&app, get("/api/me", Some(&session))).await.json();
        assert_eq!(me["user"]["subject"], "spotify-user");
        assert_eq!(me["user"]["name"], "User spotify-user");
        assert_eq!(me["user"]["email"], "spotify-user@example.test");
        assert_eq!(me["user"]["avatar_url"], "https://i.scdn.co/image/avatar");
        assert_eq!(me["user"]["profile"]["product"], "premium");
        assert_eq!(me["user"]["profile"]["country"], "TW");
        assert_eq!(me["link"]["linked"], true);
        assert_eq!(me["link"]["status"], "ok");
        assert_eq!(me["link"]["missing_scopes"], serde_json::json!([]));
        assert_eq!(count(&db, "link_attempts").await, 0);
        assert_eq!(count(&db, "links").await, 1);

        // Signed in already: Spotify is asked to show its dialog.
        let (response, ..) = start(&app, "/", Some(&session)).await;
        assert_eq!(
            query_param(response.location(), "show_dialog").as_deref(),
            Some("true")
        );
    }

    #[sqlx::test(migrations = false)]
    async fn next_is_sanitized(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let app = mcp_backbone::router(spotify.state(db).await);
        for (next, lands) in [
            ("//evil.example/x", "/"),
            ("https://evil.example", "/"),
            ("/\\evil.example", "/"),
            ("/activity", "/"),
        ] {
            let (_, oauth_state, cookie) = if next == "/activity" {
                // No `next` at all defaults to `/`.
                let response = send(&app, get("/auth/spotify/start", None)).await;
                let state = query_param(response.location(), "state").unwrap();
                let cookie = set_cookie(&response, "spmcp_link").unwrap();
                (
                    response,
                    state,
                    cookie.split(';').next().unwrap().to_owned(),
                )
            } else {
                start(&app, next, None).await
            };
            let response =
                callback(&app, &format!("code=c&state={oauth_state}"), Some(&cookie)).await;
            assert_eq!(response.location(), lands, "{next}");
        }
    }

    #[sqlx::test(migrations = false)]
    async fn attempts_are_bound_to_the_browser_and_single_use(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let app = mcp_backbone::router(spotify.state(db.clone()).await);
        let (_, oauth_state, cookie) = start(&app, "/", None).await;
        let (_, _, other_cookie) = start(&app, "/", None).await;
        let query = format!("code=c&state={oauth_state}");

        let missing = callback(&app, &query, None).await;
        assert_eq!(missing.location(), "/login?error=expired");
        let wrong = callback(&app, &query, Some(&other_cookie)).await;
        assert_eq!(wrong.location(), "/login?error=expired");
        let unknown = callback(&app, "code=c&state=nope", Some(&cookie)).await;
        assert_eq!(unknown.location(), "/login?error=expired");
        assert_eq!(
            spotify
                .mock
                .exchanges
                .load(std::sync::atomic::Ordering::SeqCst),
            0
        );

        // The right browser still gets through, once.
        let ok = callback(&app, &query, Some(&cookie)).await;
        assert_eq!(ok.location(), "/");
        let reused = callback(&app, &query, Some(&cookie)).await;
        assert_eq!(reused.location(), "/login?error=expired");
        assert!(set_cookie(&reused, "spmcp_session").is_none());

        // An expired attempt is gone too.
        let (_, oauth_state, cookie) = start(&app, "/", None).await;
        sqlx::query("UPDATE link_attempts SET expires_at = now() - interval '1 second'")
            .execute(&db)
            .await
            .unwrap();
        let expired = callback(&app, &format!("code=c&state={oauth_state}"), Some(&cookie)).await;
        assert_eq!(expired.location(), "/login?error=expired");
    }

    #[sqlx::test(migrations = false)]
    async fn declining_and_upstream_failures(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let app = mcp_backbone::router(spotify.state(db.clone()).await);

        let (_, oauth_state, cookie) = start(&app, "/", None).await;
        let denied = callback(
            &app,
            &format!("error=access_denied&state={oauth_state}"),
            Some(&cookie),
        )
        .await;
        assert_eq!(denied.status, StatusCode::SEE_OTHER);
        assert_eq!(denied.location(), "/login?error=denied");
        assert!(set_cookie(&denied, "spmcp_session").is_none());
        assert_eq!(
            count(&db, "link_attempts").await,
            0,
            "the attempt is consumed"
        );

        let (_, oauth_state, cookie) = start(&app, "/", None).await;
        spotify
            .mock
            .invalid_grant
            .store(true, std::sync::atomic::Ordering::SeqCst);
        let failed = callback(&app, &format!("code=c&state={oauth_state}"), Some(&cookie)).await;
        assert_eq!(failed.location(), "/login?error=spotify");
        assert_eq!(count(&db, "users").await, 0);
    }

    #[sqlx::test(migrations = false)]
    async fn users_off_the_allowlist_are_not_allowed(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let app = mcp_backbone::router(spotify.state(db.clone()).await);
        spotify
            .mock
            .forbid_me
            .store(true, std::sync::atomic::Ordering::SeqCst);

        let (_, oauth_state, cookie) = start(&app, "/", None).await;
        let refused = callback(&app, &format!("code=c&state={oauth_state}"), Some(&cookie)).await;
        assert_eq!(refused.status, StatusCode::SEE_OTHER);
        assert_eq!(refused.location(), "/login?error=not_allowed");
        assert!(set_cookie(&refused, "spmcp_session").is_none());
        assert_eq!(count(&db, "users").await, 0);
        assert_eq!(count(&db, "links").await, 0);
    }

    #[sqlx::test(migrations = false)]
    async fn signing_in_again_updates_the_same_user(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let app = mcp_backbone::router(spotify.state(db.clone()).await);
        let sign_in = || async {
            let (_, oauth_state, cookie) = start(&app, "/", None).await;
            let response =
                callback(&app, &format!("code=c&state={oauth_state}"), Some(&cookie)).await;
            assert_eq!(response.location(), "/");
        };
        sign_in().await;
        sqlx::query("UPDATE users SET name = 'old', last_login_at = now() - interval '1 day'")
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("UPDATE links SET status = 'needs_relink', last_error = 'x'")
            .execute(&db)
            .await
            .unwrap();
        // Development-mode apps get no email, country or product: the
        // stored ones stay.
        sqlx::query(
            "UPDATE users SET email = 'kept@example.test',
                              profile = profile || '{\"product\": \"free\"}'",
        )
        .execute(&db)
        .await
        .unwrap();
        spotify
            .mock
            .minimal_profile
            .store(true, std::sync::atomic::Ordering::SeqCst);
        sign_in().await;

        let (email, product, country): (Option<String>, Option<String>, Option<String>) =
            sqlx::query_as("SELECT email, profile->>'product', profile->>'country' FROM users")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(email.as_deref(), Some("kept@example.test"));
        assert_eq!(product.as_deref(), Some("free"));
        assert_eq!(country.as_deref(), Some("TW"));
        assert_eq!(count(&db, "users").await, 1);
        assert_eq!(count(&db, "links").await, 1);
        assert_eq!(count(&db, "dashboard_sessions").await, 2);
        let (name, recent): (String, bool) =
            sqlx::query_as("SELECT name, last_login_at > now() - interval '1 minute' FROM users")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!(name, "User spotify-user");
        assert!(recent);
        let (status, error): (String, Option<String>) =
            sqlx::query_as("SELECT status, last_error FROM links")
                .fetch_one(&db)
                .await
                .unwrap();
        assert_eq!((status.as_str(), error), ("ok", None));

        // Another Spotify account is another user.
        *spotify.mock.user_id.lock().unwrap() = "someone-else".into();
        sign_in().await;
        assert_eq!(count(&db, "users").await, 2);
    }

    /// An MCP client's authorization request while signed out goes through
    /// the login page and Spotify, and comes back to the consent page.
    #[sqlx::test(migrations = false)]
    async fn oauth_authorize_round_trips_through_sign_in(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let state = spotify.state(db.clone()).await;
        sqlx::query(
            "INSERT INTO oauth_clients (client_id, client_name, redirect_uris, grant_types, source)
             VALUES ('agent', 'Agent', ARRAY['https://agent.example/cb'],
                     ARRAY['authorization_code'], 'dcr')",
        )
        .execute(&db)
        .await
        .unwrap();
        let app = mcp_backbone::router(state);
        let authorize = "/oauth/authorize?response_type=code&client_id=agent\
            &redirect_uri=https%3A%2F%2Fagent.example%2Fcb\
            &code_challenge=E9Melhoa2OwvFrEMTJguCHaoeK1t8URWbuGJSstw-cM\
            &code_challenge_method=S256&state=xyz";

        let signed_out = send(&app, get(authorize, None)).await;
        assert!(signed_out.status.is_redirection());
        let login = signed_out.location().to_owned();
        assert!(login.starts_with("/login?next="), "{login}");
        let next = query_param(&login, "next").unwrap();
        assert_eq!(next, authorize);

        // The login page's button: /auth/spotify/start?next=<next>.
        let (_, oauth_state, cookie) = start(&app, &next, None).await;
        let back = callback(&app, &format!("code=c&state={oauth_state}"), Some(&cookie)).await;
        assert_eq!(back.location(), authorize);
        let session = set_cookie(&back, "spmcp_session").unwrap();
        let session = session.split(';').next().unwrap().to_owned();

        let signed_in = send(&app, get(back.location(), Some(&session))).await;
        assert!(signed_in.status.is_redirection());
        let consent = signed_in.location();
        let id = consent.strip_prefix("/consent/").expect(consent);
        let request = send(
            &app,
            get(&format!("/api/oauth/requests/{id}"), Some(&session)),
        )
        .await;
        assert_eq!(request.status, StatusCode::OK);
        assert_eq!(request.json()["client_id"], "agent");
    }
}
