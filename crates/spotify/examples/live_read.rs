//! Read-only smoke test against the real Web API.
//!
//! ```sh
//! SPOTIFY_ACCESS_TOKEN=... cargo run -p spotify --example live_read [query]
//! ```

use spotify::{
    Credentials, Spotify,
    types::{NoParams, SearchRequest, SearchType},
};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let token = std::env::var("SPOTIFY_ACCESS_TOKEN")
        .map_err(|_| "set SPOTIFY_ACCESS_TOKEN to a user access token")?;
    let query = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "Radiohead".into());
    // Client credentials are only needed for the token endpoint.
    let spotify = Spotify::new(Credentials {
        client_id: String::new(),
        client_secret: String::new(),
    });
    let account = spotify.account(&token);

    let me = account.get_me(NoParams {}).await?;
    println!("get_me:\n{}\n", serde_json::to_string_pretty(&me)?);

    let results = account
        .search(SearchRequest {
            query,
            types: vec![SearchType::Track, SearchType::Artist],
            market: None,
            limit: Some(3),
            offset: None,
        })
        .await?;
    println!("search:\n{}\n", serde_json::to_string_pretty(&results)?);

    let devices = account.list_devices(NoParams {}).await?;
    println!("list_devices:\n{}", serde_json::to_string_pretty(&devices)?);
    Ok(())
}
