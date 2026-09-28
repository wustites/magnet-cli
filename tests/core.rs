use magnet_cli::{
    model::{Torrent, matches_query, normalize_hash, parse_size},
    search::deduplicate,
};

#[test]
fn hashes_merge_across_encodings_and_union_trackers() {
    let hex = "0000000000000000000000000000000000000000";
    assert_eq!(
        normalize_hash("AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA").unwrap(),
        hex
    );
    let mut a = Torrent {
        title: "Ubuntu & Linux".into(),
        info_hash: Some(hex.into()),
        sources: vec!["a".into()],
        seeders: Some(10),
        magnet: Some(format!("magnet:?xt=urn:btih:{hex}&tr=udp%3A%2F%2Fa")),
        ..Default::default()
    };
    let mut b = Torrent {
        title: "Other title".into(),
        magnet: Some(
            "magnet:?xt=urn:btih:AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA&tr=udp%3A%2F%2Fb".into(),
        ),
        seeders: Some(20),
        sources: vec!["b".into()],
        ..Default::default()
    };
    a.normalize().unwrap();
    b.normalize().unwrap();
    let rows = deduplicate(vec![a, b]);
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].seeders, Some(20));
    assert_eq!(rows[0].sources, vec!["a", "b"]);
    assert_eq!(rows[0].trackers, vec!["udp://a", "udp://b"]);
    let uri = url::Url::parse(rows[0].magnet.as_ref().unwrap()).unwrap();
    assert!(
        uri.query_pairs()
            .any(|(k, v)| k == "dn" && v == "Ubuntu & Linux")
    );
}
#[test]
fn unknown_hashes_do_not_merge_by_title_and_conflicts_fail() {
    let row = Torrent {
        title: "same".into(),
        ..Default::default()
    };
    assert_eq!(deduplicate(vec![row.clone(), row]).len(), 2);
    let mut bad = Torrent {
        info_hash: Some("a".repeat(40)),
        magnet: Some(format!("magnet:?xt=urn:btih:{}", "b".repeat(40))),
        ..Default::default()
    };
    assert!(bad.normalize().is_err());
    assert!(normalize_hash("not-a-hash").is_err());
}
#[test]
fn size_units_and_overflow() {
    assert_eq!(parse_size("10G").unwrap(), 10_000_000_000);
    assert_eq!(parse_size("1.5 GiB").unwrap(), 1_610_612_736);
    assert_eq!(parse_size("18446744073709551615").unwrap(), u64::MAX);
    for bad in ["-1", "NaN", "1XB", "18446744073709551616", "1e99"] {
        assert!(parse_size(bad).is_err(), "{bad}");
    }
}

struct Stub {
    name: &'static str,
    failure: bool,
    delay: std::time::Duration,
}
#[async_trait::async_trait]
impl magnet_cli::providers::Provider for Stub {
    fn name(&self) -> &str {
        self.name
    }
    async fn search(&self, _: &str) -> Result<Vec<Torrent>, magnet_cli::providers::ProviderError> {
        tokio::time::sleep(self.delay).await;
        if self.failure {
            Err(magnet_cli::providers::ProviderError {
                message: "fixture failure".into(),
                network: false,
            })
        } else {
            Ok(vec![Torrent {
                // The shared title filter requires every query term, so a
                // fixture row has to mention the query it is searched for.
                title: format!("ubuntu {}", self.name),
                info_hash: Some("a".repeat(40)),
                sources: vec![self.name.into()],
                ..Default::default()
            }])
        }
    }
}
#[tokio::test]
async fn partial_failures_and_deadlines_keep_results() {
    use magnet_cli::{providers::Provider, search::search};
    use std::time::Duration;
    let providers: Vec<Box<dyn Provider>> = vec![
        Box::new(Stub {
            name: "slow",
            failure: false,
            delay: Duration::from_secs(10),
        }),
        Box::new(Stub {
            name: "broken",
            failure: true,
            delay: Duration::ZERO,
        }),
        Box::new(Stub {
            name: "good",
            failure: false,
            delay: Duration::ZERO,
        }),
    ];
    let report = search(&providers, "ubuntu", Duration::from_millis(50)).await;
    assert_eq!(report.successes, 1);
    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].title, "ubuntu good");
    assert_eq!(report.warnings.len(), 2);
    assert!(!report.network_only);
}
#[tokio::test]
async fn completion_order_does_not_change_preferred_title() {
    use magnet_cli::{providers::Provider, search::search};
    use std::time::Duration;
    let providers: Vec<Box<dyn Provider>> = vec![
        Box::new(Stub {
            name: "first",
            failure: false,
            delay: Duration::from_millis(20),
        }),
        Box::new(Stub {
            name: "second",
            failure: false,
            delay: Duration::ZERO,
        }),
    ];
    let report = search(&providers, "ubuntu", Duration::from_secs(1)).await;
    assert_eq!(report.successes, 2);
    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].title, "ubuntu first");
}

