pub mod date_grouper;
pub mod dupe_grouper;

use anyhow::Result;
use sqlx::SqlitePool;

pub async fn rebuild_groups(
    pool: &SqlitePool,
    group_type: &str,
    dupe_threshold: u32,
) -> Result<usize> {
    let mut total = 0;

    match group_type {
        "date" => {
            total += date_grouper::rebuild(pool).await?;
        }
        "duplicates" => {
            total += dupe_grouper::rebuild(pool, dupe_threshold).await?;
        }
        "all" => {
            total += date_grouper::rebuild(pool).await?;
            total += dupe_grouper::rebuild(pool, dupe_threshold).await?;
        }
        _ => {}
    }

    Ok(total)
}
