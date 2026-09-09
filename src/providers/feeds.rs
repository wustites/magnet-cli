use super::*;
use crate::model::{parse_date, parse_size};
use roxmltree::Node;

pub async fn search(provider: &HttpProvider, query: &str) -> Result<Vec<Torrent>, ProviderError> {
    let config = &provider.config;
    let mut url = Url::parse(&config.url).map_err(|_| ProviderError::data("invalid feed URL"))?;
    if !matches!(config.kind, Kind::Rss) {
        let retained: Vec<_> = url
            .query_pairs()
            .filter(|(k, _)| {
                !matches!(k.as_ref(), "q" | "t" | "page")
                    && !(k == "apikey" && config.api_key_env.is_some())
            })
            .map(|(k, v)| (k.into_owned(), v.into_owned()))
            .collect();
        url.set_query(None);
        url.query_pairs_mut().extend_pairs(retained);
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("q", query);
        if matches!(config.kind, Kind::Nyaa | Kind::Sukebei) {
            pairs.append_pair("page", "rss");
        } else {
            pairs.append_pair("t", "search");
            pairs.append_pair("extended", "1");
            if let Some(var) = &config.api_key_env {
                let key = std::env::var(var)
                    .map_err(|_| ProviderError::data("API key environment variable is not set"))?;
                pairs.append_pair("apikey", &key);
            }
        }
    }
    let bytes = body(provider.client.get(url)).await?;
    let mut results = parse(&bytes, &config.name)?;
    if matches!(config.kind, Kind::Rss) {
        let terms: Vec<_> = query
            .to_lowercase()
            .split_whitespace()
            .map(str::to_owned)
            .collect();
        results.retain(|t| {
            terms
                .iter()
                .all(|term| t.title.to_lowercase().contains(term))
        });
    }
    Ok(results)
}
fn field<'a>(item: Node<'a, 'a>, name: &str) -> Option<&'a str> {
    item.children()
        .find(|n| n.is_element() && n.tag_name().name().eq_ignore_ascii_case(name))
        .and_then(|n| n.text())
        .map(str::trim)
        .or_else(|| {
            item.children()
                .find(|n| {
                    n.is_element()
                        && n.tag_name().name() == "attr"
                        && n.attribute("name")
                            .is_some_and(|v| v.eq_ignore_ascii_case(name))
                })
                .and_then(|n| n.attribute("value"))
        })
}
pub fn parse(bytes: &[u8], source: &str) -> Result<Vec<Torrent>, ProviderError> {
    let xml = std::str::from_utf8(bytes).map_err(|_| ProviderError::data("feed is not UTF-8"))?;
    let doc =
        roxmltree::Document::parse(xml).map_err(|_| ProviderError::data("invalid RSS XML"))?;
    if doc.root_element().tag_name().name() != "rss"
        || !doc
            .root_element()
            .children()
            .any(|n| n.has_tag_name("channel"))
    {
        return Err(ProviderError::data(
            "expected an RSS channel; provider may have returned an API error",
        ));
    }
    Ok(doc
        .descendants()
        .filter(|n| n.has_tag_name("item"))
        .filter_map(|item| {
            let title = field(item, "title")?.to_owned();
            let enclosure = item.children().find(|n| n.has_tag_name("enclosure"));
            let magnet = [
                field(item, "magneturl"),
                field(item, "link"),
                enclosure.and_then(|n| n.attribute("url")),
                field(item, "guid"),
            ]
            .into_iter()
            .flatten()
            .find(|v| v.starts_with("magnet:"))
            .map(str::to_owned);
            let size = field(item, "size")
                .and_then(|v| parse_size(v).ok())
                .or_else(|| {
                    enclosure
                        .and_then(|n| n.attribute("length"))
                        .and_then(|s| s.parse().ok())
                });
            let seeders: Option<u32> = field(item, "seeders").and_then(|s| s.parse().ok());
            let leechers = field(item, "leechers")
                .and_then(|s| s.parse().ok())
                .or_else(|| {
                    let peers: u32 = field(item, "peers")?.parse().ok()?;
                    peers.checked_sub(seeders?)
                });
            Some(Torrent {
                title,
                magnet,
                size,
                seeders,
                leechers,
                info_hash: field(item, "infohash")
                    .filter(|s| !s.is_empty())
                    .map(str::to_owned),
                published_at: field(item, "pubDate").and_then(parse_date),
                detail_url: [
                    field(item, "comments"),
                    field(item, "guid"),
                    field(item, "link"),
                ]
                .into_iter()
                .flatten()
                .find(|v| v.starts_with("https://") || v.starts_with("http://"))
                .map(str::to_owned),
                sources: vec![source.into()],
                ..Default::default()
            })
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn parses_namespaces_cdata_and_peer_counts() {
        let xml = br#"<rss xmlns:torznab="http://torznab.com/schemas/2015/feed"><channel><item><title><![CDATA[Ubuntu & Linux]]></title><torznab:attr name="infohash" value="aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"/><torznab:attr name="seeders" value="10"/><torznab:attr name="peers" value="13"/><enclosure length="123" url="https://example.org/a.torrent"/></item></channel></rss>"#;
        let rows = parse(xml, "test").unwrap();
        assert_eq!(rows[0].title, "Ubuntu & Linux");
        assert_eq!(rows[0].leechers, Some(3));
        assert_eq!(rows[0].size, Some(123));
    }
    #[test]
    fn rejects_error_and_html() {
        assert!(parse(b"<error code='100' description='secret'/>", "test").is_err());
        assert!(parse(b"<html/>", "test").is_err());
    }
}
