//! Reusable torrent search primitives used by the `magnet` CLI.
//!
//! Implement [`providers::Provider`] to add a source, then call [`search::search`]
//! for the stable default behavior or [`search::search_with_options`] for explicit
//! concurrency, deadline, and pagination controls.
#![warn(missing_docs)]

/// Atomic persistence for search-result snapshots.
pub mod cache;
/// Command-line argument types shared with the binary.
#[allow(missing_docs)]
pub mod cli;
/// Torrent data, BTIH normalization, and parsing helpers.
pub mod model;
/// Provider configuration, HTTP implementations, and the provider trait.
pub mod providers;
/// Concurrent search scheduling and result aggregation.
pub mod search;

pub use model::Torrent;
pub use providers::{Provider, ProviderError};
pub use search::{SearchOptions, SearchReport, search, search_with_options};
