//! The user's library: saved tracks (Liked Songs), albums, episodes, shows
//! and audiobooks, and, through the same endpoints since February 2026,
//! followed artists, users and playlists.

use serde_json::{Value, json};

use super::helpers::{count, market, paging};
use crate::{
    Account, Error, Result, compact, ids,
    rest::Body,
    types::{LibraryRequest, SavedKind, SavedRequest},
};

/// The most URIs per `/me/library` request.
const MAX_LIBRARY_URIS: usize = 40;

impl Account {
    /// `get_saved`.
    pub async fn get_saved(&self, request: SavedRequest) -> Result<Value> {
        let (path, key): (&str, &str) = match request.kind {
            SavedKind::Track => ("me/tracks", "track"),
            SavedKind::Album => ("me/albums", "album"),
            SavedKind::Episode => ("me/episodes", "episode"),
            SavedKind::Show => ("me/shows", "show"),
            SavedKind::Audiobook => ("me/audiobooks", "audiobook"),
        };
        let [limit, offset] = paging(request.limit, request.offset, 50)?;
        let raw = self
            .get(
                path,
                &[
                    ("market", market(request.market.as_deref())?),
                    limit,
                    offset,
                ],
            )
            .await?;
        Ok(compact::page(&raw, |saved| {
            // Audiobooks come unwrapped; the others as `{added_at, <kind>}`.
            let inner = if saved[key].is_object() {
                &saved[key]
            } else {
                saved
            };
            let mut item = compact::item(inner);
            if let (Value::Object(map), Some(at)) = (&mut item, saved["added_at"].as_str()) {
                map.insert("added_at".into(), at.into());
            }
            item
        }))
    }

    /// `check_saved`: `[{uri, saved}]`, in the order given.
    pub async fn check_saved(&self, request: LibraryRequest) -> Result<Value> {
        let uris = library_uris(&request)?;
        let raw = self
            .get("me/library/contains", &[("uris", Some(uris.join(",")))])
            .await?;
        let flags = raw
            .as_array()
            .ok_or_else(|| Error::Decode("library/contains did not return a list".into()))?;
        Ok(uris
            .iter()
            .zip(flags)
            .map(|(uri, saved)| json!({ "uri": uri, "saved": saved }))
            .collect())
    }

    /// `save_to_library`.
    pub async fn save_to_library(&self, request: LibraryRequest) -> Result<()> {
        let uris = library_uris(&request)?;
        self.put("me/library", &[("uris", Some(uris.join(",")))], Body::None)
            .await?;
        Ok(())
    }

    /// `remove_from_library`.
    pub async fn remove_from_library(&self, request: LibraryRequest) -> Result<()> {
        let uris = library_uris(&request)?;
        self.delete("me/library", &[("uris", Some(uris.join(",")))], Body::None)
            .await?;
        Ok(())
    }
}

/// The request's items as `spotify:{kind}:{id}` URIs.
fn library_uris(request: &LibraryRequest) -> Result<Vec<String>> {
    count("ids", &request.ids, 1, MAX_LIBRARY_URIS)?;
    let kind = request.kind.as_str();
    Ok(ids::ids(&request.ids, kind, MAX_LIBRARY_URIS)?
        .into_iter()
        .map(|id| format!("spotify:{kind}:{id}"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::LibraryKind;

    #[test]
    fn builds_library_uris() {
        let request = LibraryRequest {
            kind: LibraryKind::Artist,
            ids: vec![
                "0OdUWJ0sBjDrqHygGUXeCF".into(),
                "https://open.spotify.com/artist/1vCWHaC5f2uS3yhpwWbIA6".into(),
            ],
        };
        assert_eq!(
            library_uris(&request).unwrap(),
            [
                "spotify:artist:0OdUWJ0sBjDrqHygGUXeCF",
                "spotify:artist:1vCWHaC5f2uS3yhpwWbIA6"
            ]
        );
        let wrong = LibraryRequest {
            kind: LibraryKind::Track,
            ids: vec!["spotify:album:x".into()],
        };
        assert!(library_uris(&wrong).is_err());
    }
}
