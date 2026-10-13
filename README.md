# magnet

[中文文档](README_CN.md)

A multi-provider torrent search CLI written in Rust, with a reusable library. Supports concurrent search, BTIH infohash dedup, tracker merging, and JSON output suited for scripts and agents.

Releases are published on [crates.io](https://crates.io/crates/magnet-cli). Built-in providers are Nyaa, Knaben, Sukebei (Nyaa NSFW), Bitsearch, BtGoogle, APIBay, DMHY, Anime Garden, and Mikan Project, plus configurable Torznab and RSS/Atom. Hand results off to any BitTorrent client; this project has no downloader, TUI, or MCP server.

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
| `--no-title-filter` | off | Keep results whose title omits a query term |

`--json`, `--jsonl`, `--magnet` are mutually exclusive. Without them you get a table; unknown seeders, size, and date show as `?`, sizes render as decimal GB, AGE counts days.

Sizes accept raw bytes, K/M/G/T, KB/MB/GB/TB (decimal), or KiB/MiB/GiB/TiB (binary), e.g. `10G`, `1.5GiB`. Results with unknown fields are excluded once a size or seeders filter is set. Filtering runs after dedup; `--limit` applies after sorting.

### Global Options & Environment Variables

| Option | Env var | Effect |
| --- | --- | --- |
| `--config PATH` | `MAGNET_CONFIG` | Use a TOML config file |
| `--cache PATH` | `MAGNET_CACHE` | Use a snapshot file, handy for isolated sessions |

CLI flags win over the matching env vars. Global options can go before or after the subcommand. The program never auto-reads `./config.toml`; pass a path explicitly or set `MAGNET_CONFIG`.

## Documentation

| Document | Contents |
| --- | --- |
| [Usage tutorial](docs/usage.md) | Step-by-step: install, commands, filtering, output, picking a magnet |
| [Provider configuration](docs/providers.md) | Custom providers, `kind` reference, Sukebei, Torznab, RSS |
| [Scripting & integration](docs/scripting.md) | JSON shape, aggregation, title filtering, snapshots & `get`, exit codes, pikpaktui |
| [Documentation index](docs/README.md) | Every doc under `docs/` with its status |
| [CHANGELOG](CHANGELOG.md) · [RELEASING](RELEASING.md) | Release history and process |

## Current Limits

Up to 8 providers run concurrently (configurable from 1 to 64), with a 15 s default timeout per provider and an 8 MiB cap per response. Extra sources queue. `--timeout` covers one provider and all of its requested pages; use `--deadline` when the whole command needs a bound.

Each source reads one page by default. `--pages` can request up to 20 pages from Knaben, Bitsearch, and Torznab; APIBay, Nyaa/Sukebei RSS, and generic RSS/Atom feeds are fetched once. `--limit` caps final output only and may return fewer rows than asked. Public source availability, rate limits, and completeness depend on upstream services.

BtGoogle is the one provider read by scraping HTML rather than a documented API, so it is the most likely to break when that page changes. No TUI, MCP, DHT lookup, or downloads.

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
    ├── feeds.rs        # Nyaa / Sukebei / Torznab / RSS / DMHY / Mikan
    ├── knaben.rs       # Knaben JSON API
    ├── apibay.rs       # APIBay JSON API
    ├── bitsearch.rs    # Bitsearch JSON API
    ├── btgoogle.rs     # BtGoogle HTML search endpoint
    └── animegarden.rs  # Anime Garden JSON API
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
