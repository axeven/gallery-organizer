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
) -> ScanResult {
    let start = Instant::now();

    // Phase 1: walk directory
    emit_progress(&app, "walking", 0, 0, &folder_path);

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

    let counter = Arc::new(AtomicUsize::new(0));
    let last_emit = Arc::new(std::sync::Mutex::new(Instant::now()));

    paths.par_iter().for_each(|path| {
        if cancelled.load(Ordering::Relaxed) {
            return;
        }

        let path_str = path.to_str().unwrap_or("").to_string();
        let file_name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("")
            .to_string();

        // Get file metadata
        let file_size = match std::fs::metadata(path) {
            Ok(m) => m.len() as i64,
            Err(_) => return,
        };

        // Extract EXIF
        let exif_data = exif::extract(path);

        // Compute perceptual hash
        let phash = hash::compute(path);

        // Get image dimensions (from image crate if EXIF didn't provide them)
        let (width_px, height_px) = if exif_data.width_px.is_some() && exif_data.height_px.is_some() {
            (exif_data.width_px, exif_data.height_px)
        } else {
            match image::image_dimensions(path) {
                Ok((w, h)) => (Some(w as i64), Some(h as i64)),
                Err(_) => (None, None),
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

        // Write to DB — block_in_place lets us call async from rayon thread
        let pool_clone = pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(queries::upsert_image(&pool_clone, &new_image))
                .ok();
        });

        let done = counter.fetch_add(1, Ordering::Relaxed) + 1;

        // Throttle events: emit every 100 files or every 50ms
        let should_emit = done % 100 == 0 || {
            let mut last = last_emit.lock().unwrap();
            if last.elapsed() >= Duration::from_millis(50) {
                *last = Instant::now();
                true
            } else {
                false
            }
        };

        if should_emit {
            emit_progress(&app, "hashing", done, total, &path_str);
        }
    });

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
