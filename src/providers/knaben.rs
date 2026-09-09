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

pub async fn search(provider: &HttpProvider, query: &str) -> Result<Vec<Torrent>, ProviderError> {
    let bytes = body(
        provider
            .client
            .post(&provider.config.url)
            .json(&serde_json::json!({
                "query": query, "search_field": "title", "search_type": "100%",
                "order_by": "seeders", "order_direction": "desc", "size": 150,
                "hide_unsafe": true
            })),
    )
    .await?;
    parse(&bytes, &provider.config.name)
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
