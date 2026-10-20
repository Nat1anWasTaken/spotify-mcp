use crate::{Account, Result, types::PlayRequest};

impl Account {
    pub async fn play(&self, request: PlayRequest) -> Result<()> {
        let _ = request;
        todo!("crates/spotify agent")
    }
}

impl Account {
    /// The dashboard's "now playing" card: the `NowPlaying` shape of
    /// docs/ARCHITECTURE.md, or `Value::Null` when nothing is playing.
    pub async fn now_playing(&self) -> Result<serde_json::Value> {
        todo!("crates/spotify agent")
    }
}
