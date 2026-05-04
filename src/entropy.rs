use std::fs::File;
use std::io::Read;
use crate::types::{EntropyClass, FileNode};

const SAMPLE_SIZE: usize = 65536; // 64 KB

pub fn sample_entropy(file: &mut FileNode) {
    let Ok(mut f) = File::open(&file.path) else { return };
    let mut buf = vec![0u8; SAMPLE_SIZE];
    let Ok(n) = f.read(&mut buf) else { return };
    if n == 0 { return }
    let buf = &buf[..n];

    let mut counts = [0u64; 256];
    for &b in buf { counts[b as usize] += 1; }

    let len = n as f64;
    let entropy = counts.iter().filter(|&&c| c > 0).fold(0f64, |acc, &c| {
        let p = c as f64 / len;
        acc - p * p.log2()
    });

    file.entropy = Some(entropy as f32);
}

pub fn entropy_class(entropy: f32) -> EntropyClass {
    if entropy < 6.0 {
        EntropyClass::Compressible
    } else if entropy <= 7.2 {
        EntropyClass::Mixed
    } else {
        EntropyClass::Dense
    }
}
