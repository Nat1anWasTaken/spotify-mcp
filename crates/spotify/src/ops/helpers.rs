//! Argument validation shared by the operations.

use crate::{Error, Result};

/// `value` as a query parameter, after checking it is within `min..=max`.
pub(crate) fn range(name: &str, value: Option<u32>, min: u32, max: u32) -> Result<Option<String>> {
    match value {
        Some(v) if !(min..=max).contains(&v) => Err(Error::InvalidArgument(format!(
            "`{name}` must be between {min} and {max}; got {v}"
        ))),
        v => Ok(v.map(|v| v.to_string())),
    }
}

/// `limit` (1..=`max`) and `offset` query parameters.
pub(crate) fn paging(
    limit: Option<u32>,
    offset: Option<u32>,
    max_limit: u32,
) -> Result<[(&'static str, Option<String>); 2]> {
    Ok([
        ("limit", range("limit", limit, 1, max_limit)?),
        ("offset", offset.map(|o| o.to_string())),
    ])
}

/// A checked ISO 3166-1 alpha-2 market code.
pub(crate) fn market(market: Option<&str>) -> Result<Option<String>> {
    match market.map(str::trim) {
        None | Some("") => Ok(None),
        Some(m) if m.len() == 2 && m.chars().all(|c| c.is_ascii_alphabetic()) => {
            Ok(Some(m.to_ascii_uppercase()))
        }
        Some(m) => Err(Error::InvalidArgument(format!(
            "`market` must be a two-letter country code such as US; got `{m}`"
        ))),
    }
}

/// Checks that a list has between `min` and `max` entries.
pub(crate) fn count<T>(name: &str, list: &[T], min: usize, max: usize) -> Result<()> {
    if (min..=max).contains(&list.len()) {
        Ok(())
    } else {
        Err(Error::InvalidArgument(format!(
            "`{name}` must have {min} to {max} entries; got {}",
            list.len()
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checks_ranges() {
        assert_eq!(
            range("limit", Some(10), 1, 10).unwrap().as_deref(),
            Some("10")
        );
        assert!(range("limit", Some(0), 1, 10).is_err());
        assert!(range("limit", Some(11), 1, 10).is_err());
        assert_eq!(range("limit", None, 1, 10).unwrap(), None);
    }

    #[test]
    fn checks_markets() {
        assert_eq!(market(Some("tw")).unwrap().as_deref(), Some("TW"));
        assert_eq!(market(None).unwrap(), None);
        assert!(market(Some("TWN")).is_err());
    }
}
