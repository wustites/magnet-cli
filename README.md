# magnet

[中文文档](README_CN.md)

A multi-provider torrent search CLI written in Rust, with a reusable library. Supports concurrent search, BTIH infohash dedup, tracker merging, and JSON output suited for scripts and agents.

0.1.0 is published on [crates.io](https://crates.io/crates/magnet-cli). The current source includes built-in Nyaa, Knaben, Sukebei (Nyaa NSFW), APIBay, and Bitsearch, plus configurable Torznab and RSS/Atom. Hand results off to any BitTorrent client; this project has no downloader, TUI, or MCP server.

## Install & Quick Start

Install from crates.io (the command is `magnet`):

```bash
cargo install magnet-cli --locked
magnet --version
magnet search "ubuntu" --json
```

Or install from source:

```bash
git clone https://github.com/wustites/magnet-cli.git
cd magnet-cli
cargo install --path . --locked
```

The installed command is `magnet`. If your shell can't find it, make sure Cargo's bin directory is on `PATH` (usually `~/.cargo/bin`).

Or just build and run inside the repo:

```bash
cargo build --release --locked
./target/release/magnet search "ubuntu"
```

## Common Commands

```bash
# Query default-search sources, top 20 by seeders descending
magnet search "ubuntu"

# Pick sources; comma-separated or repeated --source
magnet search "ubuntu" --source knaben --json
magnet search "ubuntu" --source nyaa,knaben --jsonl

# One magnet per line, good for piping into scripts
magnet search "ubuntu" --magnet

# Filtering, sorting, result count
magnet search "ubuntu" --min-seeds 10 --min-size 1G --max-size 10G --sort seeds --limit 20

# Read back the previous search; IDs start at 1
magnet get 1
magnet get 1 --json

# List configured providers without searching
magnet providers --json

# Build a magnet from an infohash locally; the all-zero hash is demo-only
magnet resolve 0000000000000000000000000000000000000000 --name "Example" --json
```

`resolve` accepts a 40-char hex or 32-char base32 BTIH, plus repeatable `--tracker URL`. It only validates the hash and constructs a magnet: no DHT lookup, no detail-page fetch, no torrent metadata. `get --json` and `resolve --json` print a single object; `providers --json` prints an array with `name`, `kind`, `default_search`.

See `magnet --help` or `magnet search --help` for full command help.

### Search Options

| Option | Default | Description |
| --- | --- | --- |
| `QUERY` | required | Non-empty search term; quote it when it contains spaces |
| `--source NAME` | sources with `default_search = true` | Select by provider `name`; unknown names exit 3 |
| `--json` | off | Print a JSON array |
| `--jsonl` | off | One JSON object per line, emitted after aggregation |
| `--magnet` | off | One magnet per line, skipping results without one |
| `--min-seeds N` | unlimited | Seeders lower bound, inclusive |
| `--min-size SIZE` | unlimited | Size lower bound, inclusive |
| `--max-size SIZE` | unlimited | Size upper bound, inclusive |
| `--sort seeds\|size\|date\|title` | `seeds` | First three descending, unknowns last; title ascending |
| `--limit N` | `20` | Cap on final aggregated results, must be > 0 |
| `--timeout SECONDS` | `15` | Per-provider timeout, 1–300 seconds |
| `--concurrency N` | `8` | Providers searched concurrently, 1–64 |
| `--deadline SECONDS` | none | Total search deadline, including queued providers, 1–3600 seconds |
| `--pages N` | `1` | Pages requested from providers with pagination support, 1–20 |

`--json`, `--jsonl`, `--magnet` are mutually exclusive. Without them you get a table; unknown seeders, size, and date show as `?`, sizes render as decimal GB, AGE counts days.

Sizes accept raw bytes, K/M/G/T, KB/MB/GB/TB (decimal), or KiB/MiB/GiB/TiB (binary), e.g. `10G`, `1.5GiB`. Results with unknown fields are excluded once a size or seeders filter is set. Filtering runs after dedup; `--limit` applies after sorting.

### Global Options & Environment Variables

| Option | Env var | Effect |
| --- | --- | --- |
| `--config PATH` | `MAGNET_CONFIG` | Use a TOML config file |
| `--cache PATH` | `MAGNET_CACHE` | Use a snapshot file, handy for isolated sessions |

CLI flags win over the matching env vars. Global options can go before or after the subcommand. The program never auto-reads `./config.toml`; pass a path explicitly or set `MAGNET_CONFIG`.

## Configuring Providers

With no config, Nyaa RSS, Knaben JSON, Sukebei RSS, APIBay, and Bitsearch are queried concurrently. An explicit config **replaces the default provider list**, it is not appended to it.

Start from [config.example.toml](config.example.toml):

```bash
cp config.example.toml config.toml
magnet --config config.toml providers --json
magnet --config config.toml search "ubuntu" --json
```

Each `[[providers]]` entry needs `name`, `kind`, and an HTTP(S) `url`. `name` must be non-empty, unique, and ASCII letters, digits, hyphens, or underscores only. At least one source is required; unknown config fields are rejected. `api_key_env` is accepted only for Torznab and Bitsearch and must be a valid environment variable name.

| `kind` | Request | Search behavior |
| --- | --- | --- |
| `nyaa` | HTTP GET RSS | Adds `page=rss` and query param `q` |
| `sukebei` | HTTP GET RSS | Nyaa RSS protocol, adds `page=rss` and `q` |
| `knaben` | HTTP POST JSON | Title search against the configured API URL, 150 rows per request |
| `apibay` | HTTP GET JSON | Searches APIBay across all categories; returns up to the API's fixed result set |
| `bitsearch` | HTTP GET JSON | Searches Bitsearch, 100 rows per page; optional API key header |
| `torznab` | HTTP GET RSS/XML | Adds `t=search`, `q`, `extended=1`, optional API key |
| `rss` | HTTP GET RSS/Atom | Fetches the URL as-is, filters by title locally; the query is split on whitespace, case-insensitive, every word must match |

`--pages` uses Knaben's `from` offset, Bitsearch's `page`, and Torznab's `offset`/`limit`. APIBay, Nyaa/Sukebei RSS, and configured RSS/Atom URLs are fetched once because they do not expose a reliable compatible pagination mechanism. Pagination stops early when a provider returns an empty/final page; the per-provider timeout covers all requested pages.

Bitsearch's anonymous tier currently allows 200 requests per IP per day. To use an account key, set `api_key_env` on that provider; the key is sent in the `x-api-key` header and never included in diagnostics.

### Sukebei: Nyaa NSFW

```bash
magnet search "keyword" --source sukebei --json
magnet search "keyword" --source nyaa,sukebei --jsonl
```

Sukebei lives at `https://sukebei.nyaa.si/`. A custom config can add:

```toml
[[providers]]
name = "sukebei"
kind = "sukebei"
url = "https://sukebei.nyaa.si/"
default_search = true
```

Every provider accepts optional `default_search` (defaults to `true`). With `false` it still shows in `providers` but only searches when picked via `--source`. The built-in Sukebei and the example config set it to `true`, joining the default aggregated search; flip it to `false` to sit out. With no default-searchable sources you must pass `--source`, otherwise exit 3.

### Torznab: Jackett / Prowlarr

Add the entry below to your config, or uncomment the matching entry in the example file. Replace `url` with the **full Torznab API endpoint** your service gives you, not the service homepage:

```toml
[[providers]]
name = "local"
kind = "torznab"
url = "http://localhost:9117/api/v2.0/indexers/all/results/torznab/api"
api_key_env = "TORZNAB_API_KEY"
```

`api_key_env` names the environment variable holding the key, not the key itself; only Torznab uses this field. Sources without auth omit it.

```bash
export TORZNAB_API_KEY='your-key'
magnet --config config.toml search "ubuntu" --source local --json
```

`providers` shows names and kinds only, never URLs or keys. HTTP error diagnostics never print request URLs either.

### RSS

```toml
[[providers]]
name = "linux"
kind = "rss"
url = "https://example.org/torrents.rss"
```

The URL above is a placeholder; substitute a real RSS or Atom feed. The parser understands magnet links and enclosures in RSS items and Atom entries, plus Nyaa/Torznab extension fields. A bare `.torrent` download link is never fetched to compute a hash. Plain Newznab NZB results can't become BitTorrent magnets.

Protocol references: [Knaben API](https://knaben.org/api/v1/), [Bitsearch API](https://bitsearch.eu/api), [Nyaa RSS template](https://github.com/nyaadevs/nyaa/blob/master/nyaa/templates/rss.xml), [Torznab spec](https://torznab.github.io/spec-1.3-draft/torznab/Specification-v1.3.html).

## JSON Results & Aggregation Rules

`search --json` returns an array; `--jsonl` uses the same object shape. Sample data:

```json
[
  {
    "id": 1,
    "title": "Example",
    "info_hash": "0000000000000000000000000000000000000000",
    "magnet": "magnet:?xt=urn%3Abtih%3A0000000000000000000000000000000000000000&dn=Example",
    "size": null,
    "seeders": null,
    "leechers": null,
    "published_at": null,
    "sources": ["local"],
    "detail_url": null,
    "trackers": []
  }
]
```

| Field | Type | Meaning |
| --- | --- | --- |
| `id` | integer | Position in the current search snapshot, from 1; local `resolve` prints 0 |
| `title` | string | Torrent title |
| `info_hash` | string or null | BTIH normalized to lowercase hex |
| `magnet` | string or null | URL-encoded magnet URI |
| `size` | integer or null | Bytes |
| `seeders` / `leechers` | integer or null | Seeder / leecher counts as reported |
| `published_at` | string or null | UTC RFC 3339 timestamp; null when unparseable or zone-less |
| `sources` | string array | Merged provider names, sorted and deduped |
| `detail_url` | string or null | Detail or related link from the source |
| `trackers` | string array | Merged tracker URLs, sorted and deduped |

Aggregation keys on BTIH, never on title. Base32 hashes convert to hex; hash-less results never merge even with identical names. For one hash, sources and trackers union, seeders/leechers take the max reported value, never summed.

The title comes from the first record in config order; size, publish time, and detail link keep that record's values, backfilled from later records when missing. Provider completion order doesn't affect this priority. Invalid or conflicting hashes are skipped with a warning.

Rebuilt magnets keep BTIH, display name, and trackers; other magnet params are dropped. BTIH/v1 is supported; v2-only magnets are not.

## Search Snapshots & `get`

Results hit the snapshot after filtering, sorting, and truncation, then print. Linux default is `~/.cache/magnet/last-search.json`; with `XDG_CACHE_HOME` set it lives under `magnet/last-search.json` there. Other platforms use the platform cache dir; when none is found it falls back to `./.magnet-last-search.json`.

`get ID` only reads the snapshot, never the network. IDs are valid for that one search; the next search renumbers. Any search reaching the snapshot stage — empty results and all-provider-failed included — atomically replaces the old snapshot; argument validation failures don't wipe it, write failures exit 2.

Records without hash/magnet can still appear in tables and JSON. `get ID` on one exits 1, while `get ID --json` reads the full record. A missing or corrupt cache file exits 2; search again.

Concurrent searches use the last successfully written snapshot. Scripts or agent sessions can isolate the cache explicitly:

```bash
export MAGNET_CACHE="$PWD/session-search.json"
magnet search "ubuntu" --json
magnet get 1 --json
```

## Exit Codes & Scripting

Results go to stdout, warnings and errors to stderr. Scripts should capture both separately and check the exit code.

| Code | Meaning |
| --- | --- |
| `0` | Search returned results, or other command succeeded |
| `1` | No matches, no such ID in the snapshot, or the result has no printable magnet |
| `2` | All sources failed including parse/runtime config errors, or local I/O failed |
| `3` | Bad arguments or config, including unknown source, unreadable config file |
| `4` | Every provider failed with HTTP, network, or timeout errors |

Partial source failures print warnings; the exit stays 0 while results remain. At least one success but nothing after filtering exits 1. All failed with mixed network and parse errors exits 2. A missing `api_key_env` variable counts as that provider's runtime failure.

A search reaching the output stage prints `[]` for empty JSON and zero lines for empty JSONL/magnet; on argument, config, or cache-write failure, stdout JSON is not guaranteed. JSONL emits once after aggregation, not as a per-provider stream.

The Python example below needs no extra dependencies and tells "no results" apart from failure:

```python
import json
import subprocess
import sys

result = subprocess.run(
    ["magnet", "--cache", "session-search.json", "search", "ubuntu", "--json"],
    capture_output=True,
    text=True,
)
if result.stderr:
    print(result.stderr, end="", file=sys.stderr)
if result.returncode not in (0, 1):
    raise SystemExit(result.returncode)
rows = json.loads(result.stdout)
for row in rows:
    if row["magnet"]:
        print(row["magnet"])
```

## Piping into pikpaktui

Install `jq` and `pikpaktui` and finish `pikpaktui` login first. Pipe the first magnet of a search into `pikpaktui offline` to create a PikPak cloud task:

```bash
magnet search "odv-534" --json | jq -r '.[0].magnet' | xargs pikpaktui offline
```

`.[0]` picks the top result (seeders descending by default), `jq -r` strips the JSON quotes, `xargs` passes the magnet as the argument.

For scripts, confirm success first, then extract a non-empty magnet and quote the full URI. Nothing is created when the search fails, results are empty, or the first row lacks a magnet:

```bash
if results=$(magnet search "odv-534" --json) &&
   uri=$(printf '%s' "$results" | jq -er '.[0].magnet | select(type == "string" and startswith("magnet:?"))'); then
    pikpaktui offline "$uri"
fi
```

Or read a picked ID from the last search:

```bash
if uri=$(magnet get 1); then
    pikpaktui offline "$uri" --to "/Downloads"
fi
```

`pikpaktui offline` takes `--to` for the target dir, `--name` for the task name, and `--dry-run` to preview; e.g. swap the last call for `pikpaktui offline "$uri" --dry-run`. Accounts, target dirs, and offline tasks belong to `pikpaktui`; `magnet` handles search and magnet output.

## Current Limits

Eight providers run concurrently by default (configurable from 1 to 64), with a 15 s default timeout per provider and an 8 MiB cap per response. Extra sources queue. `--timeout` covers one provider and all of its requested pages; use `--deadline` when the whole command needs a bound.

Each source reads one page by default. `--pages` can request up to 20 pages from Knaben, Bitsearch, and Torznab; APIBay, Nyaa/Sukebei RSS, and generic RSS/Atom feeds are fetched once. `--limit` caps final output only and may return fewer rows than asked. Public source availability, rate limits, and completeness depend on upstream services.

No TUI, MCP, HTML scraping, DHT lookup, or downloads.

## Develop & Extend

```text
src/
├── main.rs             # command execution, output, filtering, sorting
├── cli.rs              # clap argument definitions
├── model.rs            # Torrent model, hash/magnet normalization
├── search.rs           # concurrent scheduling, error summary, dedup
├── cache.rs            # atomic snapshot writes and reads
├── lib.rs              # library module exports
└── providers/
    ├── mod.rs          # Provider trait, config, HTTP wrapper
    ├── feeds.rs        # Nyaa / Sukebei / Torznab / RSS
    └── knaben.rs       # Knaben JSON API
```

The crate root re-exports `Provider`, `Torrent`, `SearchOptions`, and both search entry points for new providers or other front ends. Implement `Provider::search` for a basic source and optionally override `search_pages`; call `search` for backward-compatible defaults or `search_with_options` for concurrency, deadline, and pagination controls. To support a new `kind` in CLI TOML configs, also update `Kind` and `HttpProvider` dispatch.

```bash
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
```

Tests use a local HTTP stub; no public providers or API keys needed. CI runs the format check, Clippy, and tests across Linux, Windows, and macOS.

## License

[MIT](LICENSE)
