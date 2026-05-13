use diskchurn::history::Snapshot;
use diskchurn::types::{ChurnClass, EntropyClass, FolderStats};
use std::path::PathBuf;

fn make_snapshot() -> Snapshot {
    Snapshot {
        drive: "C:\\".into(),
        total_bytes: 500_000_000_000,
        free_bytes: 200_000_000_000,
        files_scanned: 98_765,
        scanned_at: 1_715_000_000,
        folders: vec![
            FolderStats {
                path: PathBuf::from("C:\\Windows\\System32"),
                total_size: 1_234_567_890,
                file_count: 4_321,
                churn: ChurnClass::Cold,
                entropy_class: EntropyClass::Dense,
                days_until_full: None,
            },
            FolderStats {
                path: PathBuf::from("C:\\Users\\Felipe\\AppData\\Local\\Temp"),
                total_size: 987_654,
                file_count: 42,
                churn: ChurnClass::Volatile,
                entropy_class: EntropyClass::Mixed,
                days_until_full: Some(14.5),
            },
        ],
    }
}

#[test]
fn snapshot_round_trips_through_bincode() {
    let original = make_snapshot();
    let encoded = bincode::serialize(&original).expect("serialize failed");
    let decoded: Snapshot = bincode::deserialize(&encoded).expect("deserialize failed");

    assert_eq!(decoded.drive,          original.drive);
    assert_eq!(decoded.total_bytes,    original.total_bytes);
    assert_eq!(decoded.free_bytes,     original.free_bytes);
    assert_eq!(decoded.files_scanned,  original.files_scanned);
    assert_eq!(decoded.scanned_at,     original.scanned_at);
    assert_eq!(decoded.folders.len(),  original.folders.len());
}

#[test]
fn round_trip_preserves_folder_fields() {
    let original = make_snapshot();
    let encoded = bincode::serialize(&original).unwrap();
    let decoded: Snapshot = bincode::deserialize(&encoded).unwrap();

    let orig_f = &original.folders[0];
    let dec_f  = &decoded.folders[0];
    assert_eq!(dec_f.path,           orig_f.path);
    assert_eq!(dec_f.total_size,     orig_f.total_size);
    assert_eq!(dec_f.file_count,     orig_f.file_count);
    assert_eq!(dec_f.churn,          orig_f.churn);
    assert_eq!(dec_f.entropy_class,  orig_f.entropy_class);
    assert_eq!(dec_f.days_until_full, orig_f.days_until_full);
}

#[test]
fn round_trip_preserves_optional_days_until_full() {
    let original = make_snapshot();
    let encoded = bincode::serialize(&original).unwrap();
    let decoded: Snapshot = bincode::deserialize(&encoded).unwrap();

    assert!(decoded.folders[0].days_until_full.is_none());
    let days = decoded.folders[1]
        .days_until_full
        .expect("Some(14.5) should survive round-trip");
    assert!((days - 14.5).abs() < 0.001);
}
