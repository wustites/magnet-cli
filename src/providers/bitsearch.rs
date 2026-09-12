use super::*;
use crate::model::parse_date;
use serde::Deserialize;

const PAGE_SIZE: u16 = 100;

#[derive(Deserialize)]
struct Response {
    success: bool,
    #[serde(default)]
    results: Vec<Hit>,
    pagination: Option<Pagination>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Hit {
    title: String,
    infohash: String,
    size: Option<u64>,
    seeders: Option<u32>,
    leechers: Option<u32>,
    created_at: Option<String>,
    updated_at: Option<String>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Pagination {
    has_next: bool,
}

pub async fn search(
    provider: &HttpProvider,
    query: &str,
    pages: u16,
) -> Result<Vec<Torrent>, ProviderError> {
    let api_key = provider
        .config
        .api_key_env
        .as_ref()
        .map(|name| {
            std::env::var(name)
                .map_err(|_| ProviderError::data("API key environment variable is not set"))
        })
        .transpose()?;
    let mut results = Vec::new();
    for page in 1..=pages.max(1) {
        let mut url = Url::parse(&provider.config.url)
            .map_err(|_| ProviderError::data("invalid Bitsearch URL"))?;
        let retained: Vec<_> = url
            .query_pairs()
            .filter(|(key, _)| !matches!(key.as_ref(), "q" | "page" | "limit" | "sort" | "order"))
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        url.set_query(None);
        url.query_pairs_mut().extend_pairs(retained);
        url.query_pairs_mut()
            .append_pair("q", query)
            .append_pair("page", &page.to_string())
            .append_pair("limit", &PAGE_SIZE.to_string())
            .append_pair("sort", "seeders")
            .append_pair("order", "desc");
        let mut request = provider.client.get(url);
        if let Some(key) = &api_key {
            request = request.header("x-api-key", key);
        }
        let bytes = body(request).await?;
        let response = parse_response(&bytes)?;
        if !response.success {
            return Err(ProviderError::data("Bitsearch API returned an error"));
        }
        let empty = response.results.is_empty();
        results.extend(response.results.into_iter().map(|hit| {
            Torrent {
                title: hit.title,
                info_hash: Some(hit.infohash),
                size: hit.size,
                seeders: hit.seeders,
                leechers: hit.leechers,
                published_at: hit
                    .created_at
                    .as_deref()
                    .or(hit.updated_at.as_deref())
                    .and_then(parse_date),
                sources: vec![provider.config.name.clone()],
                ..Default::default()
            }
        }));
        if empty || response.pagination.is_some_and(|value| !value.has_next) {
            break;
        }
    }
    Ok(results)
}

fn parse_response(bytes: &[u8]) -> Result<Response, ProviderError> {
    serde_json::from_slice(bytes)
        .map_err(|_| ProviderError::data("invalid Bitsearch JSON response"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_documented_and_live_timestamp_fields() {
        let documented = br#"{"success":true,"results":[{"title":"Ubuntu","infohash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","size":42,"seeders":5,"leechers":1,"createdAt":"2024-01-15T10:30:00Z"}],"pagination":{"hasNext":false}}"#;
        let response = parse_response(documented).unwrap();
        assert!(response.success);
        assert_eq!(response.results[0].seeders, Some(5));
        assert_eq!(
            response.results[0].created_at.as_deref(),
            Some("2024-01-15T10:30:00Z")
        );

        let live = br#"{"success":true,"results":[{"title":"Ubuntu","infohash":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","updatedAt":"2026-09-12T04:31:03.481Z"}],"pagination":{"hasNext":true}}"#;
        let response = parse_response(live).unwrap();
        assert_eq!(
            response.results[0].updated_at.as_deref(),
            Some("2026-09-12T04:31:03.481Z")
        );
        assert!(response.pagination.unwrap().has_next);
    }
}
