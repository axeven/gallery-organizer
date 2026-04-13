use anyhow::{Context, Result};
use fast_image_resize::images::Image as FirImage;
use fast_image_resize::{PixelType, ResizeAlg, ResizeOptions, Resizer};
use serde::{Deserialize, Serialize};
use std::path::Path;
use zune_jpeg::zune_core::colorspace::ColorSpace;
use zune_jpeg::zune_core::options::DecoderOptions;
use zune_jpeg::zune_core::bytestream::ZCursor;
use zune_jpeg::JpegDecoder;

/// How to resize images in a process job.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", tag = "mode")]
pub enum ResizeMode {
    /// No resizing.
    None,
    /// Scale to exactly half the original pixel dimensions.
    Half,
    /// Fit within a fixed bounding box (W×H), auto-swapped for portrait images.
    /// Never upscales.
    Fixed { width: u32, height: u32 },
}

impl Default for ResizeMode {
    fn default() -> Self {
        ResizeMode::None
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessParams {
    pub quality: Option<u8>,
    /// Legacy flat fields kept for backwards compat with old jobs stored in DB.
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fit: Option<String>,
    pub target_format: Option<String>,
    /// New unified resize spec. Takes precedence over width/height when present.
    #[serde(default)]
    pub resize: ResizeMode,
}

/// Decode a JPEG to raw RGB bytes using zune-jpeg (faster than the image crate).
/// Falls back to the image crate on any failure.
fn decode_to_rgb(path: &Path) -> Result<(u32, u32, Vec<u8>)> {
    let is_jpeg = path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| matches!(e.to_lowercase().as_str(), "jpg" | "jpeg"))
        .unwrap_or(false);

    if is_jpeg {
        let bytes = std::fs::read(path).context("failed to read file")?;
        let options = DecoderOptions::default().jpeg_set_out_colorspace(ColorSpace::RGB);
        let mut decoder = JpegDecoder::new_with_options(ZCursor::new(&bytes), options);
        // info() must be called after decode_headers() but before decode()
        if decoder.decode_headers().is_ok() {
            if let Some(info) = decoder.info() {
                let w = info.width as u32;
                let h = info.height as u32;
                if let Ok(pixels) = decoder.decode() {
                    return Ok((w, h, pixels));
                }
            }
        }
        // Fall through to image crate on any zune failure
    }

    let img = image::open(path).context("failed to open image")?;
    let (w, h) = (img.width(), img.height());
    Ok((w, h, img.to_rgb8().into_raw()))
}

/// Returns `(encoded_bytes, extension, out_width, out_height)`.
pub fn process_image(
    path: &Path,
    params: &ProcessParams,
    default_format: &str,
) -> Result<(Vec<u8>, String, u32, u32)> {
    let (orig_w, orig_h, rgb_raw) = decode_to_rgb(path)?;

    // Determine resize target dimensions
    let resize_target: Option<(u32, u32)> = match &params.resize {
        ResizeMode::None => {
            if params.width.is_some() || params.height.is_some() {
                let fit = params.fit.as_deref().unwrap_or("contain");
                Some(compute_legacy_size(
                    orig_w,
                    orig_h,
                    params.width,
                    params.height,
                    fit,
                ))
            } else {
                None
            }
        }
        ResizeMode::Half => Some((orig_w / 2, orig_h / 2)),
        ResizeMode::Fixed { width, height } => {
            let (target_w, target_h) =
                orientation_aware_target(orig_w, orig_h, *width, *height);
            let scale = f64::min(
                target_w as f64 / orig_w as f64,
                target_h as f64 / orig_h as f64,
            )
            .min(1.0);
            let new_w = (orig_w as f64 * scale).round() as u32;
            let new_h = (orig_h as f64 * scale).round() as u32;
            if new_w == orig_w && new_h == orig_h {
                None
            } else {
                Some((new_w, new_h))
            }
        }
    };

    let format = params.target_format.as_deref().unwrap_or(default_format);
    let quality = params.quality.unwrap_or(85);

    // ── Resize with fast_image_resize (SIMD) ─────────────────────────────────
    let (out_w, out_h, rgb_data) = if let Some((new_w, new_h)) = resize_target {
        if new_w > 0 && new_h > 0 {
            let src_image = FirImage::from_vec_u8(
                orig_w,
                orig_h,
                rgb_raw,
                PixelType::U8x3,
            )
            .context("failed to create source image")?;

            let mut dst_image = FirImage::new(new_w, new_h, PixelType::U8x3);

            let mut resizer = Resizer::new();
            resizer
                .resize(
                    &src_image,
                    &mut dst_image,
                    &ResizeOptions::new().resize_alg(ResizeAlg::Convolution(
                        fast_image_resize::FilterType::Lanczos3,
                    )),
                )
                .context("resize failed")?;

            (new_w, new_h, dst_image.into_vec())
        } else {
            (orig_w, orig_h, rgb_raw)
        }
    } else {
        (orig_w, orig_h, rgb_raw)
    };

    // ── Encode ────────────────────────────────────────────────────────────────
    match format {
        "webp" => {
            // webp crate works on raw RGBA; convert RGB → RGBA first
            let rgba: Vec<u8> = rgb_data
                .chunks_exact(3)
                .flat_map(|p| [p[0], p[1], p[2], 255u8])
                .collect();
            let encoder = webp::Encoder::from_rgba(&rgba, out_w, out_h);
            let webp_data = encoder.encode(quality as f32);
            Ok((webp_data.to_vec(), "webp".into(), out_w, out_h))
        }
        _ => {
            // JPEG via mozjpeg (libjpeg-turbo SIMD) — much faster than pure Rust encoder
            let encoded = std::panic::catch_unwind(|| -> Vec<u8> {
                let mut buf = Vec::new();
                let mut compress = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
                compress.set_size(out_w as usize, out_h as usize);
                compress.set_quality(quality as f32);
                let mut started = compress.start_compress(&mut buf).unwrap();
                started.write_scanlines(&rgb_data).unwrap();
                started.finish().unwrap();
                buf
            })
            .map_err(|_| anyhow::anyhow!("mozjpeg encode failed"))?;
            Ok((encoded, "jpg".into(), out_w, out_h))
        }
    }
}

/// Swap target dimensions when original and target orientations differ.
fn orientation_aware_target(orig_w: u32, orig_h: u32, target_w: u32, target_h: u32) -> (u32, u32) {
    let orig_portrait = orig_h > orig_w;
    let target_portrait = target_h > target_w;
    if orig_portrait != target_portrait {
        (target_h, target_w)
    } else {
        (target_w, target_h)
    }
}

fn compute_legacy_size(
    orig_w: u32,
    orig_h: u32,
    width: Option<u32>,
    height: Option<u32>,
    fit: &str,
) -> (u32, u32) {
    match fit {
        "exact" => (width.unwrap_or(orig_w), height.unwrap_or(orig_h)),
        "cover" => {
            let tw = width.unwrap_or(orig_w);
            let th = height.unwrap_or(orig_h);
            let scale = f64::max(tw as f64 / orig_w as f64, th as f64 / orig_h as f64);
            (
                (orig_w as f64 * scale).round() as u32,
                (orig_h as f64 * scale).round() as u32,
            )
        }
        _ => {
            let tw = width.unwrap_or(u32::MAX);
            let th = height.unwrap_or(u32::MAX);
            let scale =
                f64::min(tw as f64 / orig_w as f64, th as f64 / orig_h as f64).min(1.0);
            (
                (orig_w as f64 * scale).round() as u32,
                (orig_h as f64 * scale).round() as u32,
            )
        }
    }
}

/// Estimate the output pixel dimensions for a given image size and resize params.
pub fn estimate_output_size(orig_w: u32, orig_h: u32, resize: &ResizeMode) -> (u32, u32) {
    match resize {
        ResizeMode::None => (orig_w, orig_h),
        ResizeMode::Half => (orig_w / 2, orig_h / 2),
        ResizeMode::Fixed { width, height } => {
            let (target_w, target_h) = orientation_aware_target(orig_w, orig_h, *width, *height);
            let scale = f64::min(
                target_w as f64 / orig_w as f64,
                target_h as f64 / orig_h as f64,
            )
            .min(1.0);
            (
                (orig_w as f64 * scale).round() as u32,
                (orig_h as f64 * scale).round() as u32,
            )
        }
    }
}
