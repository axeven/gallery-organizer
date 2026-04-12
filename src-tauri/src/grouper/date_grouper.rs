use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use std::collections::HashMap;

use crate::db::queries;

pub async fn rebuild(pool: &SqlitePool, granularity: &str) -> Result<usize> {
    let prefix = match granularity {
        "month" => "date_month",
        "year" => "date_year",
        _ => "date_day",
    };

    queries::delete_groups_by_type(pool, &format!("{prefix}%")).await?;

    let images = queries::get_all_images_with_date(pool).await?;

    let mut buckets: HashMap<String, Vec<i64>> = HashMap::new();

    for img in &images {
        if let Some(ts) = img.taken_at {
            let dt = DateTime::<Utc>::from_timestamp(ts, 0)
                .unwrap_or_default()
                .naive_utc();

            let label = match granularity {
                "month" => dt.format("%Y-%m").to_string(),
                "year" => dt.format("%Y").to_string(),
                _ => dt.format("%Y-%m-%d").to_string(),
            };

            buckets.entry(label).or_default().push(img.id);
        }
    }

    let count = buckets.len();

    for (label, image_ids) in buckets {
        let group_id = queries::insert_group(pool, prefix, &label).await?;
        queries::bulk_insert_group_members(pool, group_id, &image_ids).await?;
    }

    Ok(count)
}
