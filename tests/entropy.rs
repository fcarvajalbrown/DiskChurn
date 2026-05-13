use diskchurn::entropy::entropy_class;
use diskchurn::types::EntropyClass;

fn shannon(buf: &[u8]) -> f32 {
    let mut counts = [0u64; 256];
    for &b in buf { counts[b as usize] += 1; }
    let n = buf.len() as f64;
    counts.iter().filter(|&&c| c > 0).fold(0f64, |acc, &c| {
        let p = c as f64 / n;
        acc - p * p.log2()
    }) as f32
}

// --- entropy math axioms ---

#[test]
fn single_byte_value_has_zero_entropy() {
    let buf = vec![0x42u8; 4096];
    let h = shannon(&buf);
    assert!(h < 0.001, "constant byte stream must have H=0, got {}", h);
}

#[test]
fn uniform_distribution_approaches_eight() {
    let buf: Vec<u8> = (0u8..=255).cycle().take(1024).collect();
    let h = shannon(&buf);
    assert!((h - 8.0).abs() < 0.01, "uniform dist must have H≈8.0, got {}", h);
}

// --- class boundary axioms ---

#[test]
fn below_six_is_compressible() {
    assert_eq!(entropy_class(0.0),  EntropyClass::Compressible);
    assert_eq!(entropy_class(5.99), EntropyClass::Compressible);
}

#[test]
fn boundary_six_is_mixed() {
    assert_eq!(entropy_class(6.0), EntropyClass::Mixed);
}

#[test]
fn boundary_seven_point_two_is_mixed() {
    assert_eq!(entropy_class(7.2), EntropyClass::Mixed);
}

#[test]
fn above_seven_point_two_is_dense() {
    assert_eq!(entropy_class(7.201), EntropyClass::Dense);
    assert_eq!(entropy_class(8.0),   EntropyClass::Dense);
}

#[test]
fn uniform_buffer_classifies_dense() {
    let buf: Vec<u8> = (0u8..=255).cycle().take(65536).collect();
    assert_eq!(entropy_class(shannon(&buf)), EntropyClass::Dense);
}

#[test]
fn all_zeros_classifies_compressible() {
    let buf = vec![0u8; 65536];
    assert_eq!(entropy_class(shannon(&buf)), EntropyClass::Compressible);
}
