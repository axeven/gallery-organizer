use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use rayon::prelude::*;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter};

use crate::db::queries;
use crate::processor::{compress, output};

/// Returns true if `a` and `b` refer to the same underlying file (same
/// device + inode on Unix). Used to avoid trashing a file whose extension
/// changed only in case (e.g. .JPG → .jpg) on a case-insensitive FS like
/// NTFS, where both paths resolve to the same inode.
fn same_inode(a: &Path, b: &Path) -> bool {
    use std::os::unix::fs::MetadataExt;
    match (std::fs::metadata(a), std::fs::metadata(b)) {
        (Ok(ma), Ok(mb)) => ma.dev() == mb.dev() && ma.ino() == mb.ino(),
        _ => false,
    }
}

/// Params for the unified "process" job (replaces both "organize" and "compress").
/// All fields optional — omitting resize/format/quality = copy-only.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessJobParams {
    /// Sub-folder name to create inside output_dir (typically the group label).
    pub group_label: String,
    /// If true, delete source file after successful write.
    pub move_files: bool,
    /// Resize specification. Defaults to None (no resize).
    #[serde(default)]
    pub resize: compress::ResizeMode,
    /// Target format: "jpeg" or "webp". Defaults to "jpeg".
    pub target_format: Option<String>,
    /// Encode quality 1–100. Defaults to 85.
    pub quality: Option<u8>,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct JobProgressEvent {
    pub job_id: i64,
    pub processed: i64,
    pub total: i64,
    pub current_file: String,
    pub status: String,
}