#[tokio::test]
async fn total_deadline_keeps_completed_results_and_cancels_the_rest() {
    use magnet_cli::{providers::Provider, search::search_with_options};
    use std::time::Duration;
    let providers: Vec<Box<dyn Provider>> = vec![
        Box::new(Stub {
            name: "good",
            failure: false,
            delay: Duration::ZERO,
        }),
        Box::new(Stub {
            name: "slow",
            failure: false,
            delay: Duration::from_secs(10),
        }),
    ];
    let report = search_with_options(
        &providers,
        "ubuntu",
        magnet_cli::search::SearchOptions {
            provider_timeout: Duration::from_secs(20),
            concurrency: 1,
            deadline: Some(Duration::from_millis(50)),
            pages: 1,
            title_filter: true,
        },
    )
    .await;
    assert_eq!(report.successes, 1);
    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].title, "ubuntu good");
    assert!(
        report
            .warnings
            .iter()
            .any(|warning| warning == "slow: total search deadline exceeded")
    );
}

#[test]
fn title_filter_requires_every_term_regardless_of_script() {
    // CJK, Latin, mixed case and multi-term queries all behave the same way.
    assert!(matches_query("台湾热门女神苏畅", "苏畅"));
    assert!(!matches_query("Spider-Man: Brand New Day", "苏畅"));
    assert!(matches_query("Big.Buck.Bunny.2008.1080p", "big buck bunny"));
    assert!(!matches_query("Big.Buck.Bunny.2008.1080p", "big buck tori"));
    assert!(matches_query("Microsoft Visual C++ Redistributable", "c++"));
    assert!(matches_query("anything at all", ""));
}

struct Titled {
    name: &'static str,
    title: &'static str,
    /// Distinct per row and per provider so dedup never merges the fixtures.
    hashes: [char; 2],
    delay: std::time::Duration,
}
#[async_trait::async_trait]
impl magnet_cli::providers::Provider for Titled {
    fn name(&self) -> &str {
        self.name
    }
    async fn search(&self, _: &str) -> Result<Vec<Torrent>, magnet_cli::providers::ProviderError> {
        tokio::time::sleep(self.delay).await;
        Ok(vec![
            Torrent {
                title: self.title.into(),
                info_hash: Some(self.hashes[0].to_string().repeat(40)),
                sources: vec![self.name.into()],
                ..Default::default()
            },
            Torrent {
                // What an index returns in place of a real match.
                title: "upstream popular filler".into(),
                info_hash: Some(self.hashes[1].to_string().repeat(40)),
                sources: vec![self.name.into()],
                ..Default::default()
            },
        ])
    }
}

fn titled_providers() -> Vec<Box<dyn magnet_cli::providers::Provider>> {
    vec![
        Box::new(Titled {
            name: "filler",
            title: "子子西 合集",
            hashes: ['a', 'b'],
            delay: std::time::Duration::ZERO,
        }),
        Box::new(Titled {
            name: "untitled",
            title: "",
            hashes: ['c', 'd'],
            delay: std::time::Duration::ZERO,
        }),
    ]
}

#[tokio::test]
async fn the_title_filter_drops_filler_and_reports_it() {
    use magnet_cli::search::search_with_options;
    use std::time::Duration;
    let report = search_with_options(
        &titled_providers(),
        "子子西",
        magnet_cli::search::SearchOptions {
            provider_timeout: Duration::from_secs(5),
            title_filter: true,
            ..Default::default()
        },
    )
    .await;
    // Only the row mentioning every query term survives, and a row with no
    // title can never satisfy the query.
    assert_eq!(report.results.len(), 1);
    assert_eq!(report.results[0].title, "子子西 合集");
    // Every drop is reported per provider, so nothing is lost silently.
    assert!(
        report
            .warnings
            .contains(&"filler: dropped 1 result(s) that do not match the query".into())
    );
    assert!(
        report
            .warnings
            .contains(&"untitled: dropped 2 result(s) that do not match the query".into()),
        "{:?}",
        report.warnings
    );
}

#[tokio::test]
async fn the_title_filter_can_be_disabled() {
    use magnet_cli::search::search_with_options;
    use std::time::Duration;
    let report = search_with_options(
        &titled_providers(),
        "子子西",
        magnet_cli::search::SearchOptions {
            provider_timeout: Duration::from_secs(5),
            title_filter: false,
            ..Default::default()
        },
    )
    .await;
    // 2 providers x 2 rows, and nothing was withheld.
    assert_eq!(report.results.len(), 4);
    assert!(!report.warnings.iter().any(|w| w.contains("do not match")));
}

#[test]
fn cache_round_trip_replaces_old_snapshot_and_reports_corruption() {
    use magnet_cli::cache;
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("nested/cache.json");
    let first = Torrent {
        id: 1,
        title: "first".into(),
        ..Default::default()
    };
    let second = Torrent {
        id: 1,
        title: "second".into(),
        ..Default::default()
    };
    cache::save(&path, &[first]).unwrap();
    cache::save(&path, &[second]).unwrap();
    assert_eq!(cache::read(&path).unwrap()[0].title, "second");
    std::fs::write(&path, "not json").unwrap();
    assert!(
        cache::read(&path)
            .unwrap_err()
            .to_string()
            .contains("invalid")
    );
}
