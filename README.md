# magnet

A Rust CLI and reusable library for concurrent torrent search. Built-in Nyaa and
Knaben providers, configurable Torznab and RSS feeds, BTIH deduplication, tracker
union, deterministic sorting, and machine-readable output. No downloader or browser
runtime is included.

## Install

```sh
cargo install --path . --locked
magnet search "ubuntu" --json
```

## Usage

```sh
magnet search "ubuntu"                         # table, top 20 by seeders
magnet search "ubuntu" --source knaben --json
magnet search "ubuntu" --jsonl
magnet search "ubuntu" --magnet
magnet search "ubuntu" --min-seeds 10 --min-size 1G --max-size 10G --sort seeds --limit 20
magnet get 1                                  # magnet from last search
magnet get 1 --json                           # full cached result
magnet resolve 0000000000000000000000000000000000000000 --name "Example" --json
magnet providers --json
```

`--source` accepts comma-separated names or repeated flags. `--sort` accepts
`seeds`, `size`, `date` (descending, unknown last), or `title` (ascending).
Size suffixes K/M/G/T and KB/MB/GB/TB are decimal; KiB/MiB/GiB/TiB are binary.
Unknown sizes/seeder counts are excluded when the corresponding filter is set.
`--json`, `--jsonl`, and `--magnet` are mutually exclusive. Diagnostics go to stderr.
JSONL is emitted after aggregation, not streamed from individual providers.

`resolve` only validates a hexadecimal/base32 BTIH and builds its magnet locally.
It does not query the DHT, scrape detail pages, or retrieve torrent metadata.
Use repeated `--tracker URL` to supply trackers.

## Providers

Without a config, searches query Nyaa RSS and Knaben JSON concurrently. To use
private indexers or custom feeds, copy [config.example.toml](config.example.toml):

```sh
export TORZNAB_API_KEY='your-key'
magnet --config config.toml search "ubuntu" --source local --json
```

`--config` / `MAGNET_CONFIG` replaces the default provider list. Each entry needs
a unique `name`, `kind` (`nyaa`, `knaben`, `torznab`, `rss`), and HTTP(S) `url`.
Torznab takes the full API endpoint and an optional `api_key_env` environment
variable name. `providers` lists names and kinds without exposing endpoints or keys.
RSS feeds are fetched unchanged and searched locally using all query words in the
title. RSS items can supply magnet links, enclosures, and Nyaa/Torznab extensions.
Plain Newznab NZB results cannot be converted into BitTorrent magnets.

Protocol references: [Knaben API](https://knaben.org/api/v1/),
[Nyaa RSS template](https://github.com/nyaadevs/nyaa/blob/master/nyaa/templates/rss.xml),
[Torznab specification](https://torznab.github.io/spec-1.3-draft/torznab/Specification-v1.3.html).

## Result contract

JSON is an array; JSONL uses the same object per line:

```json
{"id":1,"title":"Example","info_hash":"0000000000000000000000000000000000000000","magnet":"magnet:?xt=urn%3Abtih%3A0000000000000000000000000000000000000000&dn=Example","size":null,"seeders":null,"leechers":null,"published_at":null,"sources":["local"],"detail_url":null,"trackers":[]}
```

Hashes normalize to lowercase hex, including base32 inputs. Only equal BTIHs merge;
unknown hashes remain separate even when titles match. Duplicate records union
sources and trackers and take the maximum reported seeders/leechers, not their sum.
The first configured provider supplies the title and available metadata; missing
metadata is filled from later providers. Invalid/conflicting hashes are skipped
with a warning. BTIH/v1 is supported; v2-only magnets are not supported in this MVP.
Magnet reconstruction preserves BTIH, display name, and trackers; other magnet
parameters are not retained. Dates without an explicit timezone remain unknown.

IDs are one-based positions in the **last completed search snapshot**, saved
atomically under the platform user cache directory (`~/.cache/magnet` on Linux).
Every completed search, including empty/failed searches, replaces the snapshot.
Concurrent searches use last-writer-wins semantics. Use `--cache PATH` or
`MAGNET_CACHE` to isolate agent sessions. `get` reads this snapshot without network
access. Rows without a hash/magnet remain available in table/JSON output; `--magnet`
excludes them and `get ID` returns 1 for them (`get ID --json` still works).

## Failures and limits

| Exit | Meaning |
| --- | --- |
| 0 | Results returned, or another command succeeded |
| 1 | No matching results / cached ID or magnet unavailable |
| 2 | Provider parsing/configuration-at-runtime failure, or local I/O failure |
| 3 | Invalid arguments or configuration |
| 4 | All providers failed with HTTP/network/timeout errors |

Partial provider failures produce warnings; usable results still return 0. If any
provider succeeds but the final result set is empty, exit is 1. All-provider mixed
transport/parsing failures return 2. Empty JSON output is `[]`; JSONL/magnet output
is empty. Cache write errors return 2 before results are printed.

At most eight providers run concurrently, each with a 15-second total deadline
(`--timeout 1..300`) and an 8 MiB response cap. Searches fetch one page/feed per
provider (Knaben: 150 records); `--limit` caps the final aggregate, not exhaustive
pagination. Public indexer availability and result completeness are not guaranteed.

## Development

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test --locked
```

Tests use local HTTP fixtures and do not need public providers or API keys.
The `Provider` trait, normalized `Torrent`, and search engine are exported by the
library so additional providers or interfaces can share the same core.
