use serde_json::Value;
use std::{
    io::{Read, Write},
    net::TcpListener,
    process::{Command, Output},
    sync::{Arc, Mutex},
    thread,
};

fn server(body: String, expected: &'static str) -> (String, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let handle = thread::spawn(move || {
        let (mut socket, _) = listener.accept().unwrap();
        socket
            .set_read_timeout(Some(std::time::Duration::from_secs(5)))
            .unwrap();
        let mut request = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            let n = socket.read(&mut buffer).unwrap();
            if n == 0 {
                break;
            }
            request.extend_from_slice(&buffer[..n]);
            if request.windows(4).any(|w| w == b"\r\n\r\n") {
                break;
            }
        }
        assert!(
            String::from_utf8_lossy(&request).contains(expected),
            "{}",
            String::from_utf8_lossy(&request)
        );
        write!(
            socket,
            "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
            body.len(),
            body
        )
        .unwrap();
    });
    (url, handle)
}
fn paged_server(bodies: Vec<String>) -> (String, Arc<Mutex<Vec<String>>>, thread::JoinHandle<()>) {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    let requests = Arc::new(Mutex::new(Vec::new()));
    let captured = Arc::clone(&requests);
    let handle = thread::spawn(move || {
        for body in bodies {
            let (mut socket, _) = listener.accept().unwrap();
            socket
                .set_read_timeout(Some(std::time::Duration::from_secs(5)))
                .unwrap();
            let mut request = Vec::new();
            let mut buffer = [0u8; 4096];
            loop {
                let n = socket.read(&mut buffer).unwrap();
                if n == 0 {
                    break;
                }
                request.extend_from_slice(&buffer[..n]);
                if request.windows(4).any(|w| w == b"\r\n\r\n") {
                    break;
                }
            }
            captured
                .lock()
                .unwrap()
                .push(String::from_utf8_lossy(&request).into_owned());
            write!(
                socket,
                "HTTP/1.1 200 OK\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                body.len(),
                body
            )
            .unwrap();
        }
    });
    (url, requests, handle)
}
fn run(dir: &std::path::Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_magnet"))
        .arg("--cache")
        .arg(dir.join("cache.json"))
        .args(args)
        .output()
        .unwrap()
}
#[test]
fn aggregates_http_providers_and_gets_cached_id() {
    let hash = "a".repeat(40);
    let (nyaa, n) = server(
        format!(
            r#"<rss xmlns:nyaa="https://nyaa.si/xmlns/nyaa"><channel><item><title>Ubuntu Linux</title><nyaa:infoHash>{hash}</nyaa:infoHash><nyaa:seeders>4</nyaa:seeders><nyaa:size>1 GiB</nyaa:size><link>magnet:?xt=urn:btih:{hash}&amp;tr=udp%3A%2F%2Fa</link></item></channel></rss>"#
        ),
        "page=rss",
    );
    let (knaben,k) = server(serde_json::json!({"hits":[{"title":"Linux Ubuntu","hash":hash,"seeders":20,"bytes":1073741824,"magnetUrl":format!("magnet:?xt=urn:btih:{hash}&tr=udp%3A%2F%2Fb") }]}).to_string(),"POST /");
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.toml");
    std::fs::write(&config,format!("[[providers]]\nname='nyaa'\nkind='nyaa'\nurl='{nyaa}'\n[[providers]]\nname='knaben'\nkind='knaben'\nurl='{knaben}'\n")).unwrap();
    let output = run(
        dir.path(),
        &[
            "--config",
            config.to_str().unwrap(),
            "search",
            "Ubuntu Linux",
            "--json",
            "--min-seeds",
            "10",
            "--max-size",
            "2G",
        ],
    );
    n.join().unwrap();
    k.join().unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 1);
    assert_eq!(rows[0]["seeders"], 20);
    assert_eq!(rows[0]["trackers"].as_array().unwrap().len(), 2);
    assert_eq!(rows[0]["sources"], serde_json::json!(["knaben", "nyaa"]));
    let get = run(dir.path(), &["get", "1"]);
    assert_eq!(get.status.code(), Some(0));
    assert_eq!(
        String::from_utf8(get.stdout).unwrap().trim(),
        rows[0]["magnet"].as_str().unwrap()
    );
}
#[test]
fn rss_filters_locally_and_empty_search_clears_snapshot() {
    let (url, handle) = server(
        "<rss><channel><item><title>Other title</title></item></channel></rss>".into(),
        "GET /",
    );
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.toml");
    std::fs::write(
        &config,
        format!("[[providers]]\nname='feed'\nkind='rss'\nurl='{url}'"),
    )
    .unwrap();
    let output = run(
        dir.path(),
        &[
            "--config",
            config.to_str().unwrap(),
            "search",
            "Ubuntu",
            "--json",
        ],
    );
    handle.join().unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(output.stdout, b"[]\n");
    assert_eq!(run(dir.path(), &["get", "1"]).status.code(), Some(1));
}
#[test]
fn api_errors_are_not_empty_successes_and_secrets_are_redacted() {
    let (url, handle) = server(
        "<error code='100' description='secret-value'/>".into(),
        "t=search",
    );
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.toml");
    std::fs::write(
        &config,
        format!(
            "[[providers]]\nname='private'\nkind='torznab'\nurl='{url}/api?apikey=secret-value'"
        ),
    )
    .unwrap();
    let output = run(
        dir.path(),
        &[
            "--config",
            config.to_str().unwrap(),
            "search",
            "Ubuntu",
            "--json",
        ],
    );
    handle.join().unwrap();
    assert_eq!(output.status.code(), Some(2));
    assert_eq!(output.stdout, b"[]\n");
    assert!(!String::from_utf8_lossy(&output.stderr).contains("secret-value"));
}
#[test]
fn argument_errors_and_offline_resolver() {
    let dir = tempfile::tempdir().unwrap();
    for args in [
        vec!["search", "x", "--json", "--magnet"],
        vec!["search", "x", "--limit", "0"],
        vec!["search", "x", "--concurrency", "0"],
        vec!["search", "x", "--deadline", "0"],
        vec!["search", "x", "--pages", "0"],
        vec!["get", "0"],
        vec!["resolve", "bad"],
    ] {
        assert_eq!(run(dir.path(), &args).status.code(), Some(3));
    }
    let output = run(
        dir.path(),
        &[
            "resolve",
            "AAAAAAAAAAAAAAAAAAAAAAAAAAAAAAAA",
            "--name",
            "Ubuntu & Linux",
            "--json",
        ],
    );
    assert_eq!(output.status.code(), Some(0));
    let row: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(row["info_hash"], "0".repeat(40));
}

