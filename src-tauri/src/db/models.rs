use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DbImage {
    pub id: i64,
    pub file_path: String,
    pub file_name: String,
    pub file_size_bytes: i64,
    pub width_px: Option<i64>,
    pub height_px: Option<i64>,
    pub format: Option<String>,
    pub taken_at: Option<i64>,
    pub taken_at_source: String,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub perceptual_hash: Option<String>,
    pub scanned_at: i64,
    pub updated_at: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NewImage {
    pub file_path: String,
    pub file_name: String,
    pub file_size_bytes: i64,
    pub width_px: Option<i64>,
    pub height_px: Option<i64>,
    pub format: Option<String>,
    pub taken_at: Option<i64>,
    pub taken_at_source: String,
    pub camera_make: Option<String>,
    pub camera_model: Option<String>,
    pub perceptual_hash: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DbGroup {
    pub id: i64,
    pub group_type: String,
    pub label: String,
    pub created_at: i64,
    pub image_count: Option<i64>,
    pub cover_image_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DbProcessingJob {
    pub id: i64,
    pub status: String,
    pub operation: String,
    pub params_json: String,
    pub output_mode: String,
    pub output_dir: Option<String>,
    pub total_images: i64,
    pub processed_count: i64,
    pub failed_count: i64,
    pub error_message: Option<String>,
    pub created_at: i64,
    pub started_at: Option<i64>,
    pub finished_at: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DbJobImage {
    pub job_id: i64,
    pub image_id: i64,
    pub status: String,
    pub error: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct DbGroupMember {
    pub group_id: i64,
    pub image_id: i64,
    pub is_keeper: i64,
}
