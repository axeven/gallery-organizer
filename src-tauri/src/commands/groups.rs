use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, State};

use crate::db::{models::DbImage, queries};
use crate::processor::{compress, job_runner};
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RebuildResult {
    pub groups_created: usize,
    pub duration_ms: u128,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DuplicateCluster {
    pub cluster_id: i64,
    pub images: Vec<DbImage>,
    pub suggested_keeper_id: Option<i64>,
}

#[tauri::command]
pub async fn get_groups(
    state: State<'_, Arc<AppState>>,
    group_type: Option<String>,
) -> Result<serde_json::Value, String> {
    let groups = queries::get_groups(&state.db, group_type.as_deref())
        .await
        .map_err(|e| e.to_string())?;
    Ok(serde_json::to_value(groups).unwrap())
}

#[tauri::command]
pub async fn rebuild_groups(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    group_type: String,
) -> Result<RebuildResult, String> {
    let start = std::time::Instant::now();
    let settings = state.settings.read().unwrap().clone();

    let groups_created = crate::grouper::rebuild_groups(
        &state.db,
        &group_type,
        settings.duplicate_hash_distance,
    )
    .await
    .map_err(|e| e.to_string())?;

    let _ = app.emit("groups:rebuilt", serde_json::json!({ "group_type": group_type }));

    Ok(RebuildResult {
        groups_created,
        duration_ms: start.elapsed().as_millis(),
    })
}

#[tauri::command]
pub async fn get_duplicate_clusters(
    state: State<'_, Arc<AppState>>,
) -> Result<Vec<DuplicateCluster>, String> {
    let rows = queries::get_duplicate_cluster_members(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    // Group rows by cluster in one pass — no additional DB queries needed.
    let mut cluster_map: std::collections::HashMap<i64, (Vec<DbImage>, Option<i64>)> =
        std::collections::HashMap::new();

    for row in rows {
        let entry = cluster_map.entry(row.group_id).or_default();
        if row.is_keeper == 1 {
            entry.1 = Some(row.image.id);
        }
        entry.0.push(row.image);
    }

    let clusters = cluster_map
        .into_iter()
        .map(|(cluster_id, (images, suggested_keeper_id))| DuplicateCluster {
            cluster_id,
            images,
            suggested_keeper_id,
        })
        .collect();

    Ok(clusters)
}

#[tauri::command]
pub async fn set_keeper(
    state: State<'_, Arc<AppState>>,
    group_id: i64,
    image_id: i64,
) -> Result<(), String> {
    queries::set_keeper(&state.db, group_id, image_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn dismiss_cluster(
    state: State<'_, Arc<AppState>>,
    group_id: i64,
) -> Result<(), String> {
    queries::dismiss_cluster(&state.db, group_id)
        .await
        .map_err(|e| e.to_string())
}

/// Per-image info returned by get_group_summary, used by the frontend
/// to estimate output sizes before starting a job.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageSummaryItem {
    pub image_id: i64,
    pub file_name: String,
    pub file_size_bytes: i64,
    pub width_px: Option<i64>,
    pub height_px: Option<i64>,
}

#[tauri::command]
pub async fn get_group_summary(
    state: State<'_, Arc<AppState>>,
    group_id: i64,
) -> Result<Vec<ImageSummaryItem>, String> {
    let image_paths = queries::get_group_image_paths(&state.db, group_id)
        .await
        .map_err(|e| e.to_string())?;

    let mut items = Vec::new();
    for (image_id, _) in image_paths {
        if let Ok(Some(img)) = queries::get_image_by_id(&state.db, image_id).await {
            items.push(ImageSummaryItem {
                image_id: img.id,
                file_name: img.file_name,
                file_size_bytes: img.file_size_bytes,
                width_px: img.width_px,
                height_px: img.height_px,
            });
        }
    }
    Ok(items)
}

/// Params sent from the frontend to configure per-group processing.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProcessGroupPayload {
    /// "folder" = write to output_dir/group_label/, "overwrite" = replace originals in place.
    pub output_mode: String,
    pub output_dir: Option<String>,
    pub move_files: bool,
    pub resize: compress::ResizeMode,
    pub target_format: Option<String>,
    pub quality: Option<u8>,
}

#[tauri::command]
pub async fn process_group(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    group_id: i64,
    payload: ProcessGroupPayload,
) -> Result<i64, String> {
    if payload.output_mode == "folder" && payload.output_dir.as_deref().unwrap_or("").is_empty() {
        return Err("output_dir is required for folder mode".into());
    }

    // Fetch group label and image IDs
    let groups = queries::get_groups(&state.db, None)
        .await
        .map_err(|e| e.to_string())?;

    let group = groups
        .iter()
        .find(|g| g.id == group_id)
        .ok_or_else(|| format!("group {group_id} not found"))?;

    let group_label = group.label.clone();

    let image_paths = queries::get_group_image_paths(&state.db, group_id)
        .await
        .map_err(|e| e.to_string())?;

    let image_ids: Vec<i64> = image_paths.iter().map(|(id, _)| *id).collect();

    if image_ids.is_empty() {
        return Err("group has no images".into());
    }

    let (db_output_mode, db_output_dir) = if payload.output_mode == "overwrite" {
        ("in_place", None)
    } else {
        ("output_folder", payload.output_dir.as_deref())
    };

    let params_json = serde_json::json!({
        "groupLabel": group_label,
        "moveFiles": payload.move_files,
        "resize": payload.resize,
        "targetFormat": payload.target_format,
        "quality": payload.quality,
    })
    .to_string();

    let job_id = queries::create_job(
        &state.db,
        "process",
        &params_json,
        db_output_mode,
        db_output_dir,
        &image_ids,
    )
    .await
    .map_err(|e| e.to_string())?;

    // Start the job immediately
    let cancelled = Arc::new(AtomicBool::new(false));
    let pool = state.db.clone();
    let app_clone = app.clone();
    let processing_threads = state.settings.read().unwrap().processing_threads;

    let handle = tokio::spawn(async move {
        job_runner::run_job(pool, app_clone, job_id, cancelled, processing_threads)
            .await
            .ok();
    });

    state.job_handles.lock().unwrap().insert(job_id, handle);

    Ok(job_id)
}

#[tauri::command]
pub async fn remove_group(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    group_id: i64,
) -> Result<(), String> {
    let image_ids = queries::remove_group(&state.db, group_id)
        .await
        .map_err(|e| e.to_string())?;

    // Remove cached thumbnails for all deleted images
    if let Ok(thumb_dir) = tauri::Manager::path(&app)
        .app_data_dir()
        .map(|d| d.join("thumbnails"))
    {
        for id in image_ids {
            let thumb = thumb_dir.join(format!("{id}.jpg"));
            let _ = std::fs::remove_file(thumb);
        }
    }

    Ok(())
}

#[tauri::command]
pub async fn remove_image_from_group(
    state: State<'_, Arc<AppState>>,
    group_id: i64,
    image_id: i64,
) -> Result<(), String> {
    queries::remove_image_from_group(&state.db, group_id, image_id)
        .await
        .map_err(|e| e.to_string())
}
