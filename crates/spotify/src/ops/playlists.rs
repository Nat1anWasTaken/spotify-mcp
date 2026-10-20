//! Playlists: listing, reading and editing the user's own, and covers.
//! Since February 2026 a playlist's items can only be read for playlists the
//! user owns or collaborates on.

use base64::{Engine, engine::general_purpose::STANDARD};
use serde_json::{Map, Value, json};

use super::helpers::{count, market, paging};
use crate::{
    Account, Error, Result, compact, ids,
    rest::Body,
    types::{
        AddItemsRequest, CreatePlaylistRequest, GetPlaylistRequest, PageRequest,
        PlaylistItemsRequest, PlaylistRequest, RemoveItemsRequest, ReorderItemsRequest,
        ReplaceItemsRequest, UpdatePlaylistRequest, UploadCoverRequest,
    },
};

/// The most items per add, remove or replace.
const MAX_ITEMS: usize = 100;
/// The largest cover upload Spotify accepts (base64 text).
const MAX_COVER_BYTES: usize = 256 * 1024;
/// Playlist items may be episodes as well as tracks.
fn additional_types() -> (&'static str, Option<String>) {
    ("additional_types", Some("track,episode".into()))
}

impl Account {
    /// `list_my_playlists`: playlists the user owns or follows.
    pub async fn list_my_playlists(&self, request: PageRequest) -> Result<Value> {
        let [limit, offset] = paging(request.limit, request.offset, 50)?;
        let raw = self.get("me/playlists", &[limit, offset]).await?;
        Ok(compact::page(&raw, compact::playlist))
    }

    /// `get_playlist`: details, plus the first page of items when the user
    /// owns or collaborates on it.
    pub async fn get_playlist(&self, request: GetPlaylistRequest) -> Result<Value> {
        let id = ids::id(&request.playlist_id, "playlist")?;
        let raw = self
            .get(
                &format!("playlists/{id}"),
                &[
                    ("market", market(request.market.as_deref())?),
                    additional_types(),
                ],
            )
            .await?;
        Ok(compact::playlist(&raw))
    }

    /// `get_playlist_items`.
    pub async fn get_playlist_items(&self, request: PlaylistItemsRequest) -> Result<Value> {
        let id = ids::id(&request.playlist_id, "playlist")?;
        let [limit, offset] = paging(request.limit, request.offset, 50)?;
        let raw = self
            .get(
                &format!("playlists/{id}/items"),
                &[
                    ("market", market(request.market.as_deref())?),
                    limit,
                    offset,
                    additional_types(),
                ],
            )
            .await
            .map_err(|e| match e {
                Error::Forbidden(_) => Error::Forbidden(
                    "Spotify only shows the items of playlists the user owns or collaborates on."
                        .into(),
                ),
                e => e,
            })?;
        Ok(compact::page(&raw, compact::playlist_item))
    }

    /// `get_playlist_cover`: `[{url, width, height}]`, largest first.
    pub async fn get_playlist_cover(&self, request: PlaylistRequest) -> Result<Value> {
        let id = ids::id(&request.playlist_id, "playlist")?;
        let raw = self.get(&format!("playlists/{id}/images"), &[]).await?;
        Ok(raw
            .as_array()
            .map(|images| {
                images
                    .iter()
                    .map(|i| json!({ "url": i["url"], "width": i["width"], "height": i["height"] }))
                    .collect()
            })
            .unwrap_or_else(|| Value::Array(Vec::new())))
    }

    /// `create_playlist`: the new playlist.
    pub async fn create_playlist(&self, request: CreatePlaylistRequest) -> Result<Value> {
        if request.name.trim().is_empty() {
            return Err(Error::InvalidArgument("`name` must not be empty".into()));
        }
        if request.collaborative == Some(true) && request.public != Some(false) {
            return Err(Error::InvalidArgument(
                "a collaborative playlist must be created with `public: false`".into(),
            ));
        }
        let mut body = Map::new();
        body.insert("name".into(), request.name.into());
        insert_some(&mut body, "description", request.description);
        insert_some(&mut body, "public", request.public);
        insert_some(&mut body, "collaborative", request.collaborative);
        let raw = self
            .post("me/playlists", &[], Body::Json(Value::Object(body)))
            .await?;
        Ok(compact::playlist(&raw))
    }

    /// `update_playlist_details`.
    pub async fn update_playlist_details(&self, request: UpdatePlaylistRequest) -> Result<()> {
        let id = ids::id(&request.playlist_id, "playlist")?;
        let mut body = Map::new();
        insert_some(&mut body, "name", request.name);
        insert_some(&mut body, "description", request.description);
        insert_some(&mut body, "public", request.public);
        insert_some(&mut body, "collaborative", request.collaborative);
        if body.is_empty() {
            return Err(Error::InvalidArgument(
                "give at least one of name, description, public, collaborative".into(),
            ));
        }
        self.put(
            &format!("playlists/{id}"),
            &[],
            Body::Json(Value::Object(body)),
        )
        .await?;
        Ok(())
    }

    /// `add_playlist_items`: `{snapshot_id}`.
    pub async fn add_playlist_items(&self, request: AddItemsRequest) -> Result<Value> {
        let id = ids::id(&request.playlist_id, "playlist")?;
        let uris = item_uris(&request.uris, 1)?;
        let mut body = json!({ "uris": uris });
        if let Some(position) = request.position {
            body["position"] = position.into();
        }
        let raw = self
            .post(&format!("playlists/{id}/items"), &[], Body::Json(body))
            .await?;
        Ok(snapshot(raw))
    }

