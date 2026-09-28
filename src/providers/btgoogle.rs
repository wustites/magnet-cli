use super::*;
use crate::model::{parse_date, parse_size};
use chrono::{DateTime, Utc};

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

/// Decodes the entities BtGoogle emits, named and numeric, in a single pass so
/// that `&amp;lt;` yields the literal text `&lt;` instead of being decoded twice.
fn html_decode(value: &str) -> String {
    const NAMED: [(&str, char); 6] = [
        ("quot;", '"'),
        ("apos;", '\''),
        ("lt;", '<'),
        ("gt;", '>'),
        ("nbsp;", ' '),
        ("amp;", '&'),
    ];
    let mut out = String::with_capacity(value.len());
    let mut rest = value;
    while let Some(start) = rest.find('&') {
        out.push_str(&rest[..start]);
        rest = &rest[start + 1..];
        if let Some(digits) = rest.strip_prefix('#') {
            let radix = if digits.starts_with(['x', 'X']) {
                16
            } else {
                10
            };
            let body = if radix == 16 { &digits[1..] } else { digits };
            let end = body
                .find(|c: char| !c.is_digit(radix))
                .unwrap_or(body.len());
            if let Some(decoded) = u32::from_str_radix(&body[..end], radix)
                .ok()
                .and_then(char::from_u32)
            {
                out.push(decoded);
                // The closing `;` is optional in HTML but BtGoogle always sends it.
                rest = body[end..].strip_prefix(';').unwrap_or(&body[end..]);
                continue;
            }
        } else if let Some((entity, decoded)) =
            NAMED.iter().find(|(entity, _)| rest.starts_with(*entity))
        {
            out.push(*decoded);
            rest = &rest[entity.len()..];
            continue;
        }
        out.push('&');
    }
    out.push_str(rest);
    out
}

/// Picks the first `src` span that holds a date, so an added or removed tracker
/// label cannot shift the column BtGoogle reports publication dates in.
fn row_date(row: &str) -> Option<DateTime<Utc>> {
    const OPEN: &str = "<span class=\"src\">";
    const CLOSE: &str = "</span>";
    let mut rest = row;
    while let Some(start) = rest.find(OPEN) {
        let body = &rest[start + OPEN.len()..];
        let end = body.find(CLOSE)?;
        let value = html_decode(body[..end].trim());
        if let Some(date) = parse_date(&value).or_else(|| parse_date(&format!("{value}T00:00:00Z")))
        {
            return Some(date);
        }
        rest = &body[end..];
    }
    None
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
        // Rows stay unvalidated here on purpose: search_with_options normalizes
        // each one and reports a per-provider count, so a single malformed
        // magnet cannot discard every other row BtGoogle returned.
        results.push(Torrent {
            title,
            magnet: Some(magnet),
            size: between(row, "<span class=\"size\">", "</span>")
                .and_then(|value| parse_size(value.trim()).ok()),
            seeders: between(row, "<span class=\"seed", "</span>")
                .and_then(|value| value.split_once('>'))
                .and_then(|(_, value)| value.trim().parse().ok()),
            detail_url: between(row, "<a class=\"rname\" href=\"", "\"")
                .and_then(|href| base.join(href).ok().map(|url| url.to_string())),
            published_at: row_date(row),
            sources: vec![source.into()],
            ..Default::default()
        });
    }
    Ok(results)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_html(html: &str) -> Vec<Torrent> {
        parse(
            html.as_bytes(),
            "https://btgoogle.com/partials/search/results",
            "btgoogle",
        )
        .unwrap()
    }

    #[test]
    fn parses_cjk_title_magnet_and_fields() {
        let html = r#"<div class="row"><a class="rname" href="/info/AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA">子子西 &amp; 测试</a><span class="size">1.5 MB</span><span class="seed">7</span><span class="src">Sukebei</span><span class="src">2026-09-13</span><button data-magnet="magnet:?xt=urn:btih:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA&amp;tr=udp%3A%2F%2Ftracker.example%3A80%2Fannounce&#43;"></button></div>"#;
        let rows = parse_html(html);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].title, "子子西 & 测试");
        assert_eq!(rows[0].size, Some(1_500_000));
        assert_eq!(rows[0].seeders, Some(7));
        assert_eq!(rows[0].sources, vec!["btgoogle"]);
        assert_eq!(
            rows[0].detail_url.as_deref(),
            Some("https://btgoogle.com/info/AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA")
        );
        assert!(rows[0].published_at.is_some());
        // Normalization is the search layer's job, so the raw magnet survives.
        assert!(
            rows[0]
                .magnet
                .as_deref()
                .unwrap()
                .starts_with("magnet:?xt=urn:btih:AAAAAAAA")
        );
    }

    #[test]
    fn a_malformed_magnet_only_costs_that_row() {
        let good = format!(
            r#"<div class="row"><a class="rname" href="/good">Good</a><span class="src">Nyaa</span><button data-magnet="magnet:?xt=urn:btih:{}&amp;dn=Good"></button></div>"#,
            "a".repeat(40)
        );
        let bad = r#"<div class="row"><a class="rname" href="/bad">Bad</a><span class="src">Nyaa</span><button data-magnet="magnet:?xt=urn:btih:NOTAHASH"></button></div>"#;
        let rows = parse_html(&format!("{good}{bad}"));
        // Both rows reach the caller; only the bad one fails to normalize.
        assert_eq!(rows.len(), 2);
        let mut kept = Vec::new();
        let mut invalid = 0;
        for mut row in rows {
            if row.normalize().is_ok() {
                kept.push(row);
            } else {
                invalid += 1;
            }
        }
        assert_eq!(invalid, 1);
        assert_eq!(kept.len(), 1);
        assert_eq!(kept[0].title, "Good");
        assert_eq!(kept[0].info_hash.as_deref(), Some("a".repeat(40).as_str()));
    }

    #[test]
    fn decodes_nested_and_numeric_entities_once() {
        assert_eq!(html_decode("&amp;lt;"), "&lt;");
        assert_eq!(html_decode("&lt;tag&gt;"), "<tag>");
        assert_eq!(html_decode("&#43;&#x2b;&#38;&#47;"), "++&/");
        assert_eq!(html_decode("a & b"), "a & b");
        assert_eq!(html_decode("&unknown;"), "&unknown;");
        assert_eq!(html_decode("&#1114112;"), "&#1114112;");
    }

    #[test]
    fn the_date_column_is_found_by_value_not_position() {
        let extra = r#"<span class="src">Nyaa</span><span class="src">Sukebei</span><span class="src">2026-09-13</span>"#;
        let html = format!(
            r#"<div class="row"><a class="rname" href="/x">Row</a>{extra}<button data-magnet="magnet:?xt=urn:btih:{}"></button></div>"#,
            "b".repeat(40)
        );
        let rows = parse_html(&html);
        assert!(
            rows[0].published_at.is_some(),
            "a third src span must not shift the parsed date"
        );
    }
}
