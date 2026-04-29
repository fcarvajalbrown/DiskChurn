use std::time::{SystemTime, UNIX_EPOCH};

use crate::types::{ChurnClass, FileNode, FolderStats, EntropyClass};

const HOT_WINDOW_DAYS: f64 = 30.0;
const VOLATILE_WINDOW_DAYS: f64 = 7.0;
const VOLATILE_MIN_FILE_COUNT: u64 = 20;
const COLD_UNTOUCHED_DAYS: f64 = 90.0;

pub fn classify(files: &[FileNode], drive_total_bytes: u64, drive_free_bytes: u64) -> Vec<FolderStats> {
    let mut folders = build_folder_map(files);
    for folder in &mut folders {
        folder.churn = assign_churn(folder, files);
        if folder.churn == ChurnClass::Hot {
            folder.days_until_full =
                project_days_until_full(folder, files, drive_total_bytes, drive_free_bytes);
        }
    }
    folders
}

fn build_folder_map(files: &[FileNode]) -> Vec<FolderStats> {
    use std::collections::HashMap;

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
            reclaimable_bytes: None,
            children: vec![],
        })
        .collect()
}

fn assign_churn(folder: &FolderStats, files: &[FileNode]) -> ChurnClass {
    let now = now_secs();
    let hot_cutoff = now - (HOT_WINDOW_DAYS * 86_400.0) as u64;
    let volatile_cutoff = now - (VOLATILE_WINDOW_DAYS * 86_400.0) as u64;
    let cold_cutoff = now - (COLD_UNTOUCHED_DAYS * 86_400.0) as u64;

    let folder_files: Vec<&FileNode> = files
        .iter()
        .filter(|f| f.path.parent().map(|p| p == folder.path).unwrap_or(false))
        .collect();

    if folder_files.is_empty() {
        return ChurnClass::Cold;
    }

    let recently_modified = folder_files
        .iter()
        .filter(|f| to_secs(f.modified) >= volatile_cutoff)
        .count();

    let total = folder_files.len();

    // volatile: many short-lived files churning fast
    let volatile_ratio = recently_modified as f64 / total as f64;
    if folder.file_count >= VOLATILE_MIN_FILE_COUNT && volatile_ratio > 0.6 {
        let avg_size = folder.total_size / folder.file_count.max(1);
        if avg_size < 10 * 1024 * 1024 {
            return ChurnClass::Volatile;
        }
    }

    // hot: growing — significant recent modification activity
    let hot_modified = folder_files
        .iter()
        .filter(|f| to_secs(f.modified) >= hot_cutoff)
        .count();
    let hot_ratio = hot_modified as f64 / total as f64;
    if hot_ratio > 0.3 {
        return ChurnClass::Hot;
    }

    // cold: nothing touched recently
    let any_recent = folder_files
        .iter()
        .any(|f| to_secs(f.modified) >= cold_cutoff);
    if !any_recent {
        return ChurnClass::Cold;
    }

    ChurnClass::Cold
}

// linear regression on (modified_timestamp, cumulative_size) to extrapolate fill date
fn project_days_until_full(
    folder: &FolderStats,
    files: &[FileNode],
    drive_total: u64,
    drive_free: u64,
) -> Option<f32> {
    let mut points: Vec<(f64, f64)> = files
        .iter()
        .filter(|f| f.path.parent().map(|p| p == folder.path).unwrap_or(false))
        .map(|f| (to_secs(f.modified) as f64, f.size_bytes as f64))
        .collect();

    if points.len() < 3 {
        return None;
    }

    points.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

    // cumulative size over time
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
        return None; // not growing
    }

    let free = drive_free as f64;
    let days_left = (free / slope) / 86_400.0;

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
