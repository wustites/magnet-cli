use magnet_cli::{
    model::{Torrent, normalize_hash, parse_size},
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
                title: self.name.into(),
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
    assert_eq!(report.results[0].title, "good");
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
    assert_eq!(report.results[0].title, "first");
}
