use crate::{model::Torrent, providers::Provider};
use futures::{StreamExt, stream};
use std::{collections::HashMap, time::Duration};

pub struct SearchReport {
    pub results: Vec<Torrent>,
    pub warnings: Vec<String>,
    pub successes: usize,
    pub network_only: bool,
}

pub async fn search(
    providers: &[Box<dyn Provider>],
    query: &str,
    timeout: Duration,
) -> SearchReport {
    let mut responses = stream::iter(providers.iter().enumerate().map(
        |(index, provider)| async move {
            let result = tokio::time::timeout(timeout, provider.search(query)).await;
            (index, provider.name(), result)
        },
    ))
    .buffer_unordered(8)
    .collect::<Vec<_>>()
    .await;
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
    report.results = deduplicate(report.results);
    report
}

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
