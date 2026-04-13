use std::sync::Arc;

use serde::Serialize;
use tauri::State;

use crate::db::queries;
use crate::AppState;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PaginatedImages {
    pub items: Vec<crate::db::models::DbImage>,
    pub total: i64,
}

#[tauri::command]
pub async fn get_images(
    state: State<'_, Arc<AppState>>,
    group_id: Option<i64>,
    page: Option<i64>,
    page_size: Option<i64>,
    sort_by: Option<String>,
    sort_dir: Option<String>,
) -> Result<PaginatedImages, String> {
    let page = page.unwrap_or(0);
    let page_size = page_size.unwrap_or(50).min(200);
    let sort_by = sort_by.as_deref().unwrap_or("taken_at");
    let sort_dir = sort_dir.as_deref().unwrap_or("asc");

    // Validate sort params to prevent SQL injection
    let sort_by = match sort_by {
        "file_name" | "file_size" | "taken_at" | "perceptual_hash" => sort_by,
        _ => "taken_at",
    };
    let sort_dir = match sort_dir {
        "asc" | "desc" => sort_dir,
        _ => "asc",
    };

    let (items, total) =
        queries::get_images_paginated(&state.db, group_id, page, page_size, sort_by, sort_dir)
            .await
            .map_err(|e| e.to_string())?;

    Ok(PaginatedImages { items, total })
}

#[tauri::command]
pub async fn trash_image(
    state: State<'_, Arc<AppState>>,
    image_id: i64,
) -> Result<(), String> {
    let img = queries::get_image_by_id(&state.db, image_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("image {image_id} not found"))?;

    trash::delete(&img.file_path).map_err(|e| e.to_string())?;

    // Remove from DB and all group memberships (cascade handles group_members)
    queries::delete_image(&state.db, image_id)
        .await
        .map_err(|e| e.to_string())?;

    Ok(())
}

#[tauri::command]
pub async fn get_full_image(
    state: State<'_, Arc<AppState>>,
    image_id: i64,
) -> Result<(String, String), String> {
    // Returns (base64_data, mime_type)
    let img_record = queries::get_image_by_id(&state.db, image_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("image {image_id} not found"))?;

    let data = std::fs::read(&img_record.file_path).map_err(|e| e.to_string())?;

    let mime = match img_record.format.as_deref() {
        Some("jpg") | Some("jpeg") => "image/jpeg",
        Some("png") => "image/png",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        Some("tiff") | Some("tif") => "image/tiff",
        Some("heic") | Some("heif") => "image/heic",
        _ => "image/jpeg",
    };

    let b64 = base64::Engine::encode(&base64::engine::general_purpose::STANDARD, &data);
    Ok((b64, mime.to_string()))
}

#[tauri::command]
pub async fn get_image_detail(
    state: State<'_, Arc<AppState>>,
    image_id: i64,
) -> Result<Option<crate::db::models::DbImage>, String> {
    queries::get_image_by_id(&state.db, image_id)
        .await
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn get_thumbnail(
    app: tauri::AppHandle,
    state: State<'_, Arc<AppState>>,
    image_id: i64,
) -> Result<String, String> {
    let thumb_dir = tauri::Manager::path(&app)
        .app_data_dir()
        .map_err(|e| e.to_string())?
        .join("thumbnails");

    let thumb_path = thumb_dir.join(format!("{image_id}.jpg"));

    // Return cached thumbnail if it exists
    if thumb_path.exists() {
        let data = std::fs::read(&thumb_path).map_err(|e| e.to_string())?;
        return Ok(base64::Engine::encode(
            &base64::engine::general_purpose::STANDARD,
            &data,
        ));
    }

    // Generate thumbnail
    let img_record = queries::get_image_by_id(&state.db, image_id)
        .await
        .map_err(|e| e.to_string())?
        .ok_or_else(|| format!("image {image_id} not found"))?;

    let settings = state.settings.read().unwrap().clone();
    let thumb_size = settings.thumbnail_size_px;

    let path = std::path::Path::new(&img_record.file_path);
    let img = image::open(path).map_err(|e| e.to_string())?;
    let thumb = img.thumbnail(thumb_size, thumb_size);

    let mut buf = std::io::Cursor::new(Vec::new());
    thumb
        .write_to(&mut buf, image::ImageFormat::Jpeg)
        .map_err(|e| e.to_string())?;

    let data = buf.into_inner();
    std::fs::write(&thumb_path, &data).map_err(|e| e.to_string())?;

    Ok(base64::Engine::encode(
        &base64::engine::general_purpose::STANDARD,
        &data,
    ))
}
