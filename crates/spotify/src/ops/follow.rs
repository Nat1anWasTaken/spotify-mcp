//! Followed artists. Following and unfollowing go through the library
//! tools (see [`crate::ops::library`]).

use serde_json::Value;

use super::helpers::range;
use crate::{Account, Result, compact, types::FollowedArtistsRequest};

impl Account {
    /// `get_followed_artists`: `{items, total, next_after}`.
    pub async fn get_followed_artists(&self, request: FollowedArtistsRequest) -> Result<Value> {
        let raw = self
            .get(
                "me/following",
                &[
                    ("type", Some("artist".into())),
                    ("after", request.after),
                    ("limit", range("limit", request.limit, 1, 50)?),
                ],
            )
            .await?;
        Ok(compact::cursor_page(&raw["artists"], compact::artist))
    }
}