#[test]
fn transport_failure_uses_exit_four_and_jsonl_stays_empty() {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.toml");
    std::fs::write(
        &config,
        format!("[[providers]]\nname='offline'\nkind='rss'\nurl='{url}'"),
    )
    .unwrap();
    let output = run(
        dir.path(),
        &[
            "--config",
            config.to_str().unwrap(),
            "search",
            "Ubuntu",
            "--jsonl",
            "--timeout",
            "1",
        ],
    );
    assert_eq!(output.status.code(), Some(4));
    assert!(output.stdout.is_empty());
}

#[test]
fn sukebei_is_selectable_and_uses_nyaa_rss_protocol() {
    let hash = "b".repeat(40);
    let (url, handle) = server(
        format!(
            r#"<rss xmlns:nyaa="https://sukebei.nyaa.si/xmlns/nyaa"><channel><item><title>Fixture</title><nyaa:infoHash>{hash}</nyaa:infoHash><nyaa:seeders>3</nyaa:seeders></item></channel></rss>"#
        ),
        "q=fixture&page=rss",
    );
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.toml");
    std::fs::write(
        &config,
        format!(
            "[[providers]]\nname='sukebei'\nkind='sukebei'\nurl='{url}'\ndefault_search=false\n"
        ),
    )
    .unwrap();
    let default = run(
        dir.path(),
        &[
            "--config",
            config.to_str().unwrap(),
            "search",
            "fixture",
            "--json",
        ],
    );
    assert_eq!(default.status.code(), Some(3));
    let output = run(
        dir.path(),
        &[
            "--config",
            config.to_str().unwrap(),
            "search",
            "fixture",
            "--source",
            "sukebei",
            "--json",
        ],
    );
    handle.join().unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(rows[0]["sources"], serde_json::json!(["sukebei"]));
    assert_eq!(rows[0]["info_hash"], hash);
    assert!(rows[0]["magnet"].as_str().unwrap().starts_with("magnet:?"));
}

