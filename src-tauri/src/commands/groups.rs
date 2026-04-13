use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use serde::Serialize;
use tauri::{AppHandle, Emitter, State};

use crate::db::{models::DbImage, queries};
use crate::processor::job_runner;
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

    let mut cluster_map: std::collections::HashMap<i64, (Vec<i64>, Option<i64>)> =
        std::collections::HashMap::new();

    for (group_id, image_id, is_keeper, _resolution) in rows {
        let entry = cluster_map.entry(group_id).or_default();
        entry.0.push(image_id);
        if is_keeper == 1 {
            entry.1 = Some(image_id);
        }
    }

    let mut clusters = Vec::new();
    for (cluster_id, (image_ids, keeper_id)) in cluster_map {
        let mut images = Vec::new();
        for iid in image_ids {
            if let Ok(Some(img)) = queries::get_image_by_id(&state.db, iid).await {
                images.push(img);
            }
        }
        clusters.push(DuplicateCluster {
            cluster_id,
            images,
            suggested_keeper_id: keeper_id,
        });
    }

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

#[tauri::command]
pub async fn export_group(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    group_id: i64,
    output_dir: String,
    move_files: bool,
) -> Result<i64, String> {
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

    let params_json = serde_json::json!({
        "groupLabel": group_label,
        "moveFiles": move_files,
    })
    .to_string();

    let job_id = queries::create_job(
        &state.db,
        "organize",
        &params_json,
        "output_folder",
        Some(&output_dir),
        &image_ids,
    )
    .await
    .map_err(|e| e.to_string())?;

    // Start the job immediately
    let cancelled = Arc::new(AtomicBool::new(false));
    let pool = state.db.clone();
    let app_clone = app.clone();

    let handle = tokio::spawn(async move {
        job_runner::run_job(pool, app_clone, job_id, cancelled)
            .await
            .ok();
    });

    state.job_handles.lock().unwrap().insert(job_id, handle);

    Ok(job_id)
}

#[tauri::command]
pub async fn remove_group(
    state: State<'_, Arc<AppState>>,
    group_id: i64,
) -> Result<(), String> {
    queries::remove_group(&state.db, group_id)
        .await
        .map_err(|e| e.to_string())
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
