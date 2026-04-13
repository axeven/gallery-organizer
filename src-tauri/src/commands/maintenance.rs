use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::db::queries;
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CleanupResult {
    pub checked: usize,
    pub removed: usize,
    pub thumbnails_removed: usize,
    pub errors: Vec<String>,
}

#[tauri::command]
pub async fn cleanup_stale_images(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
) -> Result<CleanupResult, String> {
    let thumb_dir = tauri::Manager::path(&app)
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("thumbnails");

    let all = queries::get_all_image_paths(&state.db)
        .await
        .map_err(|e| e.to_string())?;

    let checked = all.len();
    let mut removed = 0;
    let mut thumbnails_removed = 0;
    let mut errors = Vec::new();

    for (id, file_path) in all {
        if std::path::Path::new(&file_path).exists() {
            continue;
        }

        // Delete from DB — cascades to group_members and job_images
        match queries::delete_image(&state.db, id).await {
            Ok(_) => {
                removed += 1;

                // Remove cached thumbnail if present
                let thumb_path = thumb_dir.join(format!("{id}.jpg"));
                if thumb_path.exists() {
                    match std::fs::remove_file(&thumb_path) {
                        Ok(_) => thumbnails_removed += 1,
                        Err(e) => errors.push(format!("thumbnail {id}: {e}")),
                    }
                }
            }
            Err(e) => errors.push(format!("image {id} ({file_path}): {e}")),
        }
    }

    Ok(CleanupResult {
        checked,
        removed,
        thumbnails_removed,
        errors,
    })
}
