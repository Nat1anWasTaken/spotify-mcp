//! Catalog items by ID: tracks, albums, artists, shows, episodes,
//! audiobooks and chapters, and their children.
//!
//! The batch "get several" endpoints are gone for development-mode apps
//! (February 2026), so each ID is fetched on its own, a few at a time.

use futures::{StreamExt, TryStreamExt, stream};
use serde_json::{Value, json};

use super::helpers::{market, paging};
use crate::{
    Account, Error, Result, compact, ids,
    types::{ArtistAlbumsRequest, IdsRequest, PagedIdRequest},
};

/// The most IDs one call fetches.
const MAX_IDS: usize = 10;
/// Requests in flight at once.
const CONCURRENCY: usize = 4;

impl Account {
    /// Fetches each ID from `/{collection}/{id}` and compacts it. IDs that
    /// don't exist come back as `{id, error}` instead of failing the call.
    async fn several(
        &self,
        request: IdsRequest,
        kind: &str,
        collection: &str,
        map: fn(&Value) -> Value,
    ) -> Result<Value> {
        let ids = ids::ids(&request.ids, kind, MAX_IDS)?;
        let market = market(request.market.as_deref())?;
        let items: Vec<Value> = stream::iter(ids)
            .map(|id| {
                let market = market.clone();
                async move {
                    let path = format!("{collection}/{id}");
                    match self.get(&path, &[("market", market)]).await {
                        Ok(raw) => Ok(map(&raw)),
                        Err(Error::NotFound(_)) => Ok(json!({ "id": id, "error": "not found" })),
                        Err(Error::InvalidArgument(message)) => {
                            Ok(json!({ "id": id, "error": message }))
                        }
                        Err(e) => Err(e),
                    }
                }
            })
            .buffered(CONCURRENCY)
            .try_collect()
            .await?;
        Ok(Value::Array(items))
    }

    /// A page of `/{collection}/{id}/{children}`.
    async fn children(
        &self,
        request: PagedIdRequest,
        kind: &str,
        path: &str,
        map: fn(&Value) -> Value,
    ) -> Result<Value> {
        let id = ids::id(&request.id, kind)?;
        let [limit, offset] = paging(request.limit, request.offset, 50)?;
        let raw = self
            .get(
                &format!("{kind}s/{id}/{path}"),
                &[
                    ("market", market(request.market.as_deref())?),
                    limit,
                    offset,
                ],
            )
            .await?;
        Ok(compact::page(&raw, map))
    }

    /// `get_tracks`.
    pub async fn get_tracks(&self, request: IdsRequest) -> Result<Value> {
        self.several(request, "track", "tracks", compact::track)
            .await
    }

    /// `get_albums`.
    pub async fn get_albums(&self, request: IdsRequest) -> Result<Value> {
        self.several(request, "album", "albums", compact::album)
            .await
    }

    /// `get_artists`.
    pub async fn get_artists(&self, request: IdsRequest) -> Result<Value> {
        self.several(request, "artist", "artists", compact::artist)
            .await
    }

    /// `get_shows`.
    pub async fn get_shows(&self, request: IdsRequest) -> Result<Value> {
        self.several(request, "show", "shows", compact::show).await
    }

    /// `get_episodes`.
    pub async fn get_episodes(&self, request: IdsRequest) -> Result<Value> {
        self.several(request, "episode", "episodes", compact::episode)
            .await
    }

    /// `get_audiobooks`.
    pub async fn get_audiobooks(&self, request: IdsRequest) -> Result<Value> {
        self.several(request, "audiobook", "audiobooks", compact::audiobook)
            .await
    }

    /// `get_chapters`.
    pub async fn get_chapters(&self, request: IdsRequest) -> Result<Value> {
        self.several(request, "chapter", "chapters", compact::chapter)
            .await
    }

    /// `get_album_tracks`.
    pub async fn get_album_tracks(&self, request: PagedIdRequest) -> Result<Value> {
        self.children(request, "album", "tracks", compact::track)
            .await
    }

    /// `get_show_episodes`.
    pub async fn get_show_episodes(&self, request: PagedIdRequest) -> Result<Value> {
        self.children(request, "show", "episodes", compact::episode)
            .await
    }

    /// `get_audiobook_chapters`.
    pub async fn get_audiobook_chapters(&self, request: PagedIdRequest) -> Result<Value> {
        self.children(request, "audiobook", "chapters", compact::chapter)
            .await
    }

    /// `get_artist_albums`.
    pub async fn get_artist_albums(&self, request: ArtistAlbumsRequest) -> Result<Value> {
        let id = ids::id(&request.id, "artist")?;
        let [limit, offset] = paging(request.limit, request.offset, 10)?;
        let groups = request
            .include_groups
            .filter(|g| !g.is_empty())
            .map(|g| g.iter().map(|g| g.as_str()).collect::<Vec<_>>().join(","));
        let raw = self
            .get(
                &format!("artists/{id}/albums"),
                &[
                    ("include_groups", groups),
                    ("market", market(request.market.as_deref())?),
                    limit,
                    offset,
                ],
            )
            .await?;
        Ok(compact::page(&raw, compact::album))
    }
}
