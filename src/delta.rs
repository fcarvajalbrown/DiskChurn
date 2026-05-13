use std::collections::HashMap;
use std::path::PathBuf;

use crate::history::Snapshot;
use crate::types::FolderStats;

#[derive(Debug, Clone, PartialEq)]
pub enum DeltaKind {
    New,
    Deleted,
    Grown,
    Shrunk,
    Unchanged,
}

#[derive(Debug, Clone)]
pub struct FolderDelta {
    pub path: PathBuf,
    pub old_size: u64,
    pub new_size: u64,
    pub delta: i64,
    pub kind: DeltaKind,
}

pub fn apply_min_filter(deltas: &[FolderDelta], min_bytes: u64, show: [bool; 4]) -> Vec<&FolderDelta> {
    deltas
        .iter()
        .filter(|d| {
            if d.delta.unsigned_abs() < min_bytes {
                return false;
            }
            match d.kind {
                DeltaKind::New      => show[0],
                DeltaKind::Grown    => show[1],
                DeltaKind::Shrunk   => show[2],
                DeltaKind::Deleted  => show[3],
                DeltaKind::Unchanged => false,
            }
        })
        .collect()
}

pub fn compute(baseline: &Snapshot, current: &[FolderStats]) -> Vec<FolderDelta> {
    let mut base_map: HashMap<&PathBuf, u64> =
        baseline.folders.iter().map(|f| (&f.path, f.total_size)).collect();

    let mut out: Vec<FolderDelta> = Vec::new();

    for folder in current {
        match base_map.remove(&folder.path) {
            Some(old_size) => {
                let delta = folder.total_size as i64 - old_size as i64;
                let kind = if delta > 0 {
                    DeltaKind::Grown
                } else if delta < 0 {
                    DeltaKind::Shrunk
                } else {
                    DeltaKind::Unchanged
                };
                out.push(FolderDelta { path: folder.path.clone(), old_size, new_size: folder.total_size, delta, kind });
            }
            None => {
                out.push(FolderDelta {
                    path: folder.path.clone(),
                    old_size: 0,
                    new_size: folder.total_size,
                    delta: folder.total_size as i64,
                    kind: DeltaKind::New,
                });
            }
        }
    }

    for (path, old_size) in base_map {
        out.push(FolderDelta {
            path: path.clone(),
            old_size,
            new_size: 0,
            delta: -(old_size as i64),
            kind: DeltaKind::Deleted,
        });
    }

    out
}
