use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::SqlitePool;
use std::collections::HashMap;

use crate::db::queries;

pub async fn rebuild(pool: &SqlitePool) -> Result<usize> {
    // Delete all date groups (all three granularities)
    queries::delete_groups_by_type(pool, "date_%").await?;

    let images = queries::get_all_images_with_date(pool).await?;

    let mut day_buckets: HashMap<String, Vec<i64>> = HashMap::new();
    let mut month_buckets: HashMap<String, Vec<i64>> = HashMap::new();
    let mut year_buckets: HashMap<String, Vec<i64>> = HashMap::new();

    for img in &images {
        if let Some(ts) = img.taken_at {
            let dt = DateTime::<Utc>::from_timestamp(ts, 0)
                .unwrap_or_default()
                .naive_utc();

            day_buckets
                .entry(dt.format("%Y-%m-%d").to_string())
                .or_default()
                .push(img.id);
            month_buckets
                .entry(dt.format("%Y-%m").to_string())
                .or_default()
                .push(img.id);
            year_buckets
                .entry(dt.format("%Y").to_string())
                .or_default()
                .push(img.id);
        }
    }

    let mut count = 0;

    for (label, image_ids) in day_buckets {
        let group_id = queries::insert_group(pool, "date_day", &label).await?;
        queries::bulk_insert_group_members(pool, group_id, &image_ids).await?;
        count += 1;
    }
    for (label, image_ids) in month_buckets {
        let group_id = queries::insert_group(pool, "date_month", &label).await?;
        queries::bulk_insert_group_members(pool, group_id, &image_ids).await?;
        count += 1;
    }
    for (label, image_ids) in year_buckets {
        let group_id = queries::insert_group(pool, "date_year", &label).await?;
        queries::bulk_insert_group_members(pool, group_id, &image_ids).await?;
        count += 1;
    }

    Ok(count)
}
