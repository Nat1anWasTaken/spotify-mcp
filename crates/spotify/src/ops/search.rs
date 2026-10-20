//! Catalog search.

use serde_json::{Map, Value};

use super::helpers::{market, paging, range};
use crate::{Account, Error, Result, compact, types::SearchRequest};

impl Account {
    /// `search`: one compact page per requested type, keyed by type
    /// (`tracks`, `albums`, ...).
    pub async fn search(&self, request: SearchRequest) -> Result<Value> {
        if request.query.trim().is_empty() {
            return Err(Error::InvalidArgument("`query` must not be empty".into()));
        }
        if request.types.is_empty() {
            return Err(Error::InvalidArgument(
                "`types` must name at least one kind, e.g. [\"track\"]".into(),
            ));
        }
        let mut types: Vec<&str> = request.types.iter().map(|t| t.as_str()).collect();
        types.sort_unstable();
        types.dedup();
        let [limit, _] = paging(request.limit, None, 10)?;
        let raw = self
            .get(
                "search",
                &[
                    ("q", Some(request.query)),
                    ("type", Some(types.join(","))),
                    ("market", market(request.market.as_deref())?),
                    limit,
                    ("offset", range("offset", request.offset, 0, 1000)?),
                ],
            )
            .await?;
        let mut out = Map::new();
        if let Value::Object(results) = raw {
            for (key, page) in results {
                let mut page = compact::page(&page, compact::item);
                // Search may return nulls for unavailable items.
                if let Some(items) = page["items"].as_array_mut() {
                    items.retain(|i| !i.is_null());
                }
                out.insert(key, page);
            }
        }
        Ok(Value::Object(out))
    }
}
