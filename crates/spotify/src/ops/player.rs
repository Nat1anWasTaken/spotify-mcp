//! Playback: state, devices, the queue and player commands. Commands need
//! Spotify Premium.

use serde_json::{Map, Value, json};

use crate::{
    Account, Result, compact, ids,
    rest::Body,
    types::{PlayOffset, PlayRequest},
};

impl Account {
    /// The dashboard's "now playing" card: the `NowPlaying` shape of
    /// docs/ARCHITECTURE.md, or `Value::Null` when nothing is playing.
    pub async fn now_playing(&self) -> Result<Value> {
        let state = self
            .get(
                "me/player",
                &[("additional_types", Some("track,episode".into()))],
            )
            .await?;
        Ok(compact::now_playing(&state))
    }

    /// `play`.
    pub async fn play(&self, request: PlayRequest) -> Result<()> {
        let mut body = Map::new();
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
        self.put(
            "me/player/play",
            &[("device_id", request.device_id)],
            Body::Json(Value::Object(body)),
        )
        .await?;
        Ok(())
    }
}
