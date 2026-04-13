pub mod exif;
pub mod hash;

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use rayon::prelude::*;
use serde::Serialize;
use tauri::{AppHandle, Emitter};
use walkdir::WalkDir;

use crate::db::{models::NewImage, queries};

const IMAGE_EXTENSIONS: &[&str] = &["jpg", "jpeg", "png", "webp", "tiff", "tif", "heic", "heif"];

#[derive(Debug, Serialize, Clone)]
pub struct ScanProgressEvent {
    pub phase: String,
    pub scanned: usize,
    pub total: usize,
    #[serde(rename = "currentPath")]
    pub current_path: String,
}

#[derive(Debug, Serialize)]
pub struct ScanResult {
    pub images_found: usize,
    pub duration_ms: u128,
}

pub async fn scan_dir(
    app: AppHandle,
    pool: sqlx::SqlitePool,
    folder_path: String,
    recursive: bool,
    cancelled: Arc<AtomicBool>,
    processing_threads: u32,
) -> ScanResult {
    let start = Instant::now();

    emit_progress(&app, "walking", 0, 0, &folder_path);

    // Phase 1: collect paths (sync, fast)
    let paths: Vec<std::path::PathBuf> = {
        let walker = if recursive {
            WalkDir::new(&folder_path)
        } else {
            WalkDir::new(&folder_path).max_depth(1)
        };

        walker
            .into_iter()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_type().is_file())
            .filter(|e| {
                e.path()
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| IMAGE_EXTENSIONS.contains(&ext.to_lowercase().as_str()))
                    .unwrap_or(false)
            })
            .map(|e| e.path().to_path_buf())
            .collect()
    };

    let total = paths.len();
    emit_progress(&app, "hashing", 0, total, "");

    // Phase 2: process images in rayon, send DB writes through a channel
    // to avoid calling async code from rayon threads.
    //
    // Use the user-configured thread limit, or default to (num_cpus - 1).
    let num_threads = if processing_threads > 0 {
        processing_threads as usize
    } else {
        (num_cpus::get().saturating_sub(1)).max(1)
    };
    let pool_rayon = rayon::ThreadPoolBuilder::new()
        .num_threads(num_threads)
        .build()
        .unwrap_or_else(|_| rayon::ThreadPoolBuilder::new().build().unwrap());

    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel::<NewImage>();
    let counter = Arc::new(AtomicUsize::new(0));
    let last_emit = Arc::new(std::sync::Mutex::new(Instant::now()));

    let app_clone = app.clone();
    let counter_clone = counter.clone();
    let cancelled_clone = cancelled.clone();
    let tx_clone = tx.clone();

    // Spawn rayon work on a blocking thread so it doesn't block the tokio executor
    let rayon_task = tokio::task::spawn_blocking(move || {
        pool_rayon.install(|| paths.par_iter().for_each(|path| {
            if cancelled_clone.load(Ordering::Relaxed) {
                return;
            }

            let path_str = path.to_str().unwrap_or("").to_string();
            let file_name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            let file_size = match std::fs::metadata(path) {
                Ok(m) => m.len() as i64,
                Err(_) => return,
            };

            let exif_data = exif::extract(path);

            // Decode image for dimensions + perceptual hash.
            // For JPEGs, use zune-jpeg's 1/8-scale partial decode —
            // the DCT downscales in-hardware, so we decode ~64× fewer
            // pixels while still getting a hash-quality thumbnail.
            // Non-JPEG files fall back to the full image crate decode.
            let is_jpeg = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| matches!(e.to_lowercase().as_str(), "jpg" | "jpeg"))
                .unwrap_or(false);

            let (width_px, height_px, phash) = if is_jpeg {
                hash::decode_jpeg_fast(path)
            } else {
                match image::open(path) {
                    Ok(img) => {
                        let w = img.width() as i64;
                        let h = img.height() as i64;
                        let small = img.thumbnail(64, 64);
                        let hash = hash::compute_from_image(&small);
                        (Some(w), Some(h), hash)
                    }
                    Err(_) => (exif_data.width_px, exif_data.height_px, None),
                }
            };

            let format = path
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase());

            let new_image = NewImage {
                file_path: path_str.clone(),
                file_name,
                file_size_bytes: file_size,
                width_px,
                height_px,
                format,
                taken_at: exif_data.taken_at,
                taken_at_source: exif_data.taken_at_source,
                camera_make: exif_data.camera_make,
                camera_model: exif_data.camera_model,
                perceptual_hash: phash,
            };

            // Send to async DB writer — non-blocking send
            let _ = tx_clone.send(new_image);

            let done = counter_clone.fetch_add(1, Ordering::Relaxed) + 1;

            let should_emit = done % 100 == 0 || {
                let mut last = last_emit.lock().unwrap();
                if last.elapsed() >= Duration::from_millis(100) {
                    *last = Instant::now();
                    true
                } else {
                    false
                }
            };

            if should_emit {
                emit_progress(&app_clone, "hashing", done, total, &path_str);
            }
        })); // end pool_rayon.install
        // Drop tx so the receiver loop below terminates
        drop(tx_clone);
    });

    // Drop our own tx so the channel closes when rayon is done
    drop(tx);

    // Phase 3: consume DB writes on the async side
    while let Some(new_image) = rx.recv().await {
        queries::upsert_image(&pool, &new_image).await.ok();
    }

    // Wait for rayon to fully finish
    let _ = rayon_task.await;

    let images_found = counter.load(Ordering::Relaxed);
    emit_progress(&app, "done", images_found, total, "");

    ScanResult {
        images_found,
        duration_ms: start.elapsed().as_millis(),
    }
}

fn emit_progress(app: &AppHandle, phase: &str, scanned: usize, total: usize, current_path: &str) {
    let _ = app.emit(
        "scan:progress",
        ScanProgressEvent {
            phase: phase.to_string(),
            scanned,
            total,
            current_path: current_path.to_string(),
        },
    );
}
