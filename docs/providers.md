# Provider configuration

[中文](providers.zh-CN.md)

magnet ships with nine built-in providers and also supports custom Torznab and RSS/Atom. This page is the full reference; for a quick tour see the [usage tutorial](usage.md) and for the overview see the [README](../README.md).

## Defaults vs. explicit config

With no config, Nyaa RSS, Knaben JSON, Sukebei RSS, Bitsearch, and BtGoogle are queried concurrently. An explicit config **replaces the default provider list**, it is not appended to it. Nine providers are built in; those five take part by default and APIBay, DMHY, Anime Garden, and Mikan are opt-in via `--source`.

Start from [config.example.toml](../config.example.toml):

```bash
cp config.example.toml config.toml
magnet --config config.toml providers --json
magnet --config config.toml search "ubuntu" --json
```

Each `[[providers]]` entry needs `name`, `kind`, and an HTTP(S) `url`. `name` must be non-empty, unique, and ASCII letters, digits, hyphens, or underscores only. At least one source is required; unknown config fields are rejected. `api_key_env` is accepted only for Torznab and Bitsearch and must be a valid environment variable name. `default_search` defaults to `true`, so write `default_search = false` explicitly for any source you want excluded from default aggregate searches.

## `kind` reference

| `kind` | Request | Search behavior |
| --- | --- | --- |
| `nyaa` | HTTP GET RSS | Adds `page=rss` and query param `q` |
| `sukebei` | HTTP GET RSS | Nyaa RSS protocol, adds `page=rss` and `q` |
| `knaben` | HTTP POST JSON | Title search against the configured API URL, 150 rows per request |
| `apibay` | HTTP GET JSON | Searches APIBay across all categories; returns up to the API's fixed result set. For non-ASCII queries, the CLI filters APIBay fallback results locally by title |
| `bitsearch` | HTTP GET JSON | Searches Bitsearch, 100 rows per page; optional API key header |
| `dmhy` | HTTP GET RSS | DMHY keyword RSS search; opt in with `--source dmhy` |
| `animegarden` | HTTP POST JSON | Anime Garden full-text anime search; opt in with `--source animegarden` |
| `mikan` | HTTP GET RSS | Mikan Project keyword RSS search; opt in with `--source mikan` |
| `btgoogle` | HTTP GET HTML | BtGoogle search results; enabled in default aggregate searches |
| `torznab` | HTTP GET RSS/XML | Adds `t=search`, `q`, `extended=1`, optional API key |
| `rss` | HTTP GET RSS/Atom | Fetches the URL as-is, filters by title locally; the query is split on whitespace, case-insensitive, every word must match |

## Pagination

`--pages` uses Knaben's `from` offset, Bitsearch's `page`, and Torznab's `offset`/`limit`. APIBay, Nyaa/Sukebei RSS, and configured RSS/Atom URLs are fetched once because they do not expose a reliable compatible pagination mechanism. Pagination stops early when a provider returns an empty/final page; the per-provider timeout covers all requested pages.

## Built-in source notes

- **APIBay**: for non-ASCII/CJK queries it sometimes returns popular fallback rows instead of matches. The CLI requires returned titles to contain every query term and removes those rows locally. Because CJK coverage is unreliable, APIBay is archived from default aggregate searches; select it explicitly with `--source apibay` when needed.
- **DMHY**: opt-in with `--source dmhy`; it uses DMHY's public keyword RSS endpoint and is disabled in default aggregate searches.
- **Anime Garden**: opt-in with `--source animegarden`; it uses the project's public JSON API and is disabled in default aggregate searches.
- **Mikan Project**: opt-in with `--source mikan`; it uses the site's keyword RSS search endpoint and is disabled in default aggregate searches.
- **BtGoogle**: uses the site's search-results endpoint and participates in default aggregate searches; it can also be queried alone with `--source btgoogle`.
- **Bitsearch**: the anonymous tier currently allows 200 requests per IP per day. To use an account key, set `api_key_env` on that provider; the key is sent in the `x-api-key` header and never included in diagnostics.

For CJK content, enable Knaben or Bitsearch as well for better coverage.

## Search test keywords

[testdata/search-keywords.csv](../testdata/search-keywords.csv) is a reusable keyword list for testing. The first entry `子子西` is a fixed CJK regression term; keep it unchanged when testing several sources. The CSV's `category`, `keyword`, and `purpose` columns hold the category, the search term, and the test intent.

For example, to take just the fixed regression term:

```bash
awk -F, 'NR > 1 { print $2; exit }' testdata/search-keywords.csv
magnet search '子子西' --source btgoogle --json
```

## Sukebei: Nyaa NSFW

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

## Torznab: Jackett / Prowlarr

See the [Prowlarr integration guide](prowlarr.md) for the local setup, Caddy reverse proxy, API key, and current test status. This project recommends `http://127.0.0.1:9696/3/api` on the same host.

The [bitmagnet integration guide](bitmagnet.md) and its [backup and restore guide](bitmagnet-backup.md) document a self-hosted DHT indexer setup used during early development. Both are **archived** (2026-09-21) and no longer maintained; the guides remain readable for reference.

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

## RSS

```toml
[[providers]]
name = "linux"
kind = "rss"
url = "https://example.org/torrents.rss"
```

The URL above is a placeholder; substitute a real RSS or Atom feed. The parser understands magnet links and enclosures in RSS items and Atom entries, plus Nyaa/Torznab extension fields. A bare `.torrent` download link is never fetched to compute a hash. Plain Newznab NZB results can't become BitTorrent magnets.

## Protocol references

- [Knaben API](https://knaben.org/api/v1/)
- [Bitsearch API](https://bitsearch.eu/api)
- [Nyaa RSS template](https://github.com/nyaadevs/nyaa/blob/master/nyaa/templates/rss.xml)
- [Torznab spec](https://torznab.github.io/spec-1.3-draft/torznab/Specification-v1.3.html)
