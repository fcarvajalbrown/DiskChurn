use std::fs;
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};

use crate::types::{DiskSnapshot, FolderStats};

const MAX_SNAPSHOTS: usize = 10;

#[derive(Serialize, Deserialize)]
pub struct Snapshot {
    pub drive: String,
    pub total_bytes: u64,
    pub free_bytes: u64,
    pub files_scanned: u64,
    pub folders: Vec<FolderStats>,
    pub scanned_at: u64,
}

impl From<&DiskSnapshot> for Snapshot {
    fn from(s: &DiskSnapshot) -> Self {
        Self {
            drive: s.drive.clone(),
            total_bytes: s.total_bytes,
            free_bytes: s.free_bytes,
            files_scanned: s.files_scanned,
            folders: s.folders.clone(),
            scanned_at: now_secs(),
        }
    }
}

fn history_dir() -> PathBuf {
    let base = std::env::var("APPDATA").unwrap_or_else(|_| ".".into());
    PathBuf::from(base).join("DiskChurn").join("snapshots")
}

fn drive_key(drive: &str) -> String {
    drive.trim_end_matches('\\').replace(':', "")
}

pub fn save(snap: &DiskSnapshot) {
    if snap.folders.is_empty() {
        return;
    }
    let dir = history_dir();
    if fs::create_dir_all(&dir).is_err() {
        return;
    }
    let ts = now_secs();
    let name = format!("{}_{}_{}.bin", drive_key(&snap.drive), ts, snap.files_scanned);
    if let Ok(data) = bincode::serialize(&Snapshot::from(snap)) {
        let _ = fs::write(dir.join(name), data);
    }
    prune(&snap.drive);
}

// returns (timestamp, files_scanned, path) newest first
pub fn list(drive: &str) -> Vec<(u64, u64, PathBuf)> {
    let dir = history_dir();
    let prefix = format!("{}_", drive_key(drive));
    let mut entries: Vec<(u64, u64, PathBuf)> = fs::read_dir(&dir)
        .ok()
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| {
            let path = e.path();
            if path.extension()?.to_str()? != "bin" {
                return None;
            }
            let stem = path.file_stem()?.to_string_lossy().into_owned();
            let rest = stem.strip_prefix(&prefix)?;
            let mut parts = rest.splitn(2, '_');
            let ts: u64 = parts.next()?.parse().ok()?;
            let fc: u64 = parts.next()?.parse().ok()?;
            Some((ts, fc, path))
        })
        .collect();
    entries.sort_by(|a, b| b.0.cmp(&a.0));
    entries
}

pub fn load(path: &Path) -> Option<Snapshot> {
    let data = fs::read(path).ok()?;
    bincode::deserialize(&data).ok()
}

fn prune(drive: &str) {
    for (_, _, path) in list(drive).into_iter().skip(MAX_SNAPSHOTS) {
        let _ = fs::remove_file(path);
    }
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drive_key_strips_colon_and_backslash() {
        assert_eq!(drive_key("C:\\"), "C");
        assert_eq!(drive_key("D:\\"), "D");
        assert_eq!(drive_key("C:"),   "C");
    }
}
