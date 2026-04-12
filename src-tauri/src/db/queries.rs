use anyhow::Result;
use sqlx::SqlitePool;

use super::models::*;

// ── Images ──────────────────────────────────────────────────────────────────

pub async fn upsert_image(pool: &SqlitePool, img: &NewImage) -> Result<i64> {
    let now = chrono::Utc::now().timestamp();
    let id = sqlx::query!(
        r#"
        INSERT INTO images
            (file_path, file_name, file_size_bytes, width_px, height_px, format,
             taken_at, taken_at_source, camera_make, camera_model, perceptual_hash,
             scanned_at, updated_at)
        VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
        ON CONFLICT(file_path) DO UPDATE SET
            file_name       = excluded.file_name,
            file_size_bytes = excluded.file_size_bytes,
            width_px        = excluded.width_px,
            height_px       = excluded.height_px,
            format          = excluded.format,
            taken_at        = excluded.taken_at,
            taken_at_source = excluded.taken_at_source,
            camera_make     = excluded.camera_make,
            camera_model    = excluded.camera_model,
            perceptual_hash = excluded.perceptual_hash,
            scanned_at      = excluded.scanned_at,
            updated_at      = excluded.updated_at
        "#,
        img.file_path,
        img.file_name,
        img.file_size_bytes,
        img.width_px,
        img.height_px,
        img.format,
        img.taken_at,
        img.taken_at_source,
        img.camera_make,
        img.camera_model,
        img.perceptual_hash,
        now,
        now,
    )
    .execute(pool)
    .await?
    .last_insert_rowid();

    Ok(id)
}

pub async fn get_images_paginated(
    pool: &SqlitePool,
    group_id: Option<i64>,
    page: i64,
    page_size: i64,
    sort_by: &str,
    sort_dir: &str,
) -> Result<(Vec<DbImage>, i64)> {
    let offset = page * page_size;

    // Build order clause safely (values validated by command layer)
    let order = match (sort_by, sort_dir) {
        ("file_name", "asc") => "file_name ASC",
        ("file_name", "desc") => "file_name DESC",
        ("file_size", "asc") => "file_size_bytes ASC",
        ("file_size", "desc") => "file_size_bytes DESC",
        ("taken_at", "desc") => "taken_at DESC",
        _ => "taken_at ASC",
    };

    let (images, total) = if let Some(gid) = group_id {
        let q = format!(
            "SELECT i.* FROM images i
             JOIN group_members gm ON gm.image_id = i.id
             WHERE gm.group_id = ?
             ORDER BY i.{order}
             LIMIT ? OFFSET ?"
        );
        let images: Vec<DbImage> = sqlx::query_as(&q)
            .bind(gid)
            .bind(page_size)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        let total: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM group_members WHERE group_id = ?",
            gid
        )
        .fetch_one(pool)
        .await?;

        (images, total)
    } else {
        let q = format!(
            "SELECT * FROM images ORDER BY {order} LIMIT ? OFFSET ?"
        );
        let images: Vec<DbImage> = sqlx::query_as(&q)
            .bind(page_size)
            .bind(offset)
            .fetch_all(pool)
            .await?;

        let total: i64 = sqlx::query_scalar!("SELECT COUNT(*) FROM images")
            .fetch_one(pool)
            .await?;

        (images, total)
    };

    Ok((images, total))
}

pub async fn get_image_by_id(pool: &SqlitePool, id: i64) -> Result<Option<DbImage>> {
    let img = sqlx::query_as!(DbImage, "SELECT * FROM images WHERE id = ?", id)
        .fetch_optional(pool)
        .await?;
    Ok(img)
}

pub async fn get_all_images_with_hash(pool: &SqlitePool) -> Result<Vec<DbImage>> {
    let images = sqlx::query_as!(
        DbImage,
        "SELECT * FROM images WHERE perceptual_hash IS NOT NULL"
    )
    .fetch_all(pool)
    .await?;
    Ok(images)
}

pub async fn get_all_images_with_date(pool: &SqlitePool) -> Result<Vec<DbImage>> {
    let images: Vec<DbImage> = sqlx::query_as(
        "SELECT * FROM images WHERE taken_at IS NOT NULL ORDER BY taken_at ASC",
    )
    .fetch_all(pool)
    .await?;
    Ok(images)
}

// ── Groups ───────────────────────────────────────────────────────────────────