#[test]
fn builtin_sukebei_participates_in_default_search() {
    let dir = tempfile::tempdir().unwrap();
    let output = run(dir.path(), &["providers", "--json"]);
    assert_eq!(output.status.code(), Some(0));
    let rows: Value = serde_json::from_slice(&output.stdout).unwrap();
    let rows = rows.as_array().unwrap();
    let sukebei = rows.iter().find(|row| row["name"] == "sukebei").unwrap();
    assert_eq!(sukebei["kind"], "sukebei");
    assert_eq!(sukebei["default_search"], true);
    assert_eq!(
        rows.iter()
            .filter(|row| row["default_search"] == true)
            .count(),
        5
    );
    for name in ["nyaa", "knaben", "sukebei", "apibay", "bitsearch"] {
        assert!(rows.iter().any(|row| row["name"] == name));
    }
}

#[test]
fn torznab_paginates_with_offsets() {
    let item =
        |title: &str| format!("<rss><channel><item><title>{title}</title></item></channel></rss>");
    let (url, requests, handle) = paged_server(vec![item("First"), item("Second")]);
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.toml");
    std::fs::write(
        &config,
        format!("[[providers]]\nname='local'\nkind='torznab'\nurl='{url}'"),
    )
    .unwrap();
    let output = run(
        dir.path(),
        &[
            "--config",
            config.to_str().unwrap(),
            "search",
            "linux",
            "--pages",
            "2",
            "--json",
        ],
    );
    handle.join().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let rows: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 2);
    let requests = requests.lock().unwrap();
    assert!(
        requests[0].contains("offset=0&limit=100") || requests[0].contains("limit=100&offset=0")
    );
    assert!(requests[1].contains("offset=100"));
}

#[test]
fn jsonl_sort_limit_and_snapshot_are_consistent() {
    let (url, handle) = server(
        "<rss><channel><item><title>Zulu Linux</title></item><item><title>Alpha Linux</title></item></channel></rss>".into(),
        "GET /",
    );
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.toml");
    std::fs::write(
        &config,
        format!("[[providers]]\nname='feed'\nkind='rss'\nurl='{url}'"),
    )
    .unwrap();
    let output = run(
        dir.path(),
        &[
            "--config",
            config.to_str().unwrap(),
            "search",
            "linux",
            "--sort",
            "title",
            "--limit",
            "1",
            "--jsonl",
        ],
    );
    handle.join().unwrap();
    assert_eq!(output.status.code(), Some(0));
    let row: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(row["title"], "Alpha Linux");
    let cached = run(dir.path(), &["get", "1", "--json"]);
    assert_eq!(cached.status.code(), Some(0));
    let cached_row: Value = serde_json::from_slice(&cached.stdout).unwrap();
    assert_eq!(cached_row["title"], "Alpha Linux");
}

#[test]
fn apibay_and_authenticated_bitsearch_are_searchable() {
    let (apibay_url, apibay_handle) = server(
        r#"[{"id":"1","name":"Ubuntu APIBay","info_hash":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","size":"42","seeders":"12","leechers":"3","added":"1700000000"}]"#.into(),
        "q=ubuntu&cat=0",
    );
    let bitsearch_body = r#"{"success":true,"results":[{"title":"Ubuntu Bitsearch","infohash":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","size":84,"seeders":24,"leechers":6,"updatedAt":"2026-09-12T04:31:03Z"}],"pagination":{"hasNext":false}}"#;
    let (bitsearch_url, requests, bitsearch_handle) = paged_server(vec![bitsearch_body.into()]);
    let dir = tempfile::tempdir().unwrap();
    let config = dir.path().join("config.toml");
    std::fs::write(
        &config,
        format!(
            "[[providers]]\nname='apibay'\nkind='apibay'\nurl='{apibay_url}'\n[[providers]]\nname='bitsearch'\nkind='bitsearch'\nurl='{bitsearch_url}'\napi_key_env='BITSEARCH_TEST_KEY'"
        ),
    )
    .unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_magnet"))
        .arg("--cache")
        .arg(dir.path().join("cache.json"))
        .args([
            "--config",
            config.to_str().unwrap(),
            "search",
            "ubuntu",
            "--json",
        ])
        .env("BITSEARCH_TEST_KEY", "secret-value")
        .output()
        .unwrap();
    apibay_handle.join().unwrap();
    bitsearch_handle.join().unwrap();
    assert_eq!(
        output.status.code(),
        Some(0),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let rows: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(rows.as_array().unwrap().len(), 2);
    assert!(
        rows.as_array()
            .unwrap()
            .iter()
            .all(|row| row["magnet"].is_string())
    );
    let requests = requests.lock().unwrap();
    let request = &requests[0];
    assert!(request.contains("q=ubuntu&page=1&limit=100&sort=seeders&order=desc"));
    assert!(
        request
            .to_ascii_lowercase()
            .contains("x-api-key: secret-value")
    );
}
