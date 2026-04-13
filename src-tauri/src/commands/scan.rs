use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use serde::Serialize;
use tauri::{AppHandle, State};

use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScanStatusResponse {
    pub is_scanning: bool,
}

#[tauri::command]
pub async fn scan_folder(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    folder_path: String,
    recursive: bool,
) -> Result<(), String> {
    let path = std::path::Path::new(&folder_path);
    if !path.exists() || !path.is_dir() {
        return Err(format!("Path does not exist or is not a directory: {folder_path}"));
    }

    // Cancel any existing scan
    {
        let mut handle = state.scan_handle.lock().unwrap();
        if let Some(h) = handle.take() {
            h.abort();
        }
    }

    let pool = state.db.clone();
    let cancelled = Arc::new(AtomicBool::new(false));
    let cancelled_clone = cancelled.clone();
    let processing_threads = state.settings.read().unwrap().processing_threads;

    let handle = tokio::spawn(async move {
        crate::scanner::scan_dir(app, pool, folder_path, recursive, cancelled_clone, processing_threads).await;
    });

    {
        let mut h = state.scan_handle.lock().unwrap();
        *h = Some(handle);
    }

    Ok(())
}

#[tauri::command]
pub async fn get_scan_status(state: State<'_, Arc<AppState>>) -> Result<ScanStatusResponse, String> {
    let is_scanning = state
        .scan_handle
        .lock()
        .unwrap()
        .as_ref()
        .map(|h| !h.is_finished())
        .unwrap_or(false);

    Ok(ScanStatusResponse { is_scanning })
}

#[tauri::command]
pub async fn cancel_scan(state: State<'_, Arc<AppState>>) -> Result<(), String> {
    let mut handle = state.scan_handle.lock().unwrap();
    if let Some(h) = handle.take() {
        h.abort();
    }
    Ok(())
}
