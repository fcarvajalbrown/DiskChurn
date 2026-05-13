use std::collections::HashMap;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::{ChurnClass, EntropyClass, FileNode, FolderStats};

const HOT_WINDOW_DAYS: f64 = 30.0;
const VOLATILE_WINDOW_DAYS: f64 = 7.0;
const VOLATILE_MIN_FILE_COUNT: u64 = 20;
const COLD_UNTOUCHED_DAYS: f64 = 90.0;

pub fn classify(files: &[FileNode], _drive_total_bytes: u64, drive_free_bytes: u64) -> Vec<FolderStats> {
    let mut folders = build_folder_map(files);

    // index by parent once so per-folder passes are O(1) lookup instead of O(n) scan
    let mut by_parent: HashMap<&Path, Vec<&FileNode>> = HashMap::new();
    for f in files {
        if let Some(p) = f.path.parent() {
            by_parent.entry(p).or_default().push(f);
        }
    }

    for folder in &mut folders {
        let folder_files = by_parent.get(folder.path.as_path()).map_or(&[][..], |v| v.as_slice());
        folder.churn = assign_churn(folder, folder_files);
        folder.entropy_class = assign_entropy_class(folder_files);
        if folder.churn == ChurnClass::Hot {
            folder.days_until_full = project_days_until_full(folder_files, drive_free_bytes);
        }
    }
    folders
}

fn build_folder_map(files: &[FileNode]) -> Vec<FolderStats> {
    let mut map: HashMap<std::path::PathBuf, (u64, u64)> = HashMap::new();

    for f in files {
        if let Some(parent) = f.path.parent() {
            let entry = map.entry(parent.to_path_buf()).or_insert((0, 0));
            entry.0 += f.size_bytes;
            entry.1 += 1;
        }
    }

    map.into_iter()
        .map(|(path, (total_size, file_count))| FolderStats {
            path,
            total_size,
            file_count,
            churn: ChurnClass::Cold,
            entropy_class: EntropyClass::Mixed,
            days_until_full: None,
        })
        .collect()
}

fn assign_churn(folder: &FolderStats, folder_files: &[&FileNode]) -> ChurnClass {
    if folder_files.is_empty() {
        return ChurnClass::Cold;
    }

    let now = now_secs();
    let hot_cutoff = now - (HOT_WINDOW_DAYS * 86_400.0) as u64;
    let volatile_cutoff = now - (VOLATILE_WINDOW_DAYS * 86_400.0) as u64;
    let cold_cutoff = now - (COLD_UNTOUCHED_DAYS * 86_400.0) as u64;

    let total = folder_files.len();
    let recently_modified = folder_files
        .iter()
        .filter(|f| to_secs(f.modified) >= volatile_cutoff)
        .count();

    let volatile_ratio = recently_modified as f64 / total as f64;
    if folder.file_count >= VOLATILE_MIN_FILE_COUNT && volatile_ratio > 0.6 {
        let avg_size = folder.total_size / folder.file_count.max(1);
        if avg_size < 10 * 1024 * 1024 {
            return ChurnClass::Volatile;
        }
    }

    let hot_modified = folder_files
        .iter()
        .filter(|f| to_secs(f.modified) >= hot_cutoff)
        .count();
    if hot_modified as f64 / total as f64 > 0.3 {
        return ChurnClass::Hot;
    }

    if !folder_files.iter().any(|f| to_secs(f.modified) >= cold_cutoff) {
        return ChurnClass::Cold;
    }

    ChurnClass::Cold
}

fn assign_entropy_class(folder_files: &[&FileNode]) -> EntropyClass {
    let entropies: Vec<f32> = folder_files.iter().filter_map(|f| f.entropy).collect();
    if entropies.is_empty() {
        return EntropyClass::Mixed;
    }
    let avg = entropies.iter().sum::<f32>() / entropies.len() as f32;
    crate::entropy::entropy_class(avg)
}

// linear regression on (modified_timestamp, cumulative_size) to extrapolate fill date
fn project_days_until_full(folder_files: &[&FileNode], drive_free: u64) -> Option<f32> {
    let mut points: Vec<(f64, f64)> = folder_files
        .iter()
        .map(|f| (to_secs(f.modified) as f64, f.size_bytes as f64))
        .collect();

    if points.len() < 3 {
        return None;
    }

    points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    let mut cum = 0.0f64;
    let points: Vec<(f64, f64)> = points
        .into_iter()
        .map(|(t, s)| {
            cum += s;
            (t, cum)
        })
        .collect();

    let (slope, _intercept) = linear_regression(&points)?;

    if slope <= 0.0 {
        return None;
    }

    let days_left = (drive_free as f64 / slope) / 86_400.0;

    if days_left > 0.0 && days_left < 3650.0 {
        Some(days_left as f32)
    } else {
        None
    }
}

fn linear_regression(points: &[(f64, f64)]) -> Option<(f64, f64)> {
    let n = points.len() as f64;
    let sum_x: f64 = points.iter().map(|(x, _)| x).sum();
    let sum_y: f64 = points.iter().map(|(_, y)| y).sum();
    let sum_xx: f64 = points.iter().map(|(x, _)| x * x).sum();
    let sum_xy: f64 = points.iter().map(|(x, y)| x * y).sum();

    let denom = n * sum_xx - sum_x * sum_x;
    if denom.abs() < f64::EPSILON {
        return None;
    }

    let slope = (n * sum_xy - sum_x * sum_y) / denom;
    let intercept = (sum_y - slope * sum_x) / n;
    Some((slope, intercept))
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn to_secs(t: SystemTime) -> u64 {
    t.duration_since(UNIX_EPOCH).unwrap_or_default().as_secs()
}
