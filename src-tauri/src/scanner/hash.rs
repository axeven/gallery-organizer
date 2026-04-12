use std::path::Path;

use image::{DynamicImage, ImageBuffer, Luma};
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

/// Decode a JPEG using zune-jpeg (faster than the image crate), converting
/// directly to grayscale — hashing doesn't need color, so this cuts memory
/// by 3× vs RGB decode. Returns (width_px, height_px, perceptual_hash).
pub fn decode_jpeg_fast(path: &Path) -> (Option<i64>, Option<i64>, Option<String>) {
    use zune_jpeg::JpegDecoder;
    use zune_jpeg::zune_core::bytestream::ZCursor;
    use zune_jpeg::zune_core::colorspace::ColorSpace;
    use zune_jpeg::zune_core::options::DecoderOptions;

    let bytes = match std::fs::read(path) {
        Ok(b) => b,
        Err(_) => return (None, None, None),
    };

    let options = DecoderOptions::default()
        .jpeg_set_out_colorspace(ColorSpace::Luma);

    let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&bytes), options);

    if decoder.decode_headers().is_err() {
        return (None, None, None);
    }

    let info = match decoder.info() {
        Some(i) => i,
        None => return (None, None, None),
    };
    let full_w = info.width as i64;
    let full_h = info.height as i64;

    let pixels = match decoder.decode() {
        Ok(p) => p,
        Err(_) => return (Some(full_w), Some(full_h), None),
    };

    let img_buf = match ImageBuffer::<Luma<u8>, _>::from_raw(full_w as u32, full_h as u32, pixels) {
        Some(b) => b,
        None => return (Some(full_w), Some(full_h), None),
    };

    // Shrink before hashing — the hasher only needs 8×8 pixels.
    let dyn_img = DynamicImage::ImageLuma8(img_buf);
    let small = dyn_img.thumbnail(64, 64);
    let hash = compute_from_image(&small);

    (Some(full_w), Some(full_h), hash)
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
