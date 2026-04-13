use anyhow::{Context, Result};
use image::imageops::FilterType;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::path::Path;

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

/// Returns `(encoded_bytes, extension, out_width, out_height)`.
pub fn process_image(path: &Path, params: &ProcessParams, default_format: &str) -> Result<(Vec<u8>, String, u32, u32)> {
    let img = image::open(path).context("failed to open image")?;
    let (orig_w, orig_h) = (img.width(), img.height());

    // Determine resize target
    let resize_target: Option<(u32, u32)> = match &params.resize {
        ResizeMode::None => {
            // Fall back to legacy width/height fields
            if params.width.is_some() || params.height.is_some() {
                let fit = params.fit.as_deref().unwrap_or("contain");
                Some(compute_legacy_size(orig_w, orig_h, params.width, params.height, fit))
            } else {
                None
            }
        }
        ResizeMode::Half => {
            Some((orig_w / 2, orig_h / 2))
        }
        ResizeMode::Fixed { width, height } => {
            // Orientation-aware swap: if original is portrait but target is landscape, swap target
            let (target_w, target_h) = orientation_aware_target(orig_w, orig_h, *width, *height);
            // Fit within target, never upscale
            let scale = f64::min(
                target_w as f64 / orig_w as f64,
                target_h as f64 / orig_h as f64,
            )
            .min(1.0);
            let new_w = (orig_w as f64 * scale).round() as u32;
            let new_h = (orig_h as f64 * scale).round() as u32;
            if new_w == orig_w && new_h == orig_h { None } else { Some((new_w, new_h)) }
        }
    };

    // Apply resize if requested
    let img = if let Some((new_w, new_h)) = resize_target {
        if new_w > 0 && new_h > 0 {
            img.resize_exact(new_w, new_h, FilterType::Lanczos3)
        } else {
            img
        }
    } else {
        img
    };

    let out_w = img.width();
    let out_h = img.height();

    let format = params
        .target_format
        .as_deref()
        .unwrap_or(default_format);

    let quality = params.quality.unwrap_or(85);

    match format {
        "webp" => {
            let rgba = img.to_rgba8();
            let encoder = webp::Encoder::from_rgba(&rgba, rgba.width(), rgba.height());
            let webp_data = encoder.encode(quality as f32);
            Ok((webp_data.to_vec(), "webp".into(), out_w, out_h))
        }
        _ => {
            // Default to JPEG
            let rgb = img.to_rgb8();
            let mut buf = Cursor::new(Vec::new());
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
            rgb.write_with_encoder(encoder)
                .context("failed to encode JPEG")?;
            Ok((buf.into_inner(), "jpeg".into(), out_w, out_h))
        }
    }
}

/// Swap target dimensions when original and target orientations differ.
/// e.g. original is portrait (h > w) but target is landscape (w > h) → swap target to portrait.
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
            ((orig_w as f64 * scale).round() as u32, (orig_h as f64 * scale).round() as u32)
        }
        _ => {
            let tw = width.unwrap_or(u32::MAX);
            let th = height.unwrap_or(u32::MAX);
            let scale = f64::min(tw as f64 / orig_w as f64, th as f64 / orig_h as f64).min(1.0);
            ((orig_w as f64 * scale).round() as u32, (orig_h as f64 * scale).round() as u32)
        }
    }
}

/// Estimate the output pixel dimensions for a given image size and resize params.
/// Returns (out_w, out_h).
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
