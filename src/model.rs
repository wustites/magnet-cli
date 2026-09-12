use anyhow::{Result, bail};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use url::Url;

#[derive(Clone, Debug, Default, Deserialize, Serialize)]
/// A normalized torrent search result.
pub struct Torrent {
    /// One-based position in a saved search, or zero before assignment.
    pub id: usize,
    /// Human-readable torrent title.
    pub title: String,
    /// Lowercase hexadecimal BitTorrent v1 infohash.
    pub info_hash: Option<String>,
    /// Canonical magnet URI rebuilt from the normalized fields.
    pub magnet: Option<String>,
    /// Content size in bytes.
    pub size: Option<u64>,
    /// Reported seed count.
    pub seeders: Option<u32>,
    /// Reported leecher count.
    pub leechers: Option<u32>,
    /// Publication time normalized to UTC.
    pub published_at: Option<DateTime<Utc>>,
    /// Provider names contributing to this result.
    pub sources: Vec<String>,
    /// Optional provider detail page.
    pub detail_url: Option<String>,
    /// Tracker announce URLs included in the magnet.
    pub trackers: Vec<String>,
}

/// Normalizes a hexadecimal or base32 BitTorrent v1 infohash.
pub fn normalize_hash(value: &str) -> Result<String> {
    let value = value.trim();
    if value.len() == 40 && value.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Ok(value.to_ascii_lowercase());
    }
    if value.len() == 32
        && let Ok(bytes) = data_encoding::BASE32.decode(value.to_ascii_uppercase().as_bytes())
    {
        return Ok(data_encoding::HEXLOWER.encode(&bytes));
    }
    bail!("expected a 40-character hexadecimal or 32-character base32 BTIH infohash")
}

impl Torrent {
    /// Validates and reconciles the explicit hash and magnet, then rebuilds the magnet.
    pub fn normalize(&mut self) -> Result<()> {
        let explicit = self.info_hash.as_deref().map(normalize_hash).transpose()?;
        let mut from_magnet = None;
        if let Some(raw) = &self.magnet {
            let uri = Url::parse(raw)?;
            if uri.scheme() != "magnet" {
                bail!("invalid magnet scheme");
            }
            for (key, value) in uri.query_pairs() {
                match key.as_ref() {
                    "xt" if value.to_ascii_lowercase().starts_with("urn:btih:") => {
                        let hash = normalize_hash(&value[9..])?;
                        if from_magnet.as_ref().is_some_and(|old| old != &hash) {
                            bail!("conflicting magnet hashes");
                        }
                        from_magnet = Some(hash);
                    }
                    "tr" => self.trackers.push(value.into_owned()),
                    "dn" if self.title.is_empty() => self.title = value.into_owned(),
                    _ => {}
                }
            }
            if from_magnet.is_none() {
                bail!("magnet has no BTIH infohash");
            }
        }
        if explicit.is_some() && from_magnet.is_some() && explicit != from_magnet {
            bail!("infohash disagrees with magnet");
        }
        self.info_hash = explicit.or(from_magnet);
        self.sources.sort();
        self.sources.dedup();
        self.refresh_magnet();
        Ok(())
    }

    /// Rebuilds the magnet from the current hash, title, and trackers.
    pub fn refresh_magnet(&mut self) {
        self.trackers.sort();
        self.trackers.dedup();
        if let Some(hash) = &self.info_hash {
            let mut uri = Url::parse("magnet:?").expect("constant URL");
            {
                let mut pairs = uri.query_pairs_mut();
                pairs.append_pair("xt", &format!("urn:btih:{hash}"));
                pairs.append_pair("dn", &self.title);
                for tracker in &self.trackers {
                    pairs.append_pair("tr", tracker);
                }
            }
            self.magnet = Some(uri.into());
        }
    }
}

/// Parses a byte count with optional decimal or binary size units.
pub fn parse_size(raw: &str) -> Result<u64, String> {
    let raw = raw.trim();
    if let Ok(bytes) = raw.parse::<u64>() {
        return Ok(bytes);
    }
    let split = raw
        .find(|c: char| !c.is_ascii_digit() && c != '.')
        .unwrap_or(raw.len());
    let number: f64 = raw[..split]
        .parse()
        .map_err(|_| "invalid size".to_string())?;
    let unit = raw[split..].trim().to_ascii_uppercase();
    let factor: f64 = match unit.as_str() {
        "" | "B" => 1.,
        "K" | "KB" => 1e3,
        "M" | "MB" => 1e6,
        "G" | "GB" => 1e9,
        "T" | "TB" => 1e12,
        "KIB" => 1024.,
        "MIB" => 1024_f64.powi(2),
        "GIB" => 1024_f64.powi(3),
        "TIB" => 1024_f64.powi(4),
        _ => return Err("use B, K/KB, M/MB, G/GB, T/TB or KiB/MiB/GiB/TiB".into()),
    };
    let bytes = number * factor;
    if !bytes.is_finite() || bytes < 0. || bytes >= u64::MAX as f64 {
        return Err("size out of range".into());
    }
    Ok(bytes as u64)
}

/// Parses an RFC 3339 or RFC 2822 timestamp and converts it to UTC.
pub fn parse_date(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .or_else(|_| DateTime::parse_from_rfc2822(raw))
        .map(|d| d.with_timezone(&Utc))
        .ok()
}
