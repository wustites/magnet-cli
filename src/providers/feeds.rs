use super::*;
use crate::model::{parse_date, parse_size};
use roxmltree::Node;

pub async fn search(
    provider: &HttpProvider,
    query: &str,
    pages: u16,
) -> Result<Vec<Torrent>, ProviderError> {
    let config = &provider.config;
    let requested_pages = if matches!(config.kind, Kind::Torznab) {
        pages.max(1)
    } else {
        1
    };
    let mut results = Vec::new();
    for page in 0..requested_pages {
        let mut url =
            Url::parse(&config.url).map_err(|_| ProviderError::data("invalid feed URL"))?;
        if !matches!(config.kind, Kind::Rss) {
            let retained: Vec<_> = url
                .query_pairs()
                .filter(|(k, _)| {
                    !matches!(k.as_ref(), "q" | "t" | "page" | "offset" | "limit")
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
                const PAGE_SIZE: u32 = 100;
                pairs.append_pair("t", "search");
                pairs.append_pair("extended", "1");
                pairs.append_pair("limit", &PAGE_SIZE.to_string());
                pairs.append_pair("offset", &(u32::from(page) * PAGE_SIZE).to_string());
                if let Some(var) = &config.api_key_env {
                    let key = std::env::var(var).map_err(|_| {
                        ProviderError::data("API key environment variable is not set")
                    })?;
                    pairs.append_pair("apikey", &key);
                }
            }
        }
        let bytes = body(provider.client.get(url)).await?;
        let rows = parse(&bytes, &config.name)?;
        let empty = rows.is_empty();
        results.extend(rows);
        if empty {
            break;
        }
    }
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
fn link_href<'a>(item: Node<'a, 'a>, relation: Option<&str>) -> Option<&'a str> {
    item.children()
        .filter(|n| n.is_element() && n.tag_name().name().eq_ignore_ascii_case("link"))
        .find(|n| {
            relation.is_none_or(|expected| {
                n.attribute("rel")
                    .is_some_and(|actual| actual.eq_ignore_ascii_case(expected))
            })
        })
        .and_then(|n| n.attribute("href"))
}
pub fn parse(bytes: &[u8], source: &str) -> Result<Vec<Torrent>, ProviderError> {
    let xml = std::str::from_utf8(bytes).map_err(|_| ProviderError::data("feed is not UTF-8"))?;
    let doc =
        roxmltree::Document::parse(xml).map_err(|_| ProviderError::data("invalid RSS/Atom XML"))?;
    let root = doc.root_element();
    let rss = root.tag_name().name().eq_ignore_ascii_case("rss")
        && root
            .children()
            .any(|n| n.tag_name().name().eq_ignore_ascii_case("channel"));
    let atom = root.tag_name().name().eq_ignore_ascii_case("feed");
    if !rss && !atom {
        return Err(ProviderError::data(
            "expected an RSS channel or Atom feed; provider may have returned an API error",
        ));
    }
    Ok(doc
        .descendants()
        .filter(|n| {
            n.is_element()
                && matches!(
                    n.tag_name().name().to_ascii_lowercase().as_str(),
                    "item" | "entry"
                )
        })
        .filter_map(|item| {
            let title = field(item, "title")?.to_owned();
            let enclosure = item.children().find(|n| {
                n.is_element()
                    && (n.tag_name().name().eq_ignore_ascii_case("enclosure")
                        || (n.tag_name().name().eq_ignore_ascii_case("link")
                            && n.attribute("rel")
                                .is_some_and(|rel| rel.eq_ignore_ascii_case("enclosure"))))
            });
            let magnet = [
                field(item, "magneturl"),
                field(item, "link"),
                link_href(item, Some("enclosure")),
                link_href(item, None),
                enclosure.and_then(|n| n.attribute("url")),
                enclosure.and_then(|n| n.attribute("href")),
                field(item, "guid"),
                field(item, "id"),
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
                detail_url: [
                    field(item, "comments"),
                    field(item, "guid"),
                    field(item, "link"),
                    link_href(item, Some("alternate")),
                    link_href(item, None),
                ]
                .into_iter()
                .flatten()
                .find(|v| v.starts_with("https://") || v.starts_with("http://"))
                .map(str::to_owned),
                published_at: field(item, "pubDate")
                    .or_else(|| field(item, "published"))
                    .or_else(|| field(item, "updated"))
                    .and_then(parse_date),
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
    #[test]
    fn parses_atom_entries_and_magnet_links() {
        let xml = br#"<feed xmlns="http://www.w3.org/2005/Atom"><entry><title>Ubuntu Atom</title><id>tag:example,1</id><published>2026-01-02T03:04:05Z</published><link rel="alternate" href="https://example.org/1"/><link rel="enclosure" href="magnet:?xt=urn:btih:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa" length="42"/></entry></feed>"#;
        let rows = parse(xml, "atom").unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "Ubuntu Atom");
        assert_eq!(rows[0].size, Some(42));
        assert_eq!(rows[0].detail_url.as_deref(), Some("https://example.org/1"));
        assert!(rows[0].published_at.is_some());
        assert!(rows[0].magnet.as_deref().unwrap().starts_with("magnet:"));
    }
}
