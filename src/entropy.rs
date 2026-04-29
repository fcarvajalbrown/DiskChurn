// stub — full implementation coming next
use crate::types::{EntropyClass, FileNode};

pub fn sample_entropy(_file: &mut FileNode) {}

pub fn entropy_class(entropy: f32) -> EntropyClass {
    if entropy < 6.0 {
        EntropyClass::Compressible
    } else if entropy <= 7.2 {
        EntropyClass::Mixed
    } else {
        EntropyClass::Dense
    }
}
