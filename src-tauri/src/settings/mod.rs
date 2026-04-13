use anyhow::Result;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::db::queries;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
    pub output_mode: String,
    pub output_dir: Option<String>,
    pub default_quality: u8,
    pub default_format: String,
    pub duplicate_hash_distance: u32,
    pub date_group_granularity: String,
    pub thumbnail_size_px: u32,
    pub scan_recursive: bool,
    /// Max rayon threads for image processing jobs. 0 = auto (num_cpus - 1).
    pub processing_threads: u32,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            output_mode: "output_folder".into(),
            output_dir: None,
            default_quality: 85,
            default_format: "jpeg".into(),
            duplicate_hash_distance: 8,
            date_group_granularity: "day".into(),
            thumbnail_size_px: 256,
            scan_recursive: true,
            processing_threads: 0,
        }
    }
}

pub async fn load_all(pool: &SqlitePool) -> Result<AppSettings> {
    let output_mode = queries::get_setting(pool, "output_mode").await?
        .and_then(|v| serde_json::from_str::<String>(&v).ok())
        .unwrap_or_else(|| "output_folder".into());

    let output_dir = queries::get_setting(pool, "output_dir").await?
        .and_then(|v| serde_json::from_str::<Option<String>>(&v).ok())
        .flatten();

    let default_quality = queries::get_setting(pool, "default_quality").await?
        .and_then(|v| serde_json::from_str::<u8>(&v).ok())
        .unwrap_or(85);

    let default_format = queries::get_setting(pool, "default_format").await?
        .and_then(|v| serde_json::from_str::<String>(&v).ok())
        .unwrap_or_else(|| "jpeg".into());

    let duplicate_hash_distance = queries::get_setting(pool, "duplicate_hash_distance").await?
        .and_then(|v| serde_json::from_str::<u32>(&v).ok())
        .unwrap_or(8);

    let date_group_granularity = queries::get_setting(pool, "date_group_granularity").await?
        .and_then(|v| serde_json::from_str::<String>(&v).ok())
        .unwrap_or_else(|| "day".into());

    let thumbnail_size_px = queries::get_setting(pool, "thumbnail_size_px").await?
        .and_then(|v| serde_json::from_str::<u32>(&v).ok())
        .unwrap_or(256);

    let scan_recursive = queries::get_setting(pool, "scan_recursive").await?
        .and_then(|v| serde_json::from_str::<bool>(&v).ok())
        .unwrap_or(true);

    let processing_threads = queries::get_setting(pool, "processing_threads").await?
        .and_then(|v| serde_json::from_str::<u32>(&v).ok())
        .unwrap_or(0);

    Ok(AppSettings {
        output_mode,
        output_dir,
        default_quality,
        default_format,
        duplicate_hash_distance,
        date_group_granularity,
        thumbnail_size_px,
        scan_recursive,
        processing_threads,
    })
}

pub async fn save(pool: &SqlitePool, s: &AppSettings) -> Result<()> {
    queries::set_setting(pool, "output_mode", &serde_json::to_string(&s.output_mode)?).await?;
    queries::set_setting(pool, "output_dir", &serde_json::to_string(&s.output_dir)?).await?;
    queries::set_setting(pool, "default_quality", &serde_json::to_string(&s.default_quality)?).await?;
    queries::set_setting(pool, "default_format", &serde_json::to_string(&s.default_format)?).await?;
    queries::set_setting(pool, "duplicate_hash_distance", &serde_json::to_string(&s.duplicate_hash_distance)?).await?;
    queries::set_setting(pool, "date_group_granularity", &serde_json::to_string(&s.date_group_granularity)?).await?;
    queries::set_setting(pool, "thumbnail_size_px", &serde_json::to_string(&s.thumbnail_size_px)?).await?;
    queries::set_setting(pool, "scan_recursive", &serde_json::to_string(&s.scan_recursive)?).await?;
    queries::set_setting(pool, "processing_threads", &serde_json::to_string(&s.processing_threads)?).await?;
    Ok(())
}
