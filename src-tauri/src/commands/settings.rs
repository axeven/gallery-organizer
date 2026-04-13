use std::sync::Arc;

use tauri::State;

use crate::settings::{self, AppSettings};
use crate::AppState;

#[tauri::command]
pub async fn get_settings(state: State<'_, Arc<AppState>>) -> Result<AppSettings, String> {
    Ok(state.settings.read().unwrap().clone())
}

#[tauri::command]
pub async fn update_settings(
    state: State<'_, Arc<AppState>>,
    payload: serde_json::Value,
) -> Result<AppSettings, String> {
    let mut current = state.settings.write().unwrap().clone();

    // Merge only provided fields
    if let Some(v) = payload.get("outputMode").and_then(|v| v.as_str()) {
        current.output_mode = v.to_string();
    }
    if let Some(v) = payload.get("outputDir") {
        current.output_dir = v.as_str().map(|s| s.to_string());
    }
    if let Some(v) = payload.get("defaultQuality").and_then(|v| v.as_u64()) {
        current.default_quality = v.min(100) as u8;
    }
    if let Some(v) = payload.get("defaultFormat").and_then(|v| v.as_str()) {
        current.default_format = v.to_string();
    }
    if let Some(v) = payload.get("duplicateHashDistance").and_then(|v| v.as_u64()) {
        current.duplicate_hash_distance = v as u32;
    }
    if let Some(v) = payload.get("dateGroupGranularity").and_then(|v| v.as_str()) {
        current.date_group_granularity = v.to_string();
    }
    if let Some(v) = payload.get("thumbnailSizePx").and_then(|v| v.as_u64()) {
        current.thumbnail_size_px = v as u32;
    }
    if let Some(v) = payload.get("scanRecursive").and_then(|v| v.as_bool()) {
        current.scan_recursive = v;
    }
    if let Some(v) = payload.get("processingThreads").and_then(|v| v.as_u64()) {
        current.processing_threads = v as u32;
    }

    settings::save(&state.db, &current)
        .await
        .map_err(|e| e.to_string())?;

    *state.settings.write().unwrap() = current.clone();

    Ok(current)
}

#[tauri::command]
pub async fn open_folder_dialog(app: tauri::AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let path = app
        .dialog()
        .file()
        .blocking_pick_folder();

    Ok(path.map(|p| p.to_string()))
}

#[tauri::command]
pub async fn reveal_in_explorer(
    app: tauri::AppHandle,
    file_path: String,
) -> Result<(), String> {
    use tauri_plugin_shell::ShellExt;

    let path = std::path::Path::new(&file_path);
    let parent = path.parent().unwrap_or(path);

    #[cfg(target_os = "windows")]
    app.shell()
        .command("explorer")
        .args([parent.to_str().unwrap_or(".")])
        .spawn()
        .map_err(|e| e.to_string())?;

    #[cfg(target_os = "macos")]
    app.shell()
        .command("open")
        .args([parent.to_str().unwrap_or(".")])
        .spawn()
        .map_err(|e| e.to_string())?;

    #[cfg(target_os = "linux")]
    app.shell()
        .command("xdg-open")
        .args([parent.to_str().unwrap_or(".")])
        .spawn()
        .map_err(|e| e.to_string())?;

    Ok(())
}
