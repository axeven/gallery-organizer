use anyhow::Result;
use sqlx::SqlitePool;

use crate::db::queries;

const MB: i64 = 1024 * 1024;

const BUCKETS: &[(&str, i64, i64)] = &[
    ("0–3 MB",   0,        3 * MB),
    ("3–6 MB",   3 * MB,   6 * MB),
    ("6–10 MB",  6 * MB,  10 * MB),
    ("10–20 MB", 10 * MB, 20 * MB),
    ("> 20 MB",  20 * MB,  i64::MAX),
];

pub async fn rebuild(pool: &SqlitePool) -> Result<usize> {
    queries::delete_groups_by_type(pool, "size").await?;

    let images = sqlx::query!("SELECT id, file_size_bytes FROM images")
        .fetch_all(pool)
        .await?;

    let mut count = 0;

    for (label, min, max) in BUCKETS {
        let ids: Vec<i64> = images
            .iter()
            .filter(|r| r.file_size_bytes >= *min && r.file_size_bytes < *max)
            .map(|r| r.id)
            .collect();

        if ids.is_empty() {
            continue;
        }

        let group_id = queries::insert_group(pool, "size", label).await?;
        queries::bulk_insert_group_members(pool, group_id, &ids).await?;
        count += 1;
    }

    Ok(count)
}
