//! Normalizing what agents pass for an item: a bare ID, a `spotify:` URI or
//! an `open.spotify.com` URL.

use crate::{Error, Result};

/// A parsed reference: the item kind (if the input named one) and its ID.
fn parse(input: &str) -> Result<(Option<String>, String)> {
    let input = input.trim();
    if input.is_empty() {
        return Err(Error::InvalidArgument("an ID must not be empty".into()));
    }
    if let Some(rest) = input.strip_prefix("spotify:") {
        // spotify:track:ID, or the legacy spotify:user:NAME:playlist:ID.
        let parts: Vec<&str> = rest.split(':').collect();
        return match parts.as_slice() {
            [kind, id] if !id.is_empty() => Ok((Some((*kind).to_owned()), (*id).to_owned())),
            ["user", _, kind, id] if !id.is_empty() => {
                Ok((Some((*kind).to_owned()), (*id).to_owned()))
            }
            _ => Err(Error::InvalidArgument(format!(
                "`{input}` is not a valid Spotify URI"
            ))),
        };
    }
    if input.starts_with("http://") || input.starts_with("https://") {
        let url = url::Url::parse(input)
            .map_err(|_| Error::InvalidArgument(format!("`{input}` is not a valid URL")))?;
        if url.host_str() != Some("open.spotify.com") {
            return Err(Error::InvalidArgument(format!(
                "`{input}` is not an open.spotify.com link"
            )));
        }
        let segments: Vec<&str> = url
            .path_segments()
            .map(|s| s.filter(|s| !s.is_empty()).collect())
            .unwrap_or_default();
        // Skip a locale prefix such as `intl-de`, and `/user/NAME/playlist/ID`.
        let segments: &[&str] = match segments.as_slice() {
            [first, rest @ ..] if first.starts_with("intl-") => rest,
            all => all,
        };
        let segments = match segments {
            ["user", _, rest @ ..] if rest.len() == 2 => rest,
            all => all,
        };
        return match segments {
            [kind, id] => Ok((Some((*kind).to_owned()), (*id).to_owned())),
            _ => Err(Error::InvalidArgument(format!(
                "`{input}` is not a link to a Spotify item"
            ))),
        };
    }
    if input.contains([':', '/', '?', '#', ' ']) {
        return Err(Error::InvalidArgument(format!(
            "`{input}` is not a Spotify ID, URI or link"
        )));
    }
    Ok((None, input.to_owned()))
}

/// The bare ID of a `kind` item (`track`, `album`, `playlist`, `user`, ...).
pub(crate) fn id(input: &str, kind: &str) -> Result<String> {
    match parse(input)? {
        (Some(found), _) if found != kind => Err(Error::InvalidArgument(format!(
            "`{input}` is a {found}, but a {kind} is expected here"
        ))),
        (_, id) => Ok(id),
    }
}

/// The bare IDs of `inputs`, at most `max` of them.
pub(crate) fn ids(inputs: &[String], kind: &str, max: usize) -> Result<Vec<String>> {
    if inputs.is_empty() {
        return Err(Error::InvalidArgument("give at least one ID".into()));
    }
    if inputs.len() > max {
        return Err(Error::InvalidArgument(format!(
            "at most {max} IDs per call; got {}",
            inputs.len()
        )));
    }
    inputs.iter().map(|i| id(i, kind)).collect()
}

/// A `spotify:{kind}:{id}` URI whose kind is one of `kinds`. A bare ID is
/// taken to be the first of `kinds`.
pub(crate) fn uri(input: &str, kinds: &[&str]) -> Result<String> {
    let (kind, id) = parse(input)?;
    let kind = kind.unwrap_or_else(|| kinds.first().copied().unwrap_or("track").to_owned());
    if !kinds.contains(&kind.as_str()) {
        return Err(Error::InvalidArgument(format!(
            "`{input}` is a {kind}; expected a {}",
            kinds.join(" or ")
        )));
    }
    Ok(format!("spotify:{kind}:{id}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    const ID: &str = "4uLU6hMCjMI75M1A2tKUQC";

    #[test]
    fn accepts_ids_uris_and_links() {
        for input in [
            ID.to_owned(),
            format!("spotify:track:{ID}"),
            format!("https://open.spotify.com/track/{ID}?si=abc"),
            format!("https://open.spotify.com/intl-de/track/{ID}"),
            format!("  {ID} "),
        ] {
            assert_eq!(id(&input, "track").unwrap(), ID, "{input}");
        }
        assert_eq!(
            id(&format!("spotify:user:bob:playlist:{ID}"), "playlist").unwrap(),
            ID
        );
        assert_eq!(
            id("https://open.spotify.com/user/bob.smith", "user").unwrap(),
            "bob.smith"
        );
    }

    #[test]
    fn rejects_wrong_kinds_and_junk() {
        assert!(id(&format!("spotify:album:{ID}"), "track").is_err());
        assert!(id("https://example.com/track/x", "track").is_err());
        assert!(id("", "track").is_err());
        assert!(id("a b", "track").is_err());
        assert!(id("spotify:track", "track").is_err());
    }

    #[test]
    fn builds_uris() {
        assert_eq!(
            uri(ID, &["track", "episode"]).unwrap(),
            format!("spotify:track:{ID}")
        );
        assert_eq!(
            uri(
                &format!("https://open.spotify.com/episode/{ID}"),
                &["track", "episode"]
            )
            .unwrap(),
            format!("spotify:episode:{ID}")
        );
        assert!(uri(&format!("spotify:album:{ID}"), &["track", "episode"]).is_err());
    }

    #[test]
    fn limits_id_lists() {
        let many = vec![ID.to_owned(); 3];
        assert!(ids(&many, "track", 2).is_err());
        assert!(ids(&[], "track", 2).is_err());
        assert_eq!(ids(&many, "track", 3).unwrap().len(), 3);
    }
}
