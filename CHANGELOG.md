# Changelog

All notable changes to this project are documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and releases
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

### Fixed

- Apply the local relevance filter to every query and every provider instead of
  only non-ASCII APIBay searches. Knaben, Bitsearch, Nyaa, and Sukebei all pad
  queries they cannot match with their most popular rows, so a CJK search could
  return a full page of unrelated titles; a default aggregate search for `子子西`
  returned 100 rows of which none matched. Each drop is now reported per provider
  on stderr, and `--no-title-filter` restores the previous behaviour.
- Keep valid BtGoogle rows when one returned magnet is malformed; a single bad
  row now costs only itself and reports `skipped N invalid result(s)` like every
  other provider, instead of failing the whole source.
- Decode BtGoogle HTML entities in one pass, so `&amp;lt;` no longer collapses
  to `<`, and add the numeric `&` and `/` references that appear in magnets.
- Find the BtGoogle publication date by value instead of assuming it is the
  second `src` span, so an added or removed tracker label cannot shift it.
- Set `default_search = false` on the APIBay entry in `config.example.toml` and
  add the missing BtGoogle, DMHY, Anime Garden, and Mikan entries, so copying
  the example no longer re-enables a source that default searches exclude.

### Changed

- Archive APIBay from default aggregate searches; it remains available through
  `--source apibay` for explicit testing.
- Enable BtGoogle in default aggregate searches.
- Document that `default_search` defaults to `true`, and correct the default
  provider list, provider counts, and source tree in both READMEs.
- Promote `matches_query` from a private APIBay helper to `model::matches_query`,
  and give the library a `SearchOptions::title_filter` switch.

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

[Unreleased]: https://github.com/wustites/magnet-cli/compare/v0.2.0...HEAD
[0.2.0]: https://github.com/wustites/magnet-cli/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/wustites/magnet-cli/releases/tag/v0.1.0
