use std::sync::atomic::AtomicBool;
use std::sync::Arc;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, State};

use crate::db::queries;
use crate::processor::{compress::ProcessParams, job_runner};
use crate::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateJobPayload {
    pub image_ids: Vec<i64>,
    pub operation: String,
    pub params: ProcessParams,
    pub output_mode: String,
    pub output_dir: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateJobResponse {
    pub job_id: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedJobs {
    pub items: Vec<crate::db::models::DbProcessingJob>,
    pub total: i64,
}

#[tauri::command]
pub async fn create_job(
    state: State<'_, Arc<AppState>>,
    payload: CreateJobPayload,
) -> Result<CreateJobResponse, String> {
    let params_json =
        serde_json::to_string(&payload.params).map_err(|e| e.to_string())?;

    let job_id = queries::create_job(
        &state.db,
        &payload.operation,
        &params_json,
        &payload.output_mode,
        payload.output_dir.as_deref(),
        &payload.image_ids,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(CreateJobResponse { job_id })
}

#[tauri::command]
pub async fn start_job(
    app: AppHandle,
    state: State<'_, Arc<AppState>>,
    job_id: i64,
) -> Result<(), String> {
    let cancelled = Arc::new(AtomicBool::new(false));
    let pool = state.db.clone();
    let app_clone = app.clone();
    let processing_threads = state.settings.read().unwrap().processing_threads;

    let handle = tokio::spawn(async move {
        job_runner::run_job(pool, app_clone, job_id, cancelled, processing_threads)
            .await
            .ok();
    });

    state
        .job_handles
        .lock()
        .unwrap()
        .insert(job_id, handle);

    Ok(())
}

#[tauri::command]
pub async fn cancel_job(
    state: State<'_, Arc<AppState>>,
    job_id: i64,
) -> Result<(), String> {
    // Scope the lock so it's not held across the await
    let handle = state.job_handles.lock().unwrap().remove(&job_id);
    if let Some(h) = handle {
        h.abort();
        queries::update_job_status(&state.db, job_id, "cancelled")
            .await
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn delete_job(
    state: State<'_, Arc<AppState>>,
    job_id: i64,
) -> Result<(), String> {
    // If somehow still running, abort first
    if let Some(h) = state.job_handles.lock().unwrap().remove(&job_id) {
        h.abort();
    }
    queries::delete_job(&state.db, job_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_jobs(
    state: State<'_, Arc<AppState>>,
    status: Option<String>,
    page: Option<i64>,
    page_size: Option<i64>,
) -> Result<PaginatedJobs, String> {
    let page = page.unwrap_or(0);
    let page_size = page_size.unwrap_or(20).min(100);

    let (items, total) = queries::get_jobs_paginated(&state.db, status.as_deref(), page, page_size)
        .await
        .map_err(|e| e.to_string())?;

    Ok(PaginatedJobs { items, total })
}

#[tauri::command]
pub async fn get_job_detail(
    state: State<'_, Arc<AppState>>,
    job_id: i64,
) -> Result<serde_json::Value, String> {
    let job = queries::get_job(&state.db, job_id)
        .await
        .map_err(|e| e.to_string())?;

    let images = queries::get_job_images(&state.db, job_id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(serde_json::json!({ "job": job, "images": images }))
}

#[tauri::command]
pub async fn retry_failed_images(
    state: State<'_, Arc<AppState>>,
    job_id: i64,
) -> Result<CreateJobResponse, String> {
    let original = queries::get_job(&state.db, job_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("job {job_id} not found"))?;

    let failed_ids = queries::get_failed_job_image_ids(&state.db, job_id)
        .await
        .map_err(|e| e.to_string())?;

    if failed_ids.is_empty() {
        return Err("no failed images to retry".into());
    }

    let new_job_id = queries::create_job(
        &state.db,
        &original.operation,
        &original.params_json,
        &original.output_mode,
        original.output_dir.as_deref(),
        &failed_ids,
    )
    .await
    .map_err(|e| e.to_string())?;

    Ok(CreateJobResponse { job_id: new_job_id })
}
