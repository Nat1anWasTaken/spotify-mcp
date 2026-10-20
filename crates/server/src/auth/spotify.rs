//! "Continue with Spotify": the dashboard sign-in, which also links the
//! user's Spotify account (Authorization Code with PKCE).
//!
//! `start` records a `link_attempts` row (hashed `state`, sealed PKCE
//! verifier, `next`) bound to this browser by the `spmcp_link` pre-auth
//! cookie, and sends the browser to Spotify. `callback` consumes the attempt,
//! exchanges the code, upserts the user by Spotify ID, stores the tokens and
//! starts a dashboard session.

use axum::{
    extract::{Query, State},
    response::{IntoResponse, Redirect, Response},
};
use axum_extra::extract::{CookieJar, cookie::Cookie};
use chrono::{TimeDelta, Utc};
use serde::Deserialize;
use sqlx::FromRow;
use uuid::Uuid;

use super::{CurrentUser, session};
use crate::{
    crypto::{hash_token, random_token, sealed},
    error::AppResult,
    links,
    state::AppState,
};

const COOKIE: &str = "spmcp_link";
const COOKIE_PATH: &str = "/auth/spotify";
const ATTEMPT_TTL: TimeDelta = TimeDelta::minutes(10);
const MAX_NEXT_CHARS: usize = 4096;

#[derive(Deserialize)]
pub struct StartQuery {
    /// Same-origin path to return to; `/` by default.
    pub next: Option<String>,
}

