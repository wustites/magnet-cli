# Changelog

All notable changes to this project are documented in this file. The format is
based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/), and releases
follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## Unreleased

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
