use super::*;
use chrono::{TimeZone, Utc};
use serde::Deserialize;

#[derive(Deserialize)]
struct Hit {
    id: String,
    name: String,
    info_hash: String,
    size: String,
    seeders: String,
    leechers: String,
    added: String,
}

pub async fn search(provider: &HttpProvider, query: &str) -> Result<Vec<Torrent>, ProviderError> {
    let mut url =
        Url::parse(&provider.config.url).map_err(|_| ProviderError::data("invalid APIBay URL"))?;
    let retained: Vec<_> = url
        .query_pairs()
        .filter(|(key, _)| !matches!(key.as_ref(), "q" | "cat"))
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect();
    url.set_query(None);
    url.query_pairs_mut().extend_pairs(retained);
    url.query_pairs_mut()
        .append_pair("q", query)
        .append_pair("cat", "0");
    let bytes = body(provider.client.get(url)).await?;
    parse(&bytes, &provider.config.name)
}

fn parse(bytes: &[u8], source: &str) -> Result<Vec<Torrent>, ProviderError> {
    let hits: Vec<Hit> = serde_json::from_slice(bytes)
        .map_err(|_| ProviderError::data("invalid APIBay JSON response"))?;
    Ok(hits
        .into_iter()
        .filter(|hit| hit.id != "0" && hit.name != "No results returned")
        .map(|hit| Torrent {
            title: hit.name,
            info_hash: Some(hit.info_hash),
            size: hit.size.parse().ok(),
            seeders: hit.seeders.parse().ok(),
            leechers: hit.leechers.parse().ok(),
            published_at: hit
                .added
                .parse::<i64>()
                .ok()
                .and_then(|timestamp| Utc.timestamp_opt(timestamp, 0).single()),
            sources: vec![source.into()],
            ..Default::default()
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_string_counts_and_filters_empty_sentinel() {
        let json = br#"[{"id":"1","name":"Ubuntu","info_hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","size":"42","seeders":"12","leechers":"3","added":"1700000000"},{"id":"0","name":"No results returned","info_hash":"0000000000000000000000000000000000000000","size":"0","seeders":"0","leechers":"0","added":"0"}]"#;
        let rows = parse(json, "apibay").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].size, Some(42));
        assert_eq!(rows[0].seeders, Some(12));
        assert!(rows[0].published_at.is_some());
    }
}
