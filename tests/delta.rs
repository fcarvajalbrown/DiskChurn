use diskchurn::delta::{apply_min_filter, compute, DeltaKind, FolderDelta};
use diskchurn::history::Snapshot;
use diskchurn::types::{ChurnClass, EntropyClass, FolderStats};
use std::path::PathBuf;

fn snap(folders: &[(&str, u64)]) -> Snapshot {
    Snapshot {
        drive: "C:\\".into(),
        total_bytes: 0,
        free_bytes: 0,
        files_scanned: 0,
        scanned_at: 0,
        folders: folders.iter().map(|(p, s)| FolderStats {
            path: PathBuf::from(p),
            total_size: *s,
            file_count: 1,
            churn: ChurnClass::Cold,
            entropy_class: EntropyClass::Mixed,
            days_until_full: None,
        }).collect(),
    }
}

fn current(folders: &[(&str, u64)]) -> Vec<FolderStats> {
    folders.iter().map(|(p, s)| FolderStats {
        path: PathBuf::from(p),
        total_size: *s,
        file_count: 1,
        churn: ChurnClass::Cold,
        entropy_class: EntropyClass::Mixed,
        days_until_full: None,
    }).collect()
}

// --- compute axioms ---

#[test]
fn grown_when_size_increases() {
    let d = compute(&snap(&[("C:\\a", 1000)]), &current(&[("C:\\a", 2000)]));
    assert_eq!(d[0].kind, DeltaKind::Grown);
    assert_eq!(d[0].delta, 1000);
    assert_eq!(d[0].old_size, 1000);
    assert_eq!(d[0].new_size, 2000);
}

#[test]
fn shrunk_when_size_decreases() {
    let d = compute(&snap(&[("C:\\a", 2000)]), &current(&[("C:\\a", 1000)]));
    assert_eq!(d[0].kind, DeltaKind::Shrunk);
    assert_eq!(d[0].delta, -1000);
}

#[test]
fn unchanged_requires_exact_zero_delta() {
    let d = compute(&snap(&[("C:\\a", 1000)]), &current(&[("C:\\a", 1000)]));
    assert_eq!(d[0].kind, DeltaKind::Unchanged);
    assert_eq!(d[0].delta, 0);
}

#[test]
fn one_byte_change_is_not_unchanged() {
    // this was suppressed by the old 4 KB heuristic — must now appear
    let d = compute(&snap(&[("C:\\a", 1000)]), &current(&[("C:\\a", 1001)]));
    assert_eq!(d[0].kind, DeltaKind::Grown);
}

#[test]
fn new_folder_absent_from_baseline() {
    let d = compute(&snap(&[]), &current(&[("C:\\new", 5000)]));
    assert_eq!(d[0].kind, DeltaKind::New);
    assert_eq!(d[0].old_size, 0);
    assert_eq!(d[0].delta, 5000);
}

#[test]
fn deleted_folder_absent_from_current() {
    let d = compute(&snap(&[("C:\\gone", 3000)]), &current(&[]));
    assert_eq!(d[0].kind, DeltaKind::Deleted);
    assert_eq!(d[0].new_size, 0);
    assert_eq!(d[0].delta, -3000);
}

#[test]
fn compute_never_filters_output() {
    // axiom: raw compute output contains every change, however small
    let baseline = snap(&[("C:\\a", 1000), ("C:\\b", 1000), ("C:\\c", 1000)]);
    let cur = current(&[
        ("C:\\a", 1001), // 1-byte change
        ("C:\\b", 1000), // exact zero
        ("C:\\d", 500),  // new
        // C:\\c absent → deleted
    ]);
    let d = compute(&baseline, &cur);
    assert_eq!(d.len(), 4, "compute must emit all 4 entries unfiltered");
}

// --- display filter axioms ---

#[test]
fn min_bytes_zero_passes_everything() {
    // axiom: the user can always see everything by setting min to 0
    let deltas = vec![
        FolderDelta { path: "C:\\a".into(), old_size: 100, new_size: 101, delta: 1, kind: DeltaKind::Grown },
    ];
    let out = apply_min_filter(&deltas, 0, [true; 4]);
    assert_eq!(out.len(), 1);
}

#[test]
fn min_bytes_excludes_below_threshold() {
    let deltas = vec![
        FolderDelta { path: "C:\\small".into(), old_size: 0, new_size: 100,       delta: 100,       kind: DeltaKind::Grown },
        FolderDelta { path: "C:\\large".into(), old_size: 0, new_size: 2_000_000, delta: 2_000_000, kind: DeltaKind::Grown },
    ];
    let out = apply_min_filter(&deltas, 1_000_000, [true; 4]);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].path, PathBuf::from("C:\\large"));
}

#[test]
fn kind_toggle_off_excludes_that_kind() {
    let deltas = vec![
        FolderDelta { path: "C:\\new".into(),   old_size: 0,   new_size: 500, delta: 500, kind: DeltaKind::New },
        FolderDelta { path: "C:\\grown".into(), old_size: 100, new_size: 600, delta: 500, kind: DeltaKind::Grown },
    ];
    let out = apply_min_filter(&deltas, 0, [false, true, true, true]);
    assert_eq!(out.len(), 1);
    assert_eq!(out[0].kind, DeltaKind::Grown);
}

#[test]
fn unchanged_is_always_excluded_from_display() {
    // axiom: Unchanged never surfaces in the display layer regardless of kind toggles
    let deltas = vec![
        FolderDelta { path: "C:\\same".into(), old_size: 1000, new_size: 1000, delta: 0, kind: DeltaKind::Unchanged },
    ];
    let out = apply_min_filter(&deltas, 0, [true; 4]);
    assert!(out.is_empty());
}
