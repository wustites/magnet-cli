use super::*;
use crate::model::parse_date;
use serde::Deserialize;

#[derive(Deserialize)]
struct Response {
    hits: Vec<Hit>,
}
#[derive(Deserialize)]
struct Hit {
    title: String,
    hash: Option<String>,
    #[serde(rename = "magnetUrl")]
    magnet: Option<String>,
    bytes: Option<u64>,
    seeders: Option<u32>,
    date: Option<String>,
    details: Option<String>,
}

fn request(query: &str, offset: usize, size: usize) -> serde_json::Value {
    serde_json::json!({
        "query": query, "search_field": "title", "search_type": "100%",
        "order_by": "seeders", "order_direction": "desc",
        "from": offset, "size": size, "hide_unsafe": true
    })
}

pub async fn search(
    provider: &HttpProvider,
    query: &str,
    pages: u16,
) -> Result<Vec<Torrent>, ProviderError> {
    const PAGE_SIZE: usize = 150;
    let mut results = Vec::new();
    for page in 0..pages.max(1) as usize {
        let bytes = body(provider.client.post(&provider.config.url).json(&request(
            query,
            page * PAGE_SIZE,
            PAGE_SIZE,
        )))
        .await?;
        let rows = parse(&bytes, &provider.config.name)?;
        let empty = rows.is_empty();
        results.extend(rows);
        if empty {
            break;
        }
    }
    Ok(results)
}
fn parse(bytes: &[u8], source: &str) -> Result<Vec<Torrent>, ProviderError> {
    let response: Response = serde_json::from_slice(bytes)
        .map_err(|_| ProviderError::data("invalid Knaben JSON response"))?;
    Ok(response
        .hits
        .into_iter()
        .map(|hit| Torrent {
            title: hit.title,
            info_hash: hit.hash.filter(|s| !s.is_empty()),
            magnet: hit.magnet.filter(|s| !s.is_empty()),
            size: hit.bytes,
            seeders: hit.seeders,
            published_at: hit.date.as_deref().and_then(parse_date),
            sources: vec![source.into()],
            detail_url: hit.details,
            ..Default::default()
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paginated_requests_use_knaben_offsets() {
        assert_eq!(request("linux", 0, 150)["from"], 0);
        assert_eq!(request("linux", 150, 150)["from"], 150);
        assert_eq!(request("linux", 150, 150)["size"], 150);
    }
}
