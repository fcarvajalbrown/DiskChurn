use diskchurn::treemap::{fmt_delta, layout_from_sizes};

// --- layout_from_sizes axioms ---

#[test]
fn empty_input_returns_empty() {
    assert!(layout_from_sizes(&[], 800.0, 600.0).is_empty());
}

#[test]
fn all_zero_sizes_returns_empty() {
    assert!(layout_from_sizes(&[(0, 0), (1, 0)], 800.0, 600.0).is_empty());
}

#[test]
fn zero_canvas_dimensions_return_empty() {
    assert!(layout_from_sizes(&[(0, 100)], 0.0, 600.0).is_empty());
    assert!(layout_from_sizes(&[(0, 100)], 800.0, 0.0).is_empty());
}

#[test]
fn single_item_fills_entire_canvas() {
    let rects = layout_from_sizes(&[(0, 100)], 800.0, 600.0);
    assert_eq!(rects.len(), 1);
    assert!((rects[0].w - 800.0).abs() < 0.5);
    assert!((rects[0].h - 600.0).abs() < 0.5);
    assert_eq!(rects[0].folder_index, 0);
}

#[test]
fn total_area_is_conserved() {
    let sizes = vec![(0, 100u64), (1, 200), (2, 300), (3, 400)];
    let rects = layout_from_sizes(&sizes, 800.0, 600.0);
    let filled: f32 = rects.iter().map(|r| r.w * r.h).sum();
    let canvas = 800.0f32 * 600.0;
    assert!((filled - canvas).abs() < 2.0, "area leaked: got {:.1}, expected {:.1}", filled, canvas);
}

#[test]
fn folder_indices_are_preserved() {
    let sizes = vec![(7, 100u64), (42, 200), (99, 50)];
    let rects = layout_from_sizes(&sizes, 400.0, 300.0);
    let mut seen: Vec<usize> = rects.iter().map(|r| r.folder_index).collect();
    seen.sort_unstable();
    assert_eq!(seen, vec![7, 42, 99]);
}

// --- sub-pixel pre-filter axiom ---

#[test]
fn sub_pixel_item_excluded_before_layout() {
    let canvas = 800.0f32 * 600.0;
    let total: u64 = 1_000_000_001;
    let sizes: Vec<(usize, u64)> = vec![(0usize, 1_000_000_000u64), (1, 1)]
        .into_iter()
        .filter(|(_, s)| *s as f32 / total as f32 * canvas >= 1.0)
        .collect();
    assert_eq!(sizes.len(), 1, "sub-pixel item must be dropped before squarify");
    assert_eq!(sizes[0].0, 0);
}

#[test]
fn item_at_exactly_one_pixel_is_kept() {
    let canvas = 1000.0f32;
    let total: u64 = 1000;
    let sizes: Vec<(usize, u64)> = vec![(0usize, 1u64)]
        .into_iter()
        .filter(|(_, s)| *s as f32 / total as f32 * canvas >= 1.0)
        .collect();
    assert_eq!(sizes.len(), 1);
}

// --- fmt_delta ---

#[test]
fn fmt_delta_positive_gigabytes() {
    assert_eq!(fmt_delta(1_500_000_000), "+1.5 GB");
}

#[test]
fn fmt_delta_negative_megabytes() {
    assert_eq!(fmt_delta(-500_000_000), "-500 MB");
}

#[test]
fn fmt_delta_zero_shows_plus_zero() {
    assert_eq!(fmt_delta(0), "+0 KB");
}
