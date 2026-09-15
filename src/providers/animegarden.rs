use super::*;
use crate::model::parse_date;
use serde::Deserialize;

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    resources: Vec<Resource>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Resource {
    title: String,
    magnet: Option<String>,
    size: Option<u64>,
    created_at: Option<String>,
    href: Option<String>,
}

pub async fn search(provider: &HttpProvider, query: &str) -> Result<Vec<Torrent>, ProviderError> {
    let bytes = body(
        provider
            .client
            .post(&provider.config.url)
            .json(&serde_json::json!({"search": [query]})),
    )
    .await?;
    let response: Response = serde_json::from_slice(&bytes)
        .map_err(|_| ProviderError::data("invalid Anime Garden JSON response"))?;
    Ok(response
        .resources
        .into_iter()
        .map(|resource| Torrent {
            title: resource.title,
            magnet: resource.magnet.filter(|value| !value.is_empty()),
            size: resource.size,
            published_at: resource.created_at.as_deref().and_then(parse_date),
            detail_url: resource.href,
            sources: vec![provider.config.name.clone()],
            ..Default::default()
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cjk_resource_fields() {
        let body = r#"{"resources":[{"title":"[我推的孩子] OST","magnet":"magnet:?xt=urn:btih:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","size":42,"createdAt":"2026-09-15T04:00:00Z","href":"https://example.org/1"}]}"#
            .as_bytes();
        let response: Response = serde_json::from_slice(body).unwrap();
        assert_eq!(response.resources[0].title, "[我推的孩子] OST");
        assert_eq!(response.resources[0].size, Some(42));
        assert!(
            response.resources[0]
                .created_at
                .as_deref()
                .and_then(parse_date)
                .is_some()
        );
    }
}
