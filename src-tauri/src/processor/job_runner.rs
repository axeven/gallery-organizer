use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use rayon::prelude::*;
use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::db::queries;
use crate::processor::{compress, output};

#[derive(Debug, Serialize, Clone)]
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
) -> Result<()> {
    let job = queries::get_job(&pool, job_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("job not found"))?;

    queries::update_job_status(&pool, job_id, "running").await?;

    let params: compress::ProcessParams = serde_json::from_str(&job.params_json)?;

    let job_images = queries::get_job_images(&pool, job_id).await?;
    let total = job_images.len() as i64;

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

    let processed = Arc::new(std::sync::atomic::AtomicI64::new(0));
    let failed = Arc::new(std::sync::atomic::AtomicI64::new(0));

    // Process images — get all image paths first
    let mut image_entries: Vec<(i64, String)> = Vec::new();
    for ji in &job_images {
        if let Ok(Some(img)) = queries::get_image_by_id(&pool, ji.image_id).await {
            image_entries.push((img.id, img.file_path));
        }
    }

    // Use rayon for parallel processing
    image_entries.par_iter().for_each(|(image_id, file_path)| {
        if cancelled.load(Ordering::Relaxed) {
            return;
        }

        let path = PathBuf::from(file_path);
        let result = compress::process_image(&path, &params, "jpeg");

        let (status, error) = match result {
            Ok((data, ext)) => {
                match output::write(&path, &data, &ext, &output_mode) {
                    Ok(_) => ("done", None),
                    Err(e) => ("failed", Some(e.to_string())),
                }
            }
            Err(e) => ("failed", Some(e.to_string())),
        };

        let pool_clone = pool.clone();
        let jid = job_id;
        let iid = *image_id;
        let err_ref = error.as_deref();

        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(queries::update_job_image_status(&pool_clone, jid, iid, status, err_ref))
                .ok();
        });

        if status == "done" {
            processed.fetch_add(1, Ordering::Relaxed);
        } else {
            failed.fetch_add(1, Ordering::Relaxed);
        }

        let done = processed.load(Ordering::Relaxed);
        let fail = failed.load(Ordering::Relaxed);

        let pool_clone = pool.clone();
        tokio::task::block_in_place(|| {
            tokio::runtime::Handle::current()
                .block_on(queries::update_job_progress(&pool_clone, jid, done, fail))
                .ok();
        });

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