pub async fn run_job(
    pool: sqlx::SqlitePool,
    app: AppHandle,
    job_id: i64,
    cancelled: Arc<AtomicBool>,
    // Max rayon threads. 0 = auto (num_cpus - 1).
    processing_threads: u32,
) -> Result<()> {
    let job = queries::get_job(&pool, job_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("job not found"))?;

    queries::update_job_status(&pool, job_id, "running").await?;

    let job_images = queries::get_job_images(&pool, job_id).await?;
    let total = job_images.len() as i64;

    let processed = Arc::new(std::sync::atomic::AtomicI64::new(0));
    let failed = Arc::new(std::sync::atomic::AtomicI64::new(0));

    // Collect (image_id, file_path) for all images in this job
    let mut image_entries: Vec<(i64, String)> = Vec::new();
    for ji in &job_images {
        if let Ok(Some(img)) = queries::get_image_by_id(&pool, ji.image_id).await {
            image_entries.push((img.id, img.file_path));
        }
    }

    // ── Unified "process" operation ──────────────────────────────────────────
    // Also handles the legacy "organize" name so old DB jobs still work.
    if job.operation == "process" || job.operation == "organize" {
        let params: ProcessJobParams = serde_json::from_str(&job.params_json)?;
        let in_place = job.output_mode == "in_place";
        log::info!(
            "run_job {job_id}: output_mode={:?} in_place={in_place} resize={:?} target_format={:?} quality={:?}",
            job.output_mode, params.resize, params.target_format, params.quality
        );

        // For folder mode, create the destination directory up front.
        let out_dir: Option<PathBuf> = if !in_place {
            // Sanitize the label for use as a directory name: replace characters
            // that are invalid on Windows/NTFS (<>:"/\|?*) with underscores,
            // and trim leading/trailing dots and spaces.
            let safe_label: String = params
                .group_label
                .chars()
                .map(|c| match c {
                    '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
                    c => c,
                })
                .collect::<String>()
                .trim_matches(|c: char| c == '.' || c == ' ')
                .to_string();
            let folder_name = if safe_label.is_empty() { "output".to_string() } else { safe_label };
            let d = PathBuf::from(job.output_dir.as_deref().unwrap_or("."))
                .join(&folder_name);
            if let Err(e) = std::fs::create_dir_all(&d) {
                queries::update_job_status(&pool, job_id, "failed").await?;
                return Err(anyhow::anyhow!("failed to create output dir: {e}"));
            }
            Some(d)
        } else {
            None
        };

        // Build compress params once (shared across all images)
        let compress_params = compress::ProcessParams {
            quality: params.quality,
            width: None,
            height: None,
            fit: None,
            target_format: params.target_format.clone(),
            resize: params.resize.clone(),
        };

        let do_reencode = params.resize != compress::ResizeMode::None
            || params.target_format.is_some()
            || params.quality.is_some();

        // Capture the tokio handle before entering rayon. Rayon threads are not
        // tokio threads, so Handle::current() inside par_iter would panic if the
        // parent task was aborted (cancel_job). Capturing it here guarantees the
        // handle stays valid for the lifetime of the rayon work even after abort.
        let rt = tokio::runtime::Handle::current();

        // Use the user-configured thread limit, or default to (num_cpus - 1).
        let num_threads = if processing_threads > 0 {
            processing_threads as usize
        } else {
            (num_cpus::get().saturating_sub(1)).max(1)
        };
        let rayon_pool = rayon::ThreadPoolBuilder::new()
            .num_threads(num_threads)
            .build()
            .unwrap_or_else(|_| rayon::ThreadPoolBuilder::new().build().unwrap());

        rayon_pool.install(|| image_entries.par_iter().for_each(|(image_id, file_path)| {
            if cancelled.load(Ordering::Relaxed) {
                return;
            }

            let src = PathBuf::from(file_path);

            // Warn early if the path is suspiciously long — helps diagnose
            // "file name too long" errors on NTFS/WSL2 paths.
            if file_path.len() > 240 {
                log::warn!("long path ({} chars): {file_path}", file_path.len());
            }

            // On success: (status, error, new_path, Option<(size, w, h, fmt)> for metadata update)
            type MetaUpdate = Option<(i64, u32, u32, String)>;
            // Derive the source extension (lowercase, no dot) for use as the
            // default output format — avoids silently re-encoding e.g. PNG→JPEG.
            let src_ext = src
                .extension()
                .and_then(|e| e.to_str())
                .map(|e| e.to_lowercase())
                .unwrap_or_else(|| "jpg".to_string());

            let (status, error, new_path, meta): (&str, Option<String>, Option<String>, MetaUpdate) = if in_place {
                // ── Overwrite in place ──────────────────────────────────────
                let fmt = compress_params.target_format.as_deref().unwrap_or(&src_ext);
                match compress::process_image(&src, &compress_params, fmt) {
                    Ok((data, ext, out_w, out_h)) => {
                        let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
                        let parent = src.parent().unwrap_or(std::path::Path::new("."));
                        let dst = parent.join(format!("{stem}.{ext}"));
                        let size = data.len() as i64;
                        // Write to a temp file in the system temp dir (avoids
                        // cross-device rename issues on WSL2 NTFS mounts), then
                        // copy over the destination and remove the temp file.
                        let result = (|| -> std::io::Result<()> {
                            let tmp = tempfile::Builder::new()
                                .prefix(".gallery-tmp-")
                                .suffix(&format!(".{ext}"))
                                .tempfile()?;
                            let tmp_path = tmp.path().to_path_buf();
                            std::fs::write(&tmp_path, &data)?;
                            // Try atomic rename first; fall back to copy+delete if
                            // src and tmp are on different filesystems (WSL2 NTFS).
                            if std::fs::rename(&tmp_path, &dst).is_err() {
                                std::fs::copy(&tmp_path, &dst)?;
                                // tmp dropped here cleans up the temp file
                            } else {
                                std::mem::forget(tmp);
                            }
                            Ok(())
                        })();
                        match result {
                            Ok(_) => {
                                let new = dst.to_string_lossy().into_owned();
                                // If the extension changed (e.g. .JPG → .jpg), we may
                                // have written a new file alongside the original.
                                // Guard: only trash src when dst and src are provably
                                // different files (different inodes or different canonical
                                // paths). On NTFS mounts, .JPG and .jpg resolve to the
                                // same inode so we must not trash in that case.
                                if dst != src {
                                    let same_file = same_inode(&src, &dst);
                                    if !same_file {
                                        let _ = trash::delete(&src);
                                    }
                                }
                                ("done", None, Some(new), Some((size, out_w, out_h, ext)))
                            }
                            Err(e) => {
                                log::error!("in-place write failed for {src:?}: {e}");
                                ("failed", Some(e.to_string()), None, None)
                            }
                        }
                    }
                    Err(e) => ("failed", Some(e.to_string()), None, None),
                }
            } else if do_reencode {
                // ── Re-encode to output folder ──────────────────────────────
                let dir = out_dir.as_ref().unwrap();
                let fmt = compress_params.target_format.as_deref().unwrap_or(&src_ext);
                match compress::process_image(&src, &compress_params, fmt) {
                    Ok((data, ext, out_w, out_h)) => {
                        let stem = src.file_stem().and_then(|s| s.to_str()).unwrap_or("image");
                        let dst = dir.join(format!("{stem}.{ext}"));
                        let size = data.len() as i64;
                        match std::fs::write(&dst, &data) {
                            Ok(_) => {
                                if params.move_files {
                                    let _ = trash::delete(&src);
                                    let new = dst.to_string_lossy().into_owned();
                                    // Moved + re-encoded: update path and metadata
                                    ("done", None, Some(new), Some((size, out_w, out_h, ext)))
                                } else {
                                    // Copied + re-encoded: original unchanged, don't touch its record
                                    ("done", None, None, None)
                                }
                            }
                            Err(e) => ("failed", Some(e.to_string()), None, None),
                        }
                    }
                    Err(e) => ("failed", Some(e.to_string()), None, None),
                }
            } else {
                // ── Copy-only to output folder (no re-encode) ───────────────
                let dir = out_dir.as_ref().unwrap();
                let file_name = src.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                let dst = dir.join(&file_name);
                if params.move_files {
                    let result = std::fs::rename(&src, &dst).or_else(|_| {
                        std::fs::copy(&src, &dst).map(|_| ())?;
                        trash::delete(&src).map_err(|e| std::io::Error::other(e.to_string()))
                    });
                    match result {
                        // Only path changes, file content identical
                        Ok(_) => ("done", None, Some(dst.to_string_lossy().into_owned()), None),
                        Err(e) => ("failed", Some(e.to_string()), None, None),
                    }
                } else {
                    match std::fs::copy(&src, &dst) {
                        Ok(_) => ("done", None, None, None),
                        Err(e) => ("failed", Some(e.to_string()), None, None),
                    }
                }
            };

            let pool_clone = pool.clone();
            let jid = job_id;
            let iid = *image_id;
            let err_ref = error.as_deref();
            rt.block_on(queries::update_job_image_status(
                &pool_clone, jid, iid, status, err_ref,
            ))
            .ok();
            if let (Some(ref np), Some((size, w, h, ref fmt))) = (new_path.as_ref(), meta.as_ref()) {
                rt.block_on(queries::update_image_after_process(
                    &pool_clone, iid, np, *size, *w, *h, fmt,
                ))
                .ok();
            } else if let Some(ref np) = new_path {
                rt.block_on(queries::update_image_path(&pool_clone, iid, np))
                    .ok();
            }

            if status == "done" {
                processed.fetch_add(1, Ordering::Relaxed);
            } else {
                failed.fetch_add(1, Ordering::Relaxed);
            }

            let done = processed.load(Ordering::Relaxed);
            let fail = failed.load(Ordering::Relaxed);
            let pool_clone = pool.clone();
            rt.block_on(queries::update_job_progress(&pool_clone, jid, done, fail))
                .ok();

            let _ = app.emit(
                "job:progress",
                JobProgressEvent {
                    job_id: jid,
                    processed: done + fail,
                    total,
                    current_file: file_path.clone(),
                    status: status.to_string(),
                },
            );
        }));
    } else {
        // ── Legacy "compress" operation (kept for old jobs in DB) ────────────
        let params: compress::ProcessParams = serde_json::from_str(&job.params_json)?;

        let output_mode = match job.output_mode.as_str() {
            "in_place" => output::OutputMode::InPlace,
            _ => {
                let dir = PathBuf::from(job.output_dir.as_deref().unwrap_or("."));
                output::OutputMode::OutputFolder {
                    dir,
                    scan_root: PathBuf::from("/"),
                }
            }
        };

        let rt = tokio::runtime::Handle::current();

        image_entries.par_iter().for_each(|(image_id, file_path)| {
            if cancelled.load(Ordering::Relaxed) {
                return;
            }

            let path = PathBuf::from(file_path);
            let format = params.target_format.as_deref().unwrap_or("jpeg");
            let result = compress::process_image(&path, &params, format);

            let (status, error) = match result {
                Ok((data, ext, _w, _h)) => match output::write(&path, &data, &ext, &output_mode) {
                    Ok(_) => ("done", None),
                    Err(e) => ("failed", Some(e.to_string())),
                },
                Err(e) => ("failed", Some(e.to_string())),
            };

            let pool_clone = pool.clone();
            let jid = job_id;
            let iid = *image_id;
            let err_ref = error.as_deref();
            rt.block_on(queries::update_job_image_status(
                &pool_clone, jid, iid, status, err_ref,
            ))
            .ok();

            if status == "done" {
                processed.fetch_add(1, Ordering::Relaxed);
            } else {
                failed.fetch_add(1, Ordering::Relaxed);
            }

            let done = processed.load(Ordering::Relaxed);
            let fail = failed.load(Ordering::Relaxed);

            let pool_clone = pool.clone();
            rt.block_on(queries::update_job_progress(&pool_clone, jid, done, fail))
                .ok();

            let _ = app.emit(
                "job:progress",
                JobProgressEvent {
                    job_id: jid,
                    processed: done + fail,
                    total,
                    current_file: file_path.clone(),
                    status: status.to_string(),
                },
            );
        });
    }

    let final_failed = failed.load(Ordering::Relaxed);
    let final_status = if cancelled.load(Ordering::Relaxed) {
        "cancelled"
    } else if final_failed > 0 && final_failed == total {
        "failed"
    } else {
        "done"
    };

    queries::update_job_status(&pool, job_id, final_status).await?;

    let event = if final_status == "done" {
        "job:complete"
    } else {
        "job:failed"
    };

    let _ = app.emit(
        event,
        serde_json::json!({ "job_id": job_id, "status": final_status }),
    );

    Ok(())
}