    /// `reorder_playlist_items`: `{snapshot_id}`.
    pub async fn reorder_playlist_items(&self, request: ReorderItemsRequest) -> Result<Value> {
        let id = ids::id(&request.playlist_id, "playlist")?;
        if request.range_length == Some(0) {
            return Err(Error::InvalidArgument(
                "`range_length` must be at least 1".into(),
            ));
        }
        let mut body = json!({
            "range_start": request.range_start,
            "insert_before": request.insert_before,
        });
        if let Some(length) = request.range_length {
            body["range_length"] = length.into();
        }
        if let Some(snapshot) = request.snapshot_id {
            body["snapshot_id"] = snapshot.into();
        }
        let raw = self
            .put(&format!("playlists/{id}/items"), &[], Body::Json(body))
            .await?;
        Ok(snapshot(raw))
    }

    /// `remove_playlist_items`: `{snapshot_id}`.
    pub async fn remove_playlist_items(&self, request: RemoveItemsRequest) -> Result<Value> {
        let id = ids::id(&request.playlist_id, "playlist")?;
        let items: Vec<Value> = item_uris(&request.uris, 1)?
            .into_iter()
            .map(|uri| json!({ "uri": uri }))
            .collect();
        let mut body = json!({ "items": items });
        if let Some(snapshot) = request.snapshot_id {
            body["snapshot_id"] = snapshot.into();
        }
        let raw = self
            .delete(&format!("playlists/{id}/items"), &[], Body::Json(body))
            .await?;
        Ok(snapshot(raw))
    }

    /// `replace_playlist_items`: `{snapshot_id}`.
    pub async fn replace_playlist_items(&self, request: ReplaceItemsRequest) -> Result<Value> {
        let id = ids::id(&request.playlist_id, "playlist")?;
        let uris = item_uris(&request.uris, 0)?;
        let raw = self
            .put(
                &format!("playlists/{id}/items"),
                &[],
                Body::Json(json!({ "uris": uris })),
            )
            .await?;
        Ok(snapshot(raw))
    }

    /// `upload_playlist_cover`.
    pub async fn upload_playlist_cover(&self, request: UploadCoverRequest) -> Result<()> {
        let id = ids::id(&request.playlist_id, "playlist")?;
        let jpeg = cover_base64(&request.jpeg_base64)?;
        self.put(&format!("playlists/{id}/images"), &[], Body::Jpeg(jpeg))
            .await?;
        Ok(())
    }
}

fn insert_some<T: Into<Value>>(map: &mut Map<String, Value>, key: &str, value: Option<T>) {
    if let Some(value) = value {
        map.insert(key.into(), value.into());
    }
}

/// Track or episode URIs, `min..=100` of them.
fn item_uris(inputs: &[String], min: usize) -> Result<Vec<String>> {
    count("uris", inputs, min, MAX_ITEMS)?;
    inputs
        .iter()
        .map(|u| ids::uri(u, &["track", "episode"]))
        .collect()
}

/// `{snapshot_id}` from a write response (an object or a bare string).
fn snapshot(raw: Value) -> Value {
    match raw {
        Value::String(s) => json!({ "snapshot_id": s }),
        Value::Object(_) => json!({ "snapshot_id": raw["snapshot_id"] }),
        _ => json!({ "snapshot_id": null }),
    }
}

/// Checks and cleans a base64 JPEG: strips a `data:` prefix and whitespace,
/// and enforces Spotify's 256 KB limit.
fn cover_base64(input: &str) -> Result<String> {
    let input = input.trim();
    let input = match input.strip_prefix("data:") {
        Some(rest) => rest.split_once(',').map_or(rest, |(_, data)| data),
        None => input,
    };
    let clean: String = input.chars().filter(|c| !c.is_whitespace()).collect();
    if clean.len() > MAX_COVER_BYTES {
        return Err(Error::InvalidArgument(format!(
            "the cover is {} KB of base64; Spotify accepts at most 256 KB",
            clean.len().div_ceil(1024)
        )));
    }
    let bytes = STANDARD
        .decode(&clean)
        .map_err(|_| Error::InvalidArgument("`jpeg_base64` is not valid base64".into()))?;
    if !bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Err(Error::InvalidArgument(
            "the cover must be a JPEG image".into(),
        ));
    }
    Ok(clean)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_covers() {
        let jpeg = STANDARD.encode([0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3]);
        assert_eq!(cover_base64(&jpeg).unwrap(), jpeg);
        assert_eq!(
            cover_base64(&format!("data:image/jpeg;base64,{jpeg}\n")).unwrap(),
            jpeg
        );
        assert!(cover_base64(&STANDARD.encode(b"\x89PNG....")).is_err());
        assert!(cover_base64("not base64!").is_err());
        assert!(cover_base64(&"A".repeat(MAX_COVER_BYTES + 4)).is_err());
    }

    #[test]
    fn checks_item_uris() {
        let uris = item_uris(&["spotify:episode:abc".into(), "def".into()], 1).unwrap();
        assert_eq!(uris, ["spotify:episode:abc", "spotify:track:def"]);
        assert!(item_uris(&[], 1).is_err());
        assert!(item_uris(&[], 0).unwrap().is_empty());
        assert!(item_uris(&["spotify:album:x".into()], 1).is_err());
    }

    #[test]
    fn reads_snapshots() {
        assert_eq!(snapshot(json!("abc")), json!({"snapshot_id": "abc"}));
        assert_eq!(
            snapshot(json!({"snapshot_id": "x"})),
            json!({"snapshot_id": "x"})
        );
    }
}
