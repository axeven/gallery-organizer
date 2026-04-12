use anyhow::{Context, Result};
use image::imageops::FilterType;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::path::Path;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessParams {
    pub quality: Option<u8>,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub fit: Option<String>,
    pub target_format: Option<String>,
}

pub fn process_image(path: &Path, params: &ProcessParams, default_format: &str) -> Result<(Vec<u8>, String)> {
    let img = image::open(path).context("failed to open image")?;

    // Apply resize if requested
    let img = if params.width.is_some() || params.height.is_some() {
        let (orig_w, orig_h) = (img.width(), img.height());
        let fit = params.fit.as_deref().unwrap_or("contain");

        let (new_w, new_h) = match fit {
            "exact" => (
                params.width.unwrap_or(orig_w),
                params.height.unwrap_or(orig_h),
            ),
            "cover" => {
                let target_w = params.width.unwrap_or(orig_w);
                let target_h = params.height.unwrap_or(orig_h);
                let scale = f64::max(
                    target_w as f64 / orig_w as f64,
                    target_h as f64 / orig_h as f64,
                );
                (
                    (orig_w as f64 * scale).round() as u32,
                    (orig_h as f64 * scale).round() as u32,
                )
            }
            _ => {
                // contain: fit within bounds, preserve aspect ratio
                let target_w = params.width.unwrap_or(u32::MAX);
                let target_h = params.height.unwrap_or(u32::MAX);
                let scale = f64::min(
                    target_w as f64 / orig_w as f64,
                    target_h as f64 / orig_h as f64,
                );
                let scale = scale.min(1.0); // never upscale
                (
                    (orig_w as f64 * scale).round() as u32,
                    (orig_h as f64 * scale).round() as u32,
                )
            }
        };

        img.resize_exact(new_w, new_h, FilterType::Lanczos3)
    } else {
        img
    };

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
            Ok((webp_data.to_vec(), "webp".into()))
        }
        _ => {
            // Default to JPEG
            let rgb = img.to_rgb8();
            let mut buf = Cursor::new(Vec::new());
            let encoder = image::codecs::jpeg::JpegEncoder::new_with_quality(&mut buf, quality);
            rgb.write_with_encoder(encoder)
                .context("failed to encode JPEG")?;
            Ok((buf.into_inner(), "jpeg".into()))
        }
    }
}
