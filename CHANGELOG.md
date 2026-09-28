# Changelog

All notable changes to this project are documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and releases
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

## 0.3.0 - 2026-09-28

### Added

- `--no-title-filter` opts out of the local relevance filter for queries whose
  spelling never appears literally in the titles they should match.
- `model::matches_query` exposes the title-match predicate that was previously a
  private APIBay helper, and `SearchOptions::title_filter` lets library callers
  control the behaviour.

### Changed

- **Breaking:** results whose title does not contain every query term are now
  dropped, for every query and every provider rather than only non-ASCII APIBay
  searches. Knaben, Bitsearch, Nyaa, and Sukebei all pad a query they cannot
  match with their most popular rows, so a default aggregate search for `子子西`
  returned 100 rows of which none matched; it now returns 12 rows and all 12
  match. Each drop is reported per provider on stderr, so nothing is lost
  silently. Ordinary queries are unaffected: `ubuntu`, `big buck bunny`,
  `ubuntu 24.04`, `SPY x FAMILY`, `苏畅`, and `1080p` all keep a 100% survival
  rate.
- Enable BtGoogle in default aggregate searches, and archive APIBay from them;
  APIBay remains available through `--source apibay`.
- Document that `default_search` defaults to `true`, which is why an example
  config has to set it to `false` explicitly to exclude a source.

### Fixed

- Update rustls to 0.23.45 for RUSTSEC-2026-0285, where a TLS 1.3 peer could
  send handshake messages at the wrong encryption level and rustls accepted them
  instead of alerting as RFC 8446 section 5.1 requires.
- Keep valid BtGoogle rows when one returned magnet is malformed. A single bad
  row now costs only itself and reports `skipped N invalid result(s)` like every
  other provider, instead of failing the whole source.
- Decode BtGoogle HTML entities in one pass, so `&amp;lt;` no longer collapses to
  `<`, and add the numeric `&` and `/` references that appear inside magnets.
- Find the BtGoogle publication date by value instead of assuming it is the
  second `src` span, so an added or removed tracker label cannot shift it.
- Set `default_search = false` on the APIBay entry in `config.example.toml` and
  add the missing BtGoogle, DMHY, Anime Garden, and Mikan entries, so copying the
  example no longer re-enables a source that default searches exclude.
- Grant the RustSec audit job `checks: write`; `audit-check` reports through a
  check run and could not create one with `contents: read` alone.
- Correct the default provider list, the provider count, the source tree, and
  the "no HTML scraping" claim in both READMEs, and add the missing 0.2.1
  changelog link.

## 0.2.1 - 2026-09-15

### Fixed

- Filter unrelated APIBay fallback rows for non-ASCII/CJK queries and document the provider limitation.

### Added

- Add opt-in DMHY keyword RSS search via `--source dmhy`.
- Add opt-in Anime Garden JSON search via `--source animegarden`.
- Add opt-in Mikan Project keyword RSS search via `--source mikan`.
- Add opt-in BtGoogle HTML search via `--source btgoogle`.

## 0.2.0 - 2026-09-12

### Added

- Optional provider pagination with `search --pages`.
- Built-in APIBay and Bitsearch search providers.
- Configurable provider concurrency and an optional total search deadline.
- Atom feed parsing for configured feed providers.
- A documented Rust 1.88 minimum supported version, Dependabot update automation,
  and RustSec CI.

### Changed

- Provider configuration now rejects `api_key_env` outside Torznab and Bitsearch
  entries, as well as invalid environment variable names.

## 0.1.0 - 2026-09-09

### Added

- Concurrent Nyaa, Knaben, Sukebei, Torznab, and RSS search.
- BTIH normalization, deduplication, tracker merging, filtering, and sorting.
- JSON, JSONL, magnet-only, and human-readable output.
- Atomic last-search snapshots with local `get` and `resolve` commands.

[Unreleased]: https://github.com/wustites/magnet-cli/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/wustites/magnet-cli/compare/v0.2.1...v0.3.0
[0.2.1]: https://github.com/wustites/magnet-cli/compare/v0.2.0...v0.2.1
[0.2.0]: https://github.com/wustites/magnet-cli/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/wustites/magnet-cli/releases/tag/v0.1.0
