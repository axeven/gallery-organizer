use std::path::Path;

use image_hasher::{HashAlg, HasherConfig};

pub fn compute(path: &Path) -> Option<String> {
    let img = image::open(path).ok()?;
    let hasher = HasherConfig::new()
        .hash_alg(HashAlg::Gradient)
        .hash_size(8, 8)
        .to_hasher();
    let hash = hasher.hash_image(&img);
    Some(hex::encode(hash.as_bytes()))
}

pub fn hamming_distance(a: &str, b: &str) -> Option<u32> {
    let a_bytes = hex::decode(a).ok()?;
    let b_bytes = hex::decode(b).ok()?;
    if a_bytes.len() != b_bytes.len() {
        return None;
    }
    let dist = a_bytes
        .iter()
        .zip(b_bytes.iter())
        .map(|(x, y)| (x ^ y).count_ones())
        .sum();
    Some(dist)
}