/// `GET /auth/spotify/start?next=`: redirects to Spotify's consent screen.
pub async fn start(
    State(state): State<AppState>,
    jar: CookieJar,
    signed_in: Option<CurrentUser>,
    Query(query): Query<StartQuery>,
) -> AppResult<Response> {
    let next = query
        .next
        .filter(|next| is_local_path(next))
        .unwrap_or_else(|| "/".to_owned());
    let oauth_state = random_token();
    let preauth = random_token();
    let verifier = spotify::oauth::new_verifier();
    let attempt_id = Uuid::new_v4();
    let verifier_sealed = sealed::seal(
        &state.config.encryption_key,
        attempt_id.as_bytes(),
        verifier.as_bytes(),
    )?;
    sqlx::query(
        "INSERT INTO link_attempts (id, state_hash, preauth_hash, verifier_sealed, next, expires_at)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(attempt_id)
    .bind(hash_token(&oauth_state))
    .bind(hash_token(&preauth))
    .bind(verifier_sealed)
    .bind(&next)
    .bind(Utc::now() + ATTEMPT_TTL)
    .execute(&state.db)
    .await?;

    // Already signed in: this is a re-link or an account switch, so let the
    // user see (and change) which Spotify account they grant.
    let authorize_url = state.spotify.authorize_url(
        &redirect_uri(&state),
        &oauth_state,
        &spotify::oauth::challenge(&verifier),
        signed_in.is_some(),
    );
    let cookie = super::cookie(
        &state.config,
        COOKIE,
        preauth,
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

#[derive(FromRow)]
struct Attempt {
    id: Uuid,
    verifier_sealed: Vec<u8>,
    next: String,
}

/// Why a sign-in failed, as the `/login?error=` code.
#[derive(Clone, Copy)]
enum Failure {
    /// The user declined on Spotify's consent screen.
    Denied,
    /// The attempt is gone, or this browser did not start it.
    Expired,
    /// Spotify failed.
    Spotify,
}

impl Failure {
    fn redirect(self) -> Redirect {
        let code = match self {
            Self::Denied => "denied",
            Self::Expired => "expired",
            Self::Spotify => "spotify",
        };
        Redirect::to(&format!("/login?error={code}"))
    }
}

/// `GET /auth/spotify/callback`: finishes the sign-in and redirects to `next`.
pub async fn callback(
    State(state): State<AppState>,
    jar: CookieJar,
    Query(query): Query<CallbackQuery>,
) -> AppResult<Response> {
    let preauth = jar.get(COOKIE).map(|cookie| cookie.value().to_owned());
    let jar = jar.remove(Cookie::build(COOKIE).path(COOKIE_PATH));

    // Consume the attempt first, so a state is single use however this ends.
    let attempt = match (&query.state, &preauth) {
        (Some(oauth_state), Some(preauth)) => take_attempt(&state, oauth_state, preauth).await?,
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
        Err(SignInError::Spotify(err)) => {
            tracing::warn!(error = %err, "Spotify sign-in failed");
            return Ok((jar, Failure::Spotify.redirect()).into_response());
        }
        Err(SignInError::Internal(err)) => return Err(err.into()),
    };
    // Spotify said who this is: replace whatever session the browser had.
    let session = session::start(&state.db, &state.config, user_id).await?;
    let next = Some(attempt.next).filter(|next| is_local_path(next));
    let next = next.as_deref().unwrap_or("/");
    Ok((jar.add(session), Redirect::to(next)).into_response())
}

/// Deletes and returns the live attempt `oauth_state` names, if this browser
/// started it.
async fn take_attempt(
    state: &AppState,
    oauth_state: &str,
    preauth: &str,
) -> sqlx::Result<Option<Attempt>> {
    sqlx::query_as(
        "DELETE FROM link_attempts
         WHERE state_hash = $1 AND preauth_hash = $2 AND expires_at > now()
         RETURNING id, verifier_sealed, next",
    )
    .bind(hash_token(oauth_state))
    .bind(hash_token(preauth))
    .fetch_optional(&state.db)
    .await
}

enum SignInError {
    Spotify(spotify::Error),
    Internal(anyhow::Error),
}

fn internal(err: impl Into<anyhow::Error>) -> SignInError {
    SignInError::Internal(err.into())
}

/// Exchanges the code, loads the profile, upserts the user and stores the
/// tokens. Returns the user's ID.
async fn sign_in(state: &AppState, attempt: &Attempt, code: &str) -> Result<Uuid, SignInError> {
    let verifier = sealed::open(
        &state.config.encryption_key,
        attempt.id.as_bytes(),
        &attempt.verifier_sealed,
    )
    .map_err(internal)?;
    let verifier = String::from_utf8(verifier).map_err(internal)?;
    let tokens = state
        .spotify
        .exchange_code(code, &redirect_uri(state), &verifier)
        .await
        .map_err(SignInError::Spotify)?;
    let profile = state
        .spotify
        .account(&tokens.access_token)
        .profile()
        .await
        .map_err(SignInError::Spotify)?;

    let user_id: Uuid = sqlx::query_scalar(
        "INSERT INTO users (spotify_id, display_name, email, avatar_url, product, country)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (spotify_id) DO UPDATE SET
             display_name = EXCLUDED.display_name,
             email = EXCLUDED.email,
             avatar_url = EXCLUDED.avatar_url,
             product = EXCLUDED.product,
             country = EXCLUDED.country,
             last_login_at = now()
         RETURNING id",
    )
    .bind(&profile.id)
    .bind(&profile.display_name)
    .bind(&profile.email)
    .bind(&profile.avatar_url)
    .bind(&profile.product)
    .bind(&profile.country)
    .fetch_one(&state.db)
    .await
    .map_err(internal)?;
    links::store_linked(state, user_id, &tokens)
        .await
        .map_err(internal)?;
    tracing::info!(%user_id, spotify_id = %profile.id, "signed in with Spotify");
    Ok(user_id)
}

fn redirect_uri(state: &AppState) -> String {
    format!("{}/auth/spotify/callback", state.config.public_url)
}

/// Whether `next` is a path on this origin, so redirecting to it after sign-in
/// cannot send the browser elsewhere. Browsers treat `\` like `/` and drop
/// tabs and newlines, so `/\host` and `/\t/host` would leave the site.
fn is_local_path(next: &str) -> bool {
    next.starts_with('/')
        && !next.starts_with("//")
        && !next.contains('\\')
        && !next.chars().any(char::is_control)
        && next.len() <= MAX_NEXT_CHARS
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        http::{Request, StatusCode, header},
    };
    use sqlx::PgPool;
    use url::{Url, form_urlencoded};

    use super::is_local_path;
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

    #[test]
    fn next_must_stay_on_this_origin() {
        for ok in ["/", "/sessions/1", "/oauth/authorize?client_id=x&state=y"] {
            assert!(is_local_path(ok), "{ok}");
        }
        for bad in [
            "",
            "sessions",
            "https://evil.example",
            "//evil.example",
            "/\\evil.example",
            "/a\\b",
            "/\t/evil.example",
            "/a\nb",
            "javascript:alert(1)",
        ] {
            assert!(!is_local_path(bad), "{bad:?}");
        }
        assert!(!is_local_path(&format!("/{}", "a".repeat(5000))));
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn sign_in_links_the_account_and_lands_on_next(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let state = spotify.state(db.clone());
        let app = crate::app(state.clone());

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
        assert_eq!(me["user"]["spotify_id"], "spotify-user");
        assert_eq!(me["user"]["display_name"], "User spotify-user");
        assert_eq!(me["user"]["email"], "spotify-user@example.test");
        assert_eq!(me["user"]["avatar_url"], "https://i.scdn.co/image/avatar");
        assert_eq!(me["user"]["product"], "premium");
        assert_eq!(me["user"]["country"], "TW");
        assert_eq!(me["link"]["linked"], true);
        assert_eq!(me["link"]["status"], "ok");
        assert_eq!(me["link"]["missing_scopes"], serde_json::json!([]));
        assert_eq!(count(&db, "link_attempts").await, 0);
        assert_eq!(count(&db, "spotify_links").await, 1);

        // Signed in already: Spotify is asked to show its dialog.
        let (response, ..) = start(&app, "/", Some(&session)).await;
        assert_eq!(
            query_param(response.location(), "show_dialog").as_deref(),
            Some("true")
        );
    }

    #[sqlx::test(migrations = "../../migrations")]
    async fn next_is_sanitized(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let app = crate::app(spotify.state(db));
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

    #[sqlx::test(migrations = "../../migrations")]
    async fn attempts_are_bound_to_the_browser_and_single_use(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let app = crate::app(spotify.state(db.clone()));
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

    #[sqlx::test(migrations = "../../migrations")]
    async fn declining_and_upstream_failures(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let app = crate::app(spotify.state(db.clone()));

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

    #[sqlx::test(migrations = "../../migrations")]
    async fn signing_in_again_updates_the_same_user(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let app = crate::app(spotify.state(db.clone()));
        let sign_in = || async {
            let (_, oauth_state, cookie) = start(&app, "/", None).await;
            let response =
                callback(&app, &format!("code=c&state={oauth_state}"), Some(&cookie)).await;
            assert_eq!(response.location(), "/");
        };
        sign_in().await;
        sqlx::query(
            "UPDATE users SET display_name = 'old', last_login_at = now() - interval '1 day'",
        )
        .execute(&db)
        .await
        .unwrap();
        sqlx::query("UPDATE spotify_links SET status = 'needs_relink', last_error = 'x'")
            .execute(&db)
            .await
            .unwrap();
        sign_in().await;

        assert_eq!(count(&db, "users").await, 1);
        assert_eq!(count(&db, "spotify_links").await, 1);
        assert_eq!(count(&db, "dashboard_sessions").await, 2);
        let (name, recent): (String, bool) = sqlx::query_as(
            "SELECT display_name, last_login_at > now() - interval '1 minute' FROM users",
        )
        .fetch_one(&db)
        .await
        .unwrap();
        assert_eq!(name, "User spotify-user");
        assert!(recent);
        let (status, error): (String, Option<String>) =
            sqlx::query_as("SELECT status, last_error FROM spotify_links")
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
    #[sqlx::test(migrations = "../../migrations")]
    async fn oauth_authorize_round_trips_through_sign_in(db: PgPool) {
        let spotify = MockSpotify::start().await;
        let state = spotify.state(db.clone());
        sqlx::query(
            "INSERT INTO oauth_clients (client_id, client_name, redirect_uris, grant_types, source)
             VALUES ('agent', 'Agent', ARRAY['https://agent.example/cb'],
                     ARRAY['authorization_code'], 'dcr')",
        )
        .execute(&db)
        .await
        .unwrap();
        let app = crate::app(state);
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
