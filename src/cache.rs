use crate::model::Torrent;
use anyhow::{Context, Result};
use std::{
    io::Write,
    path::{Path, PathBuf},
};

/// Returns the platform-appropriate path for the last-search snapshot.
pub fn default_path() -> PathBuf {
    directories::ProjectDirs::from("", "", "magnet")
        .map(|p| p.cache_dir().join("last-search.json"))
        .unwrap_or_else(|| PathBuf::from(".magnet-last-search.json"))
}
/// Atomically replaces `path` with a JSON snapshot of `rows`.
pub fn save(path: &Path, rows: &[Torrent]) -> Result<()> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    std::fs::create_dir_all(parent).context("cannot create cache directory")?;
    // tempfile uses private permissions and rename makes concurrent writes atomic.
    let mut temp = tempfile::NamedTempFile::new_in(parent).context("cannot create cache file")?;
    serde_json::to_writer(&mut temp, rows)?;
    temp.flush()?;
    temp.as_file().sync_all()?;
    temp.persist(path).context("cannot replace search cache")?;
    Ok(())
}
/// Reads and validates a JSON search snapshot.
pub fn read(path: &Path) -> Result<Vec<Torrent>> {
    let file =
        std::fs::File::open(path).context("no readable search cache; run magnet search first")?;
    serde_json::from_reader(file).context("invalid search cache; run magnet search again")
}
