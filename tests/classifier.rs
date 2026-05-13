use diskchurn::classifier::classify;
use diskchurn::types::{ChurnClass, FileNode};
use std::path::PathBuf;
use std::time::{Duration, UNIX_EPOCH};

fn file_at(path: &str, size: u64, days_ago: u64) -> FileNode {
    let secs = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
        .saturating_sub(days_ago * 86_400);
    FileNode {
        path: PathBuf::from(path),
        size_bytes: size,
        modified: UNIX_EPOCH + Duration::from_secs(secs),
        entropy: None,
    }
}

// --- size aggregation ---

#[test]
fn sizes_and_counts_aggregate_per_parent() {
    let files = vec![
        file_at("C:\\foo\\a.txt", 1_000, 0),
        file_at("C:\\foo\\b.txt", 2_000, 0),
        file_at("C:\\bar\\c.txt",   500, 0),
    ];
    let folders = classify(&files, 0, 0);
    let foo = folders.iter().find(|f| f.path == PathBuf::from("C:\\foo")).unwrap();
    assert_eq!(foo.total_size, 3_000);
    assert_eq!(foo.file_count, 2);
    let bar = folders.iter().find(|f| f.path == PathBuf::from("C:\\bar")).unwrap();
    assert_eq!(bar.total_size, 500);
    assert_eq!(bar.file_count, 1);
}

// --- ChurnClass axioms: cutoffs are HOT=30d, VOLATILE=7d+20files, COLD=90d ---

#[test]
fn untouched_120_days_is_cold() {
    let files = vec![
        file_at("C:\\archive\\a.zip", 1_000_000, 120),
        file_at("C:\\archive\\b.zip", 2_000_000, 150),
    ];
    let folders = classify(&files, 1_000_000_000, 500_000_000);
    let f = folders.iter().find(|f| f.path == PathBuf::from("C:\\archive")).unwrap();
    assert_eq!(f.churn, ChurnClass::Cold);
}

#[test]
fn majority_modified_within_30_days_is_hot() {
    // 5 files all touched 5 days ago → hot_ratio = 5/5 = 100% > 30% threshold
    let files: Vec<FileNode> = (0..5)
        .map(|i| file_at(&format!("C:\\logs\\f{}.log", i), 100_000, 5))
        .collect();
    let folders = classify(&files, 1_000_000_000, 500_000_000);
    let f = folders.iter().find(|f| f.path == PathBuf::from("C:\\logs")).unwrap();
    assert_eq!(f.churn, ChurnClass::Hot);
}

#[test]
fn many_small_recent_files_is_volatile() {
    // VOLATILE requires >= 20 files, >60% modified in last 7 days, avg size < 10 MB
    let files: Vec<FileNode> = (0..25)
        .map(|i| file_at(&format!("C:\\cache\\tmp{}.dat", i), 4_096, 2))
        .collect();
    let folders = classify(&files, 1_000_000_000, 500_000_000);
    let f = folders.iter().find(|f| f.path == PathBuf::from("C:\\cache")).unwrap();
    assert_eq!(f.churn, ChurnClass::Volatile);
}

#[test]
fn volatile_requires_minimum_20_files() {
    // only 10 files — can't be volatile even if all recent and small
    let files: Vec<FileNode> = (0..10)
        .map(|i| file_at(&format!("C:\\small\\f{}.dat", i), 100, 1))
        .collect();
    let folders = classify(&files, 1_000_000_000, 500_000_000);
    let f = folders.iter().find(|f| f.path == PathBuf::from("C:\\small")).unwrap();
    assert_ne!(f.churn, ChurnClass::Volatile);
}

// --- pre-index correctness ---

#[test]
fn files_from_different_folders_do_not_cross_contaminate() {
    let files = vec![
        file_at("C:\\hot\\a.log",      1_000, 1),
        file_at("C:\\cold\\b.zip", 1_000_000, 180),
    ];
    let folders = classify(&files, 1_000_000_000, 500_000_000);
    let hot  = folders.iter().find(|f| f.path == PathBuf::from("C:\\hot")).unwrap();
    let cold = folders.iter().find(|f| f.path == PathBuf::from("C:\\cold")).unwrap();
    assert_eq!(hot.churn,  ChurnClass::Hot);
    assert_eq!(cold.churn, ChurnClass::Cold);
}
