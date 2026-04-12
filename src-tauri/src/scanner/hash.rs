use image::DynamicImage;
use image_hasher::{HashAlg, HasherConfig};

fn make_hasher() -> image_hasher::Hasher {
    HasherConfig::new()
        .hash_alg(HashAlg::Gradient)
        .hash_size(8, 8)
        .to_hasher()
}

/// Hash an already-decoded image (avoids a second decode).
pub fn compute_from_image(img: &DynamicImage) -> Option<String> {
    let hash = make_hasher().hash_image(img);
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
