//! Playback: state, devices, the queue and player commands. Commands need
//! Spotify Premium.

use serde_json::{Map, Value, json};

use super::helpers::market;
use crate::{
    Account, Error, Result, compact, ids,
    rest::Body,
    types::{
        DeviceRequest, MarketRequest, NoParams, PlayOffset, PlayRequest, QueueRequest,
        RepeatRequest, SeekRequest, ShuffleRequest, TransferRequest, VolumeRequest,
    },
};

/// `additional_types`, so episodes are returned too.
fn episodes_too() -> (&'static str, Option<String>) {
    ("additional_types", Some("track,episode".into()))
}

impl Account {
    /// The dashboard's "now playing" card: the `NowPlaying` shape of
    /// docs/ARCHITECTURE.md (plus `progress_ms` and `item.duration_ms`), or
    /// `Value::Null` when nothing is playing.
    pub async fn now_playing(&self) -> Result<Value> {
        let state = self.get("me/player", &[episodes_too()]).await?;
        Ok(compact::now_playing(&state))
    }

    /// `get_playback_state`: device, shuffle/repeat, context and item, or
    /// null when nothing is playing.
    pub async fn get_playback_state(&self, request: MarketRequest) -> Result<Value> {
        let state = self
            .get(
                "me/player",
                &[
                    ("market", market(request.market.as_deref())?),
                    episodes_too(),
                ],
            )
            .await?;
        Ok(compact::playback_state(&state))
    }

    /// `get_currently_playing`: the item and progress, or null.
    pub async fn get_currently_playing(&self, request: MarketRequest) -> Result<Value> {
        let state = self
            .get(
                "me/player/currently-playing",
                &[
                    ("market", market(request.market.as_deref())?),
                    episodes_too(),
                ],
            )
            .await?;
        Ok(compact::playback_state(&state))
    }

    /// `list_devices`.
    pub async fn list_devices(&self, _: NoParams) -> Result<Value> {
        let raw = self.get("me/player/devices", &[]).await?;
        Ok(raw["devices"]
            .as_array()
            .map(|devices| devices.iter().map(compact::device).collect())
            .unwrap_or_else(|| Value::Array(Vec::new())))
    }

    /// `get_queue`: `{currently_playing, queue}`.
    pub async fn get_queue(&self, _: NoParams) -> Result<Value> {
        let raw = self.get("me/player/queue", &[]).await?;
        let queue: Vec<Value> = raw["queue"]
            .as_array()
            .map(|items| items.iter().map(compact::item).collect())
            .unwrap_or_default();
        Ok(json!({
            "currently_playing": compact::item(&raw["currently_playing"]),
            "queue": queue,
        }))
    }

    /// `transfer_playback`.
    pub async fn transfer_playback(&self, request: TransferRequest) -> Result<()> {
        let mut body = json!({ "device_ids": [request.device_id] });
        if let Some(play) = request.play {
            body["play"] = play.into();
        }
        self.put("me/player", &[], Body::Json(body)).await?;
        Ok(())
    }

    /// `play`.
    pub async fn play(&self, request: PlayRequest) -> Result<()> {
        let mut body = Map::new();
        if request.context_uri.is_some() && request.uris.is_some() {
            return Err(Error::InvalidArgument(
                "give `context_uri` or `uris`, not both".into(),
            ));
        }
        if let Some(context) = &request.context_uri {
            body.insert(
                "context_uri".into(),
                ids::uri(
                    context,
                    &["album", "playlist", "artist", "show", "audiobook"],
                )?
                .into(),
            );
        }
        if let Some(uris) = &request.uris {
            let uris = uris
                .iter()
                .map(|u| ids::uri(u, &["track", "episode"]))
                .collect::<Result<Vec<_>>>()?;
            body.insert("uris".into(), uris.into());
        }
        if let Some(offset) = &request.offset {
            let offset = match offset {
                PlayOffset::Position(position) => json!({ "position": position }),
                PlayOffset::Uri(uri) => json!({ "uri": ids::uri(uri, &["track", "episode"])? }),
            };
            body.insert("offset".into(), offset);
        }
        if let Some(position) = request.position_ms {
            body.insert("position_ms".into(), position.into());
        }
        let body = if body.is_empty() {
            Body::None
        } else {
            Body::Json(Value::Object(body))
        };
        self.put("me/player/play", &[("device_id", request.device_id)], body)
            .await?;
        Ok(())
    }

    /// `pause`.
    pub async fn pause(&self, request: DeviceRequest) -> Result<()> {
        self.put(
            "me/player/pause",
            &[("device_id", request.device_id)],
            Body::None,
        )
        .await?;
        Ok(())
    }

    /// `next`.
    pub async fn next(&self, request: DeviceRequest) -> Result<()> {
        self.post(
            "me/player/next",
            &[("device_id", request.device_id)],
            Body::None,
        )
        .await?;
        Ok(())
    }

    /// `previous`.
    pub async fn previous(&self, request: DeviceRequest) -> Result<()> {
        self.post(
            "me/player/previous",
            &[("device_id", request.device_id)],
            Body::None,
        )
        .await?;
        Ok(())
    }

    /// `seek`.
    pub async fn seek(&self, request: SeekRequest) -> Result<()> {
        self.put(
            "me/player/seek",
            &[
                ("position_ms", Some(request.position_ms.to_string())),
                ("device_id", request.device_id),
            ],
            Body::None,
        )
        .await?;
        Ok(())
    }

    /// `set_repeat`.
    pub async fn set_repeat(&self, request: RepeatRequest) -> Result<()> {
        self.put(
            "me/player/repeat",
            &[
                ("state", Some(request.state.as_str().into())),
                ("device_id", request.device_id),
            ],
            Body::None,
        )
        .await?;
        Ok(())
    }

    /// `set_shuffle`.
    pub async fn set_shuffle(&self, request: ShuffleRequest) -> Result<()> {
        self.put(
            "me/player/shuffle",
            &[
                ("state", Some(request.state.to_string())),
                ("device_id", request.device_id),
            ],
            Body::None,
        )
        .await?;
        Ok(())
    }

    /// `set_volume`.
    pub async fn set_volume(&self, request: VolumeRequest) -> Result<()> {
        if request.volume_percent > 100 {
            return Err(Error::InvalidArgument(format!(
                "`volume_percent` must be 0 to 100; got {}",
                request.volume_percent
            )));
        }
        self.put(
            "me/player/volume",
            &[
                ("volume_percent", Some(request.volume_percent.to_string())),
                ("device_id", request.device_id),
            ],
            Body::None,
        )
        .await?;
        Ok(())
    }

    /// `add_to_queue`.
    pub async fn add_to_queue(&self, request: QueueRequest) -> Result<()> {
        let uri = ids::uri(&request.uri, &["track", "episode"])?;
        self.post(
            "me/player/queue",
            &[("uri", Some(uri)), ("device_id", request.device_id)],
            Body::None,
        )
        .await?;
        Ok(())
    }
}
