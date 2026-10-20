//! The user's listening: top artists and tracks, recently played.

use serde_json::{Value, json};

use super::helpers::{paging, range};
use crate::{
    Account, Error, Result, compact,
    types::{RecentlyPlayedRequest, TopItemsRequest, TopKind},
};

impl Account {
    /// `get_top_items`.
    pub async fn get_top_items(&self, request: TopItemsRequest) -> Result<Value> {
        let (path, map): (&str, fn(&Value) -> Value) = match request.kind {
            TopKind::Artists => ("me/top/artists", compact::artist),
            TopKind::Tracks => ("me/top/tracks", compact::track),
        };
        let [limit, offset] = paging(request.limit, request.offset, 50)?;
        let raw = self
            .get(
                path,
                &[
                    ("time_range", request.time_range.map(|t| t.as_str().into())),
                    limit,
                    offset,
                ],
            )
            .await?;
        Ok(compact::page(&raw, map))
    }

    /// `get_recently_played`: `{items, next_before}`; each item is a track
    /// with `played_at` and the `context` it played from.
    pub async fn get_recently_played(&self, request: RecentlyPlayedRequest) -> Result<Value> {
        if request.after.is_some() && request.before.is_some() {
            return Err(Error::InvalidArgument(
                "give `after` or `before`, not both".into(),
            ));
        }
        let raw = self
            .get(
                "me/player/recently-played",
                &[
                    ("limit", range("limit", request.limit, 1, 50)?),
                    ("after", request.after.map(|a| a.to_string())),
                    ("before", request.before.map(|b| b.to_string())),
                ],
            )
            .await?;
        let items: Vec<Value> = raw["items"]
            .as_array()
            .map(|items| {
                items
                    .iter()
                    .map(|play| {
                        let mut track = compact::track(&play["track"]);
                        if let Value::Object(map) = &mut track {
                            map.insert("played_at".into(), play["played_at"].clone());
                            if let Some(uri) = play["context"]["uri"].as_str() {
                                map.insert("context".into(), uri.into());
                            }
                        }
                        track
                    })
                    .collect()
            })
            .unwrap_or_default();
        let next_before = if raw["next"].is_null() {
            Value::Null
        } else {
            raw["cursors"]["before"]
                .as_str()
                .and_then(|b| b.parse::<i64>().ok())
                .map_or(Value::Null, Value::from)
        };
        Ok(json!({ "items": items, "next_before": next_before }))
    }
}
