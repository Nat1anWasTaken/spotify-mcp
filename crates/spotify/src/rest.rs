//! Web API requests and the mapping of Spotify's errors to [`crate::Error`].

use std::time::Duration;

use reqwest::{Method, StatusCode, header};
use serde::Deserialize;
use serde_json::Value;

use crate::{Account, Error, Result};

/// The longest `Retry-After` that is waited out in place (once) instead of
/// being returned as [`Error::RateLimited`].
const MAX_INLINE_RETRY: Duration = Duration::from_secs(5);

/// Query parameters; `None` values are left out.
pub(crate) type Query<'a> = &'a [(&'a str, Option<String>)];

/// A request body.
#[derive(Clone)]
pub(crate) enum Body {
    None,
    Json(Value),
    /// Base64 text of a JPEG, sent as `image/jpeg` (cover uploads).
    Jpeg(String),
}

impl Account {
    /// `GET /v1/{path}`.
    pub(crate) async fn get(&self, path: &str, query: Query<'_>) -> Result<Value> {
        self.send(Method::GET, path, query, Body::None).await
    }

    /// `PUT /v1/{path}` with a JSON body.
    pub(crate) async fn put(&self, path: &str, query: Query<'_>, body: Body) -> Result<Value> {
        self.send(Method::PUT, path, query, body).await
    }

    /// `POST /v1/{path}`.
    pub(crate) async fn post(&self, path: &str, query: Query<'_>, body: Body) -> Result<Value> {
        self.send(Method::POST, path, query, body).await
    }

    /// `DELETE /v1/{path}`.
    pub(crate) async fn delete(&self, path: &str, query: Query<'_>, body: Body) -> Result<Value> {
        self.send(Method::DELETE, path, query, body).await
    }

    /// Sends a Web API request with the user's token. Returns the JSON body,
    /// or `Value::Null` for 204 and empty bodies. A 429 with a short
    /// `Retry-After` is retried once.
    pub(crate) async fn send(
        &self,
        method: Method,
        path: &str,
        query: Query<'_>,
        body: Body,
    ) -> Result<Value> {
        match self
            .send_once(method.clone(), path, query, body.clone())
            .await
        {
            Err(Error::RateLimited { retry_after }) if retry_after <= MAX_INLINE_RETRY => {
                tracing::debug!(?retry_after, path, "rate limited, retrying");
                tokio::time::sleep(retry_after).await;
                self.send_once(method, path, query, body).await
            }
            other => other,
        }
    }

    async fn send_once(
        &self,
        method: Method,
        path: &str,
        query: Query<'_>,
        body: Body,
    ) -> Result<Value> {
        let inner = &self.spotify.inner;
        let url = format!("{}/v1/{}", inner.api_url, path.trim_start_matches('/'));
        let query: Vec<(&str, &str)> = query
            .iter()
            .filter_map(|(k, v)| v.as_deref().map(|v| (*k, v)))
            .collect();
        let is_get = method == Method::GET;
        let mut request = inner
            .http
            .request(method, url)
            .bearer_auth(&*self.token)
            .query(&query);
        request = match body {
            // Spotify answers 411 to a body-less PUT/POST without a length.
            Body::None if is_get => request,
            Body::None => request.header(header::CONTENT_LENGTH, "0"),
            Body::Json(json) => request.json(&json),
            Body::Jpeg(base64) => request
                .header(header::CONTENT_TYPE, "image/jpeg")
                .body(base64),
        };
        let response = request.send().await.map_err(http)?;
        let status = response.status();
        let retry_after = response
            .headers()
            .get(header::RETRY_AFTER)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.trim().parse::<u64>().ok())
            .map(Duration::from_secs);
        let text = response.text().await.map_err(http)?;
        if !status.is_success() {
            return Err(map_error(status, retry_after, &text));
        }
        if status == StatusCode::NO_CONTENT || text.trim().is_empty() {
            return Ok(Value::Null);
        }
        match serde_json::from_str(&text) {
            Ok(value) => Ok(value),
            // Some write endpoints answer with a bare snapshot ID.
            Err(_) if !text.trim_start().starts_with(['{', '[']) => {
                Ok(Value::String(text.trim().to_owned()))
            }
            Err(e) => Err(Error::Decode(format!("{path}: {e}"))),
        }
    }
}

/// Drops the URL (and with it any query) from a transport error.
fn http(error: reqwest::Error) -> Error {
    Error::Http(error.without_url())
}

/// Maps a failed Web API response. Spotify's error body is
/// `{"error": {"status", "message", "reason"?}}`; the player adds `reason`.
pub(crate) fn map_error(status: StatusCode, retry_after: Option<Duration>, body: &str) -> Error {
    #[derive(Deserialize)]
    struct Envelope {
        error: Detail,
    }
    #[derive(Deserialize)]
    struct Detail {
        #[serde(default)]
        message: String,
        reason: Option<String>,
    }
    let (message, reason) = match serde_json::from_str::<Envelope>(body) {
        Ok(Envelope { error }) => (error.message, error.reason),
        Err(_) => (String::new(), None),
    };
    let message = if message.is_empty() {
        status
            .canonical_reason()
            .unwrap_or("request failed")
            .to_owned()
    } else {
        message
    };
    let reason = reason.as_deref().unwrap_or_default();
    match status.as_u16() {
        401 => Error::Unauthorized,
        429 => Error::RateLimited {
            retry_after: retry_after.unwrap_or(Duration::from_secs(1)),
        },
        403 if reason == "PREMIUM_REQUIRED" || message.contains("Premium required") => {
            Error::PremiumRequired
        }
        404 if reason == "NO_ACTIVE_DEVICE" || message.contains("No active device") => {
            Error::NoActiveDevice
        }
        404 => Error::NotFound(message),
        403 => Error::Forbidden(message),
        400 => Error::InvalidArgument(message),
        status => Error::Api { status, message },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn map(status: u16, body: &str) -> Error {
        map_error(StatusCode::from_u16(status).unwrap(), None, body)
    }

    #[test]
    fn maps_statuses() {
        assert!(matches!(
            map(
                401,
                r#"{"error":{"status":401,"message":"The access token expired"}}"#
            ),
            Error::Unauthorized
        ));
        assert!(
            matches!(map(400, r#"{"error":{"status":400,"message":"Invalid limit"}}"#), Error::InvalidArgument(m) if m == "Invalid limit")
        );
        assert!(matches!(
            map(
                404,
                r#"{"error":{"status":404,"message":"Resource not found"}}"#
            ),
            Error::NotFound(_)
        ));
        assert!(
            matches!(map(403, r#"{"error":{"status":403,"message":"Insufficient client scope"}}"#), Error::Forbidden(m) if m.contains("scope"))
        );
        assert!(
            matches!(map(500, "oops"), Error::Api { status: 500, message } if message == "Internal Server Error")
        );
    }

    #[test]
    fn maps_player_reasons() {
        assert!(matches!(
            map(
                403,
                r#"{"error":{"status":403,"message":"Player command failed: Premium required","reason":"PREMIUM_REQUIRED"}}"#
            ),
            Error::PremiumRequired
        ));
        assert!(matches!(
            map(
                404,
                r#"{"error":{"status":404,"message":"Player command failed: No active device found","reason":"NO_ACTIVE_DEVICE"}}"#
            ),
            Error::NoActiveDevice
        ));
        assert!(matches!(
            map(
                404,
                r#"{"error":{"status":404,"message":"Player command failed: No active device found"}}"#
            ),
            Error::NoActiveDevice
        ));
        assert!(matches!(
            map(
                403,
                r#"{"error":{"status":403,"message":"Player command failed: Restriction violated","reason":"UNKNOWN"}}"#
            ),
            Error::Forbidden(_)
        ));
    }

    #[test]
    fn maps_retry_after() {
        let e = map_error(
            StatusCode::TOO_MANY_REQUESTS,
            Some(Duration::from_secs(30)),
            "",
        );
        assert!(matches!(e, Error::RateLimited { retry_after } if retry_after.as_secs() == 30));
        assert!(e.to_string().contains("30 seconds"));
    }
}
