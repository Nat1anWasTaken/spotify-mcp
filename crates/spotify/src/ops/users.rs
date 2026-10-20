use serde::Serialize;
use serde_json::Value;

use crate::{Account, Result, types::NoParams};

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
        todo!("crates/spotify agent")
    }

    pub async fn get_me(&self, _: NoParams) -> Result<Value> {
        todo!("crates/spotify agent")
    }
}
