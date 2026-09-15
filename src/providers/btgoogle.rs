use super::*;
use crate::model::{parse_date, parse_size};

pub async fn search(
    provider: &HttpProvider,
    query: &str,
    pages: u16,
) -> Result<Vec<Torrent>, ProviderError> {
    let mut results = Vec::new();
    for page in 1..=pages.max(1) {
        let mut url = Url::parse(&provider.config.url)
            .map_err(|_| ProviderError::data("invalid BtGoogle URL"))?;
        let retained: Vec<_> = url
            .query_pairs()
            .filter(|(key, _)| !matches!(key.as_ref(), "q" | "sort" | "page"))
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect();
        url.set_query(None);
        url.query_pairs_mut()
            .extend_pairs(retained)
            .append_pair("q", query)
            .append_pair("sort", "relevance")
            .append_pair("page", &page.to_string());

        let bytes = body(provider.client.get(url)).await?;
        let page_results = parse(&bytes, &provider.config.url, &provider.config.name)?;
        let empty = page_results.is_empty();
        results.extend(page_results);
        if empty {
            break;
        }
    }
    Ok(results)
}

fn between<'a>(value: &'a str, start: &str, end: &str) -> Option<&'a str> {
    let start = value.find(start)? + start.len();
    let end = value[start..].find(end)? + start;
    Some(&value[start..end])
}

fn nth_between<'a>(value: &'a str, start: &str, end: &str, occurrence: usize) -> Option<&'a str> {
    let mut offset = 0;
    for _ in 0..occurrence {
        let relative = value[offset..].find(start)?;
        offset += relative + start.len();
    }
    let end = value[offset..].find(end)? + offset;
    Some(&value[offset..end])
}

fn html_decode(value: &str) -> String {
    value
        .replace("&amp;", "&")
        .replace("&quot;", "\"")
        .replace("&#39;", "'")
        .replace("&#43;", "+")
        .replace("&#x2b;", "+")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
}

pub fn parse(bytes: &[u8], endpoint: &str, source: &str) -> Result<Vec<Torrent>, ProviderError> {
    let html = std::str::from_utf8(bytes)
        .map_err(|_| ProviderError::data("BtGoogle response is not UTF-8"))?;
    let starts: Vec<_> = html
        .match_indices("<div class=\"row\">")
        .map(|(i, _)| i)
        .collect();
    let base = Url::parse(endpoint).map_err(|_| ProviderError::data("invalid BtGoogle URL"))?;
    let mut results = Vec::new();
    for (index, start) in starts.iter().enumerate() {
        let end = starts.get(index + 1).copied().unwrap_or(html.len());
        let row = &html[*start..end];
        let title = between(row, "<a class=\"rname\"", "</a>")
            .and_then(|value| value.split_once('>'))
            .map(|(_, value)| html_decode(value.trim()))
            .filter(|value| !value.is_empty());
        let magnet = between(row, "data-magnet=\"", "\"").map(html_decode);
        let Some(title) = title else { continue };
        let Some(magnet) = magnet else { continue };
        let mut torrent = Torrent {
            title,
            magnet: Some(magnet),
            size: between(row, "<span class=\"size\">", "</span>")
                .and_then(|value| parse_size(value.trim()).ok()),
            seeders: between(row, "<span class=\"seed", "</span>")
                .and_then(|value| value.split_once('>'))
                .and_then(|(_, value)| value.trim().parse().ok()),
            detail_url: between(row, "<a class=\"rname\" href=\"", "\"")
                .and_then(|href| base.join(href).ok().map(|url| url.to_string())),
            published_at: nth_between(row, "<span class=\"src\">", "</span>", 2)
                .and_then(|value| parse_date(&format!("{value}T00:00:00Z"))),
            sources: vec![source.into()],
            ..Default::default()
        };
        torrent
            .normalize()
            .map_err(|_| ProviderError::data("BtGoogle returned an invalid magnet"))?;
        results.push(torrent);
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_cjk_title_magnet_and_fields() {
        let html = r#"<div class="row"><a class="rname" href="/info/AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA">子子西 &amp; 测试</a><span class="size">1.5 MB</span><span class="seed">7</span><span class="src">Sukebei</span><span class="src">2026-09-13</span><button data-magnet="magnet:?xt=urn:btih:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA&amp;tr=udp%3A%2F%2Ftracker.example%3A80%2Fannounce&#43;"></button></div>"#;
        let rows = parse(
            html.as_bytes(),
            "https://btgoogle.com/partials/search/results",
            "btgoogle",
        )
        .unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "子子西 & 测试");
        assert_eq!(
            rows[0].info_hash.as_deref(),
            Some("aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa")
        );
        assert_eq!(rows[0].size, Some(1_500_000));
        assert_eq!(rows[0].seeders, Some(7));
        assert_eq!(rows[0].sources, vec!["btgoogle"]);
    }
}
