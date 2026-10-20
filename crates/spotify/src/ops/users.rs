//! The user's profile and other users' public profiles.

use serde::Serialize;
use serde_json::{Value, json};

use crate::{Account, Error, Result, compact, types::NoParams};

/// The signed-in user's profile, as stored for the dashboard.
#[derive(Debug, Clone, Serialize)]
pub struct Profile {
    pub id: String,
    pub display_name: Option<String>,
    pub email: Option<String>,
    pub avatar_url: Option<String>,
    pub product: Option<String>,
    pub country: Option<String>,
}

impl Account {
    /// `GET /v1/me`, for sign-in.
    pub async fn profile(&self) -> Result<Profile> {
        let me = self.get("me", &[]).await?;
        let text = |key: &str| me[key].as_str().map(str::to_owned);
        Ok(Profile {
            id: text("id").ok_or_else(|| Error::Decode("/me has no id".into()))?,
            display_name: text("display_name"),
            email: text("email"),
            avatar_url: compact::image(&me).as_str().map(str::to_owned),
            product: text("product"),
            country: text("country"),
        })
    }

    /// `get_me`.
    pub async fn get_me(&self, _: NoParams) -> Result<Value> {
        let me = self.get("me", &[]).await?;
        Ok(json!({
            "uri": me["uri"],
            "id": me["id"],
            "name": me["display_name"],
            "email": me["email"],
            "country": me["country"],
            "product": me["product"],
            "followers": me["followers"]["total"],
            "explicit_content_filtered": me["explicit_content"]["filter_enabled"],
            "image": compact::image(&me),
        }))
    }
}
