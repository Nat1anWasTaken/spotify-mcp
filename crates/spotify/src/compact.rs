//! Compact, agent-friendly shapes of Web API objects: `uri`, `id`, `name`,
//! artists as `[{name, uri}]`, one image URL and `duration` as `m:ss`, with
//! `available_markets`, `href`, `external_urls` and similar noise dropped.

use serde_json::{Map, Value, json};

/// `ms` as `m:ss` (minutes are not wrapped into hours).
pub(crate) fn duration(ms: u64) -> String {
    let secs = ms / 1000;
    format!("{}:{:02}", secs / 60, secs % 60)
}

/// `value[key]` (milliseconds) as `m:ss`, or null.
fn duration_of(value: &Value, key: &str) -> Value {
    value[key]
        .as_u64()
        .map_or(Value::Null, |ms| Value::String(duration(ms)))
}

/// The first (largest) image URL of `value.images`, or null.
pub(crate) fn image(value: &Value) -> Value {
    value["images"]
        .as_array()
        .and_then(|images| images.first())
        .map_or(Value::Null, |image| image["url"].clone())
}

/// `[{name, uri}]` of `value.artists`.
pub(crate) fn artists(value: &Value) -> Value {
    value["artists"]
        .as_array()
        .map(|artists| {
            artists
                .iter()
                .map(|a| json!({ "name": a["name"], "uri": a["uri"] }))
                .collect()
        })
        .unwrap_or_else(|| Value::Array(Vec::new()))
}

/// Builds an object from `pairs`, leaving out nulls.
fn object<const N: usize>(pairs: [(&str, Value); N]) -> Value {
    Value::Object(
        pairs
            .into_iter()
            .filter(|(_, v)| !v.is_null())
            .map(|(k, v)| (k.to_owned(), v))
            .collect::<Map<_, _>>(),
    )
}

