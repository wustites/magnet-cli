use crate::{model::Torrent, providers::Provider};
use futures::{StreamExt, stream};
use std::{collections::HashMap, time::Duration};

/// Aggregated output and provider diagnostics from a search.
pub struct SearchReport {
    /// Normalized and deduplicated search results.
    pub results: Vec<Torrent>,
    /// Safe-to-display provider warnings.
    pub warnings: Vec<String>,
    /// Number of providers that returned a valid response.
    pub successes: usize,
    /// Whether every failure was caused by transport or timeout errors.
    pub network_only: bool,
}

/// Controls provider scheduling and pagination.
#[derive(Clone, Copy, Debug)]
pub struct SearchOptions {
    /// Timeout applied separately to each provider.
    pub provider_timeout: Duration,
    /// Maximum number of provider searches polled concurrently.
    pub concurrency: usize,
    /// Optional deadline for the complete search, including queued providers.
    pub deadline: Option<Duration>,
    /// Pages requested from providers with pagination support.
    pub pages: u16,
}

impl Default for SearchOptions {
    fn default() -> Self {
        Self {
            provider_timeout: Duration::from_secs(15),
            concurrency: 8,
            deadline: None,
            pages: 1,
        }
    }
}

/// Searches with the historical defaults except for the supplied provider timeout.
pub async fn search(
    providers: &[Box<dyn Provider>],
    query: &str,
    timeout: Duration,
) -> SearchReport {
    search_with_options(
        providers,
        query,
        SearchOptions {
            provider_timeout: timeout,
            ..SearchOptions::default()
        },
    )
    .await
}

/// Searches providers concurrently with explicit scheduling options.
pub async fn search_with_options(
    providers: &[Box<dyn Provider>],
    query: &str,
    options: SearchOptions,
) -> SearchReport {
    let mut pending = stream::iter(providers.iter().enumerate().map(
        |(index, provider)| async move {
            let result = tokio::time::timeout(
                options.provider_timeout,
                provider.search_pages(query, options.pages.max(1)),
            )
            .await;
            (index, provider.name(), result)
        },
    ))
    .buffer_unordered(options.concurrency.max(1));
    let mut responses = Vec::with_capacity(providers.len());
    let mut deadline_reached = false;
    if let Some(after) = options.deadline {
        let deadline = tokio::time::Instant::now() + after;
        loop {
            match tokio::time::timeout_at(deadline, pending.next()).await {
                Ok(Some(response)) => responses.push(response),
                Ok(None) => break,
                Err(_) => {
                    deadline_reached = true;
                    break;
                }
            }
        }
    } else {
        responses = pending.by_ref().collect::<Vec<_>>().await;
    }
    drop(pending);
    let completed: std::collections::HashSet<_> =
        responses.iter().map(|(index, _, _)| *index).collect();
    // Preserve provider order so completion timing never changes the chosen title.
    responses.sort_by_key(|(index, _, _)| *index);
    let mut report = SearchReport {
        results: vec![],
        warnings: vec![],
        successes: 0,
        network_only: true,
    };
    for (_, name, response) in responses {
        match response {
            Ok(Ok(rows)) => {
                report.successes += 1;
                let mut invalid = 0;
                for mut row in rows {
                    if row.normalize().is_ok() {
                        report.results.push(row);
                    } else {
                        invalid += 1;
                    }
                }
                if invalid > 0 {
                    report
                        .warnings
                        .push(format!("{name}: skipped {invalid} invalid result(s)"));
                }
            }
            Ok(Err(error)) => {
                report.network_only &= error.network;
                report.warnings.push(format!("{name}: {}", error.message));
            }
            Err(_) => report.warnings.push(format!("{name}: request timed out")),
        }
    }
    if deadline_reached {
        for (index, provider) in providers.iter().enumerate() {
            if !completed.contains(&index) {
                report.warnings.push(format!(
                    "{}: total search deadline exceeded",
                    provider.name()
                ));
            }
        }
    }
    report.results = deduplicate(report.results);
    report
}

/// Merges rows that share a normalized BTIH while preserving provider priority.
pub fn deduplicate(rows: Vec<Torrent>) -> Vec<Torrent> {
    let mut results: Vec<Torrent> = Vec::new();
    let mut hashes = HashMap::<String, usize>::new();
    for row in rows {
        if let Some(index) = row.info_hash.as_ref().and_then(|hash| hashes.get(hash)) {
            let existing = &mut results[*index];
            existing.seeders = existing.seeders.max(row.seeders);
            existing.leechers = existing.leechers.max(row.leechers);
            existing.size = existing.size.or(row.size);
            existing.published_at = existing.published_at.or(row.published_at);
            existing.detail_url = existing.detail_url.take().or(row.detail_url);
            existing.sources.extend(row.sources);
            existing.sources.sort();
            existing.sources.dedup();
            existing.trackers.extend(row.trackers);
            existing.refresh_magnet();
        } else {
            if let Some(hash) = &row.info_hash {
                hashes.insert(hash.clone(), results.len());
            }
            results.push(row);
        }
    }
    results
}