pub async fn get_groups(pool: &SqlitePool, group_type: Option<&str>) -> Result<Vec<DbGroup>> {
    let groups: Vec<DbGroup> = if let Some(gt) = group_type {
        sqlx::query_as(
            r#"
            SELECT g.id, g.group_type, g.label, g.created_at,
                   COUNT(gm.image_id) AS image_count,
                   MIN(gm.image_id) AS cover_image_id
            FROM groups g
            LEFT JOIN group_members gm ON gm.group_id = g.id
            WHERE g.group_type LIKE ?
            GROUP BY g.id
            ORDER BY g.label ASC
            "#,
        )
        .bind(gt)
        .fetch_all(pool)
        .await?
    } else {
        sqlx::query_as(
            r#"
            SELECT g.id, g.group_type, g.label, g.created_at,
                   COUNT(gm.image_id) AS image_count,
                   MIN(gm.image_id) AS cover_image_id
            FROM groups g
            LEFT JOIN group_members gm ON gm.group_id = g.id
            GROUP BY g.id
            ORDER BY g.label ASC
            "#,
        )
        .fetch_all(pool)
        .await?
    };
    Ok(groups)
}

pub async fn delete_groups_by_type(pool: &SqlitePool, group_type: &str) -> Result<()> {
    sqlx::query!("DELETE FROM groups WHERE group_type LIKE ?", group_type)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn insert_group(pool: &SqlitePool, group_type: &str, label: &str) -> Result<i64> {
    let now = chrono::Utc::now().timestamp();
    let id = sqlx::query!(
        "INSERT INTO groups (group_type, label, created_at) VALUES (?, ?, ?)",
        group_type,
        label,
        now
    )
    .execute(pool)
    .await?
    .last_insert_rowid();
    Ok(id)
}

pub async fn bulk_insert_group_members(
    pool: &SqlitePool,
    group_id: i64,
    image_ids: &[i64],
) -> Result<()> {
    for &image_id in image_ids {
        sqlx::query!(
            "INSERT OR IGNORE INTO group_members (group_id, image_id) VALUES (?, ?)",
            group_id,
            image_id
        )
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn set_keeper(pool: &SqlitePool, group_id: i64, image_id: i64) -> Result<()> {
    sqlx::query!(
        "UPDATE group_members SET is_keeper = 0 WHERE group_id = ?",
        group_id
    )
    .execute(pool)
    .await?;
    sqlx::query!(
        "UPDATE group_members SET is_keeper = 1 WHERE group_id = ? AND image_id = ?",
        group_id,
        image_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn dismiss_cluster(pool: &SqlitePool, group_id: i64) -> Result<()> {
    sqlx::query!("DELETE FROM groups WHERE id = ?", group_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn get_duplicate_cluster_members(
    pool: &SqlitePool,
) -> Result<Vec<(i64, i64, i64, i64)>> {
    // Returns (group_id, image_id, is_keeper, width_px * height_px as resolution)
    let rows = sqlx::query!(
        r#"
        SELECT gm.group_id, gm.image_id, gm.is_keeper,
               COALESCE(i.width_px * i.height_px, 0) AS resolution
        FROM group_members gm
        JOIN groups g ON g.id = gm.group_id
        JOIN images i ON i.id = gm.image_id
        WHERE g.group_type = 'duplicate_cluster'
        ORDER BY gm.group_id, resolution DESC
        "#
    )
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| (r.group_id, r.image_id, r.is_keeper as i64, r.resolution))
        .collect())
}

// ── Jobs ─────────────────────────────────────────────────────────────────────

pub async fn create_job(
    pool: &SqlitePool,
    operation: &str,
    params_json: &str,
    output_mode: &str,
    output_dir: Option<&str>,
    image_ids: &[i64],
) -> Result<i64> {
    let now = chrono::Utc::now().timestamp();
    let total = image_ids.len() as i64;

    let job_id = sqlx::query!(
        r#"
        INSERT INTO processing_jobs
            (status, operation, params_json, output_mode, output_dir, total_images, created_at)
        VALUES ('queued', ?, ?, ?, ?, ?, ?)
        "#,
        operation,
        params_json,
        output_mode,
        output_dir,
        total,
        now
    )
    .execute(pool)
    .await?
    .last_insert_rowid();

    for &image_id in image_ids {
        sqlx::query!(
            "INSERT INTO job_images (job_id, image_id, status) VALUES (?, ?, 'queued')",
            job_id,
            image_id
        )
        .execute(pool)
        .await?;
    }

    Ok(job_id)
}

pub async fn get_job(pool: &SqlitePool, job_id: i64) -> Result<Option<DbProcessingJob>> {
    let job = sqlx::query_as!(
        DbProcessingJob,
        "SELECT * FROM processing_jobs WHERE id = ?",
        job_id
    )
    .fetch_optional(pool)
    .await?;
    Ok(job)
}

pub async fn get_jobs_paginated(
    pool: &SqlitePool,
    status: Option<&str>,
    page: i64,
    page_size: i64,
) -> Result<(Vec<DbProcessingJob>, i64)> {
    let offset = page * page_size;
    let (jobs, total) = if let Some(s) = status {
        let jobs = sqlx::query_as!(
            DbProcessingJob,
            "SELECT * FROM processing_jobs WHERE status = ? ORDER BY created_at DESC LIMIT ? OFFSET ?",
            s,
            page_size,
            offset
        )
        .fetch_all(pool)
        .await?;
        let total: i64 = sqlx::query_scalar!(
            "SELECT COUNT(*) FROM processing_jobs WHERE status = ?",
            s
        )
        .fetch_one(pool)
        .await?;
        (jobs, total)
    } else {
        let jobs = sqlx::query_as!(
            DbProcessingJob,
            "SELECT * FROM processing_jobs ORDER BY created_at DESC LIMIT ? OFFSET ?",
            page_size,
            offset
        )
        .fetch_all(pool)
        .await?;
        let total: i64 =
            sqlx::query_scalar!("SELECT COUNT(*) FROM processing_jobs")
                .fetch_one(pool)
                .await?;
        (jobs, total)
    };
    Ok((jobs, total))
}

pub async fn get_job_images(pool: &SqlitePool, job_id: i64) -> Result<Vec<DbJobImage>> {
    let images = sqlx::query_as!(
        DbJobImage,
        "SELECT * FROM job_images WHERE job_id = ?",
        job_id
    )
    .fetch_all(pool)
    .await?;
    Ok(images)
}

pub async fn update_job_status(pool: &SqlitePool, job_id: i64, status: &str) -> Result<()> {
    let now = chrono::Utc::now().timestamp();
    match status {
        "running" => {
            sqlx::query!(
                "UPDATE processing_jobs SET status = ?, started_at = ? WHERE id = ?",
                status,
                now,
                job_id
            )
            .execute(pool)
            .await?;
        }
        "done" | "failed" | "cancelled" => {
            sqlx::query!(
                "UPDATE processing_jobs SET status = ?, finished_at = ? WHERE id = ?",
                status,
                now,
                job_id
            )
            .execute(pool)
            .await?;
        }
        _ => {
            sqlx::query!(
                "UPDATE processing_jobs SET status = ? WHERE id = ?",
                status,
                job_id
            )
            .execute(pool)
            .await?;
        }
    }
    Ok(())
}

pub async fn update_job_progress(
    pool: &SqlitePool,
    job_id: i64,
    processed: i64,
    failed: i64,
) -> Result<()> {
    sqlx::query!(
        "UPDATE processing_jobs SET processed_count = ?, failed_count = ? WHERE id = ?",
        processed,
        failed,
        job_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn update_job_image_status(
    pool: &SqlitePool,
    job_id: i64,
    image_id: i64,
    status: &str,
    error: Option<&str>,
) -> Result<()> {
    sqlx::query!(
        "UPDATE job_images SET status = ?, error = ? WHERE job_id = ? AND image_id = ?",
        status,
        error,
        job_id,
        image_id
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_failed_job_image_ids(pool: &SqlitePool, job_id: i64) -> Result<Vec<i64>> {
    let ids = sqlx::query_scalar!(
        "SELECT image_id FROM job_images WHERE job_id = ? AND status = 'failed'",
        job_id
    )
    .fetch_all(pool)
    .await?;
    Ok(ids)
}

// ── Settings ──────────────────────────────────────────────────────────────────

pub async fn get_setting(pool: &SqlitePool, key: &str) -> Result<Option<String>> {
    let val = sqlx::query_scalar!(
        "SELECT value_json FROM app_settings WHERE key = ?",
        key
    )
    .fetch_optional(pool)
    .await?;
    Ok(val)
}

pub async fn set_setting(pool: &SqlitePool, key: &str, value_json: &str) -> Result<()> {
    let now = chrono::Utc::now().timestamp();
    sqlx::query!(
        r#"
        INSERT INTO app_settings (key, value_json, updated_at) VALUES (?, ?, ?)
        ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json, updated_at = excluded.updated_at
        "#,
        key,
        value_json,
        now
    )
    .execute(pool)
    .await?;
    Ok(())
}