/// The common head of every item.
fn head(value: &Value) -> [(&'static str, Value); 3] {
    [
        ("uri", value["uri"].clone()),
        ("id", value["id"].clone()),
        ("name", value["name"].clone()),
    ]
}

/// Any item, by its `type`. Null stays null (unavailable playlist items).
pub(crate) fn item(value: &Value) -> Value {
    match value["type"].as_str() {
        Some("track") => track(value),
        Some("album") => album(value),
        Some("artist") => artist(value),
        Some("playlist") => playlist(value),
        Some("show") => show(value),
        Some("episode") => episode(value),
        Some("audiobook") => audiobook(value),
        Some("chapter") => chapter(value),
        Some("user") => user(value),
        _ => value.clone(),
    }
}

pub(crate) fn track(value: &Value) -> Value {
    if value.is_null() {
        return Value::Null;
    }
    let [uri, id, name] = head(value);
    let album = &value["album"];
    object([
        uri,
        id,
        name,
        ("artists", artists(value)),
        (
            "album",
            if album.is_object() {
                json!({ "name": album["name"], "uri": album["uri"] })
            } else {
                Value::Null
            },
        ),
        ("image", image(album)),
        ("duration", duration_of(value, "duration_ms")),
        ("explicit", value["explicit"].clone()),
        ("popularity", value["popularity"].clone()),
        ("track_number", value["track_number"].clone()),
        ("disc_number", value["disc_number"].clone()),
        ("is_playable", value["is_playable"].clone()),
        (
            "is_local",
            value["is_local"].as_bool().filter(|l| *l).into(),
        ),
    ])
}

pub(crate) fn album(value: &Value) -> Value {
    if value.is_null() {
        return Value::Null;
    }
    let [uri, id, name] = head(value);
    let tracks = &value["tracks"];
    object([
        uri,
        id,
        name,
        ("album_type", value["album_type"].clone()),
        ("artists", artists(value)),
        ("release_date", value["release_date"].clone()),
        ("total_tracks", value["total_tracks"].clone()),
        ("image", image(value)),
        ("label", value["label"].clone()),
        ("popularity", value["popularity"].clone()),
        ("album_group", value["album_group"].clone()),
        (
            "tracks",
            if tracks.is_object() {
                page(tracks, track)
            } else {
                Value::Null
            },
        ),
    ])
}

pub(crate) fn artist(value: &Value) -> Value {
    if value.is_null() {
        return Value::Null;
    }
    let [uri, id, name] = head(value);
    object([
        uri,
        id,
        name,
        ("genres", value["genres"].clone()),
        ("followers", value["followers"]["total"].clone()),
        ("popularity", value["popularity"].clone()),
        ("image", image(value)),
    ])
}

pub(crate) fn playlist(value: &Value) -> Value {
    if value.is_null() {
        return Value::Null;
    }
    let [uri, id, name] = head(value);
    // Spotify renamed `tracks` to `items` in February 2026; accept both.
    let items = if value["items"].is_object() {
        &value["items"]
    } else {
        &value["tracks"]
    };
    object([
        uri,
        id,
        name,
        ("description", value["description"].clone()),
        (
            "owner",
            json!({ "id": value["owner"]["id"], "name": value["owner"]["display_name"] }),
        ),
        ("public", value["public"].clone()),
        ("collaborative", value["collaborative"].clone()),
        ("followers", value["followers"]["total"].clone()),
        ("snapshot_id", value["snapshot_id"].clone()),
        ("image", image(value)),
        (
            "items",
            if items["items"].is_array() {
                page(items, playlist_item)
            } else {
                Value::Null
            },
        ),
        ("total_items", items["total"].clone()),
    ])
}

/// A playlist entry: the track or episode, plus who added it and when.
pub(crate) fn playlist_item(value: &Value) -> Value {
    // `item` since February 2026, `track` before.
    let entry = if value["item"].is_object() {
        &value["item"]
    } else {
        &value["track"]
    };
    let mut compact = item(entry);
    if let Value::Object(map) = &mut compact {
        if !value["added_at"].is_null() {
            map.insert("added_at".into(), value["added_at"].clone());
        }
        if let Some(by) = value["added_by"]["id"].as_str() {
            map.insert("added_by".into(), by.into());
        }
    }
    compact
}

pub(crate) fn show(value: &Value) -> Value {
    if value.is_null() {
        return Value::Null;
    }
    let [uri, id, name] = head(value);
    object([
        uri,
        id,
        name,
        ("publisher", value["publisher"].clone()),
        ("description", value["description"].clone()),
        ("total_episodes", value["total_episodes"].clone()),
        ("explicit", value["explicit"].clone()),
        ("image", image(value)),
    ])
}

pub(crate) fn episode(value: &Value) -> Value {
    if value.is_null() {
        return Value::Null;
    }
    let [uri, id, name] = head(value);
    let show = &value["show"];
    object([
        uri,
        id,
        name,
        (
            "show",
            if show.is_object() {
                json!({ "name": show["name"], "uri": show["uri"] })
            } else {
                Value::Null
            },
        ),
        ("description", value["description"].clone()),
        ("release_date", value["release_date"].clone()),
        ("duration", duration_of(value, "duration_ms")),
        ("explicit", value["explicit"].clone()),
        (
            "resume_point",
            value["resume_point"]["resume_position_ms"]
                .as_u64()
                .map_or(Value::Null, |ms| {
                    json!({
                        "fully_played": value["resume_point"]["fully_played"],
                        "position": duration(ms),
                    })
                }),
        ),
        ("image", image(value)),
    ])
}

pub(crate) fn audiobook(value: &Value) -> Value {
    if value.is_null() {
        return Value::Null;
    }
    let [uri, id, name] = head(value);
    let names = |key: &str| -> Value {
        value[key]
            .as_array()
            .map(|list| list.iter().map(|a| a["name"].clone()).collect())
            .unwrap_or(Value::Null)
    };
    object([
        uri,
        id,
        name,
        ("authors", names("authors")),
        ("narrators", names("narrators")),
        ("publisher", value["publisher"].clone()),
        ("description", value["description"].clone()),
        ("edition", value["edition"].clone()),
        ("total_chapters", value["total_chapters"].clone()),
        ("explicit", value["explicit"].clone()),
        ("image", image(value)),
    ])
}

pub(crate) fn chapter(value: &Value) -> Value {
    if value.is_null() {
        return Value::Null;
    }
    let [uri, id, name] = head(value);
    let book = &value["audiobook"];
    object([
        uri,
        id,
        name,
        ("chapter_number", value["chapter_number"].clone()),
        (
            "audiobook",
            if book.is_object() {
                json!({ "name": book["name"], "uri": book["uri"] })
            } else {
                Value::Null
            },
        ),
        ("description", value["description"].clone()),
        ("release_date", value["release_date"].clone()),
        ("duration", duration_of(value, "duration_ms")),
        ("image", image(value)),
    ])
}

pub(crate) fn user(value: &Value) -> Value {
    if value.is_null() {
        return Value::Null;
    }
    object([
        ("uri", value["uri"].clone()),
        ("id", value["id"].clone()),
        ("name", value["display_name"].clone()),
        ("followers", value["followers"]["total"].clone()),
        ("image", image(value)),
    ])
}

pub(crate) fn device(value: &Value) -> Value {
    object([
        ("id", value["id"].clone()),
        ("name", value["name"].clone()),
        ("type", value["type"].clone()),
        ("is_active", value["is_active"].clone()),
        ("is_restricted", value["is_restricted"].clone()),
        ("is_private_session", value["is_private_session"].clone()),
        ("volume_percent", value["volume_percent"].clone()),
        ("supports_volume", value["supports_volume"].clone()),
    ])
}

/// An offset-paged list as `{items, total, offset, next_offset}`;
/// `next_offset` is null on the last page.
pub(crate) fn page(value: &Value, map: impl Fn(&Value) -> Value) -> Value {
    let items: Vec<Value> = value["items"]
        .as_array()
        .map(|items| items.iter().map(&map).collect())
        .unwrap_or_default();
    let offset = value["offset"].as_u64().unwrap_or(0);
    let limit = value["limit"].as_u64().unwrap_or(items.len() as u64);
    let next_offset = if value["next"].is_null() {
        Value::Null
    } else {
        (offset + limit).into()
    };
    json!({
        "items": items,
        "total": value["total"],
        "offset": offset,
        "next_offset": next_offset,
    })
}

/// A cursor-paged list as `{items, total?, next_after}` (followed artists).
pub(crate) fn cursor_page(value: &Value, map: impl Fn(&Value) -> Value) -> Value {
    let items: Vec<Value> = value["items"]
        .as_array()
        .map(|items| items.iter().map(&map).collect())
        .unwrap_or_default();
    let next_after = if value["next"].is_null() {
        Value::Null
    } else {
        value["cursors"]["after"].clone()
    };
    object([
        ("items", Value::Array(items)),
        ("total", value["total"].clone()),
        ("next_after", next_after),
    ])
}

/// The `NowPlaying` shape of docs/ARCHITECTURE.md from a playback state;
/// null when nothing is loaded.
pub(crate) fn now_playing(state: &Value) -> Value {
    if state.is_null() {
        return Value::Null;
    }
    let device = &state["device"];
    let item = &state["item"];
    let item = if item.is_object() {
        let (artists, album, image) = if item["type"] == "episode" {
            let show = &item["show"];
            (
                json!([{ "name": show["name"], "uri": show["uri"] }]),
                Value::Null,
                if item["images"].is_array() {
                    self::image(item)
                } else {
                    self::image(show)
                },
            )
        } else {
            (
                artists(item),
                item["album"]["name"].clone(),
                self::image(&item["album"]),
            )
        };
        json!({
            "uri": item["uri"],
            "name": item["name"],
            "artists": artists,
            "album": album,
            "image": image,
            "duration": duration(item["duration_ms"].as_u64().unwrap_or(0)),
            "duration_ms": item["duration_ms"].as_u64().unwrap_or(0),
        })
    } else {
        Value::Null
    };
    json!({
        "is_playing": state["is_playing"].as_bool().unwrap_or(false),
        "progress": state["progress_ms"].as_u64().map(duration),
        "progress_ms": state["progress_ms"].as_u64(),
        "device": if device.is_object() {
            json!({
                "name": device["name"],
                "type": device["type"],
                "volume_percent": device["volume_percent"],
            })
        } else {
            Value::Null
        },
        "item": item,
    })
}

/// A playback state (`get_playback_state`) for agents.
pub(crate) fn playback_state(state: &Value) -> Value {
    if state.is_null() {
        return Value::Null;
    }
    object([
        ("is_playing", state["is_playing"].clone()),
        (
            "progress",
            state["progress_ms"].as_u64().map(duration).into(),
        ),
        ("shuffle", state["shuffle_state"].clone()),
        ("repeat", state["repeat_state"].clone()),
        (
            "device",
            if state["device"].is_object() {
                device(&state["device"])
            } else {
                Value::Null
            },
        ),
        (
            "context",
            if state["context"].is_object() {
                json!({ "type": state["context"]["type"], "uri": state["context"]["uri"] })
            } else {
                Value::Null
            },
        ),
        (
            "currently_playing_type",
            state["currently_playing_type"].clone(),
        ),
        ("item", item(&state["item"])),
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_durations() {
        assert_eq!(duration(0), "0:00");
        assert_eq!(duration(59_999), "0:59");
        assert_eq!(duration(215_000), "3:35");
        assert_eq!(duration(3_725_000), "62:05");
    }

    fn raw_track() -> Value {
        json!({
            "type": "track", "id": "t1", "uri": "spotify:track:t1", "name": "Song",
            "href": "https://api.spotify.com/v1/tracks/t1",
            "external_urls": {"spotify": "https://open.spotify.com/track/t1"},
            "available_markets": ["US", "TW"],
            "duration_ms": 215_000, "explicit": false, "popularity": 50,
            "artists": [{"name": "A", "uri": "spotify:artist:a", "href": "x", "id": "a"}],
            "album": {"name": "Al", "uri": "spotify:album:al", "available_markets": [],
                      "images": [{"url": "big", "width": 640}, {"url": "small", "width": 64}]},
            "is_local": false
        })
    }

    #[test]
    fn compacts_tracks() {
        let t = track(&raw_track());
        assert_eq!(
            t,
            json!({
                "uri": "spotify:track:t1", "id": "t1", "name": "Song",
                "artists": [{"name": "A", "uri": "spotify:artist:a"}],
                "album": {"name": "Al", "uri": "spotify:album:al"},
                "image": "big", "duration": "3:35", "explicit": false, "popularity": 50
            })
        );
    }

    #[test]
    fn pages() {
        let raw = json!({"items": [raw_track()], "total": 3, "offset": 0, "limit": 1,
                         "next": "https://api/x?offset=1", "href": "h"});
        let p = page(&raw, track);
        assert_eq!(p["next_offset"], 1);
        assert_eq!(p["total"], 3);
        assert_eq!(p["items"][0]["id"], "t1");
        let last = json!({"items": [], "total": 3, "offset": 2, "limit": 1, "next": null});
        assert_eq!(page(&last, track)["next_offset"], Value::Null);
    }

    #[test]
    fn playlist_items_old_and_new_shapes() {
        let new = json!({"added_at": "2026-01-01T00:00:00Z", "added_by": {"id": "u"}, "item": raw_track()});
        let old = json!({"added_at": "2026-01-01T00:00:00Z", "track": raw_track()});
        assert_eq!(playlist_item(&new)["id"], "t1");
        assert_eq!(playlist_item(&new)["added_by"], "u");
        assert_eq!(playlist_item(&old)["id"], "t1");
        assert_eq!(playlist_item(&json!({"item": null})), Value::Null);
    }

    #[test]
    fn now_playing_shape() {
        assert_eq!(now_playing(&Value::Null), Value::Null);
        let state = json!({
            "is_playing": true, "progress_ms": 61_000,
            "device": {"id": "d", "name": "Mac", "type": "Computer", "volume_percent": 40},
            "item": raw_track()
        });
        assert_eq!(
            now_playing(&state),
            json!({
                "is_playing": true, "progress": "1:01", "progress_ms": 61_000,
                "device": {"name": "Mac", "type": "Computer", "volume_percent": 40},
                "item": {"uri": "spotify:track:t1", "name": "Song",
                         "artists": [{"name": "A", "uri": "spotify:artist:a"}],
                         "album": "Al", "image": "big", "duration": "3:35",
                         "duration_ms": 215_000}
            })
        );
    }

    #[test]
    fn dispatches_on_type() {
        let a = item(
            &json!({"type": "artist", "id": "a", "uri": "spotify:artist:a", "name": "A",
                             "followers": {"href": null, "total": 7}, "genres": [], "images": []}),
        );
        assert_eq!(
            a,
            json!({"uri": "spotify:artist:a", "id": "a", "name": "A", "genres": [], "followers": 7})
        );
    }
}
