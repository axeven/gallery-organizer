use anyhow::Result;
use sqlx::SqlitePool;

use crate::db::queries;
use crate::scanner::hash::hamming_distance;

pub async fn rebuild(pool: &SqlitePool, max_distance: u32) -> Result<usize> {
    queries::delete_groups_by_type(pool, "duplicate_cluster").await?;

    let images = queries::get_all_images_with_hash(pool).await?;

    if images.is_empty() {
        return Ok(0);
    }

    // Union-Find for clustering
    let n = images.len();
    let mut parent: Vec<usize> = (0..n).collect();

    fn find(parent: &mut Vec<usize>, x: usize) -> usize {
        if parent[x] != x {
            parent[x] = find(parent, parent[x]);
        }
        parent[x]
    }

    fn union(parent: &mut Vec<usize>, x: usize, y: usize) {
        let px = find(parent, x);
        let py = find(parent, y);
        if px != py {
            parent[px] = py;
        }
    }

    // O(n²) hamming distance comparison — acceptable up to ~50k images
    for i in 0..n {
        for j in (i + 1)..n {
            let hash_a = images[i].perceptual_hash.as_deref().unwrap_or("");
            let hash_b = images[j].perceptual_hash.as_deref().unwrap_or("");

            if let Some(dist) = hamming_distance(hash_a, hash_b) {
                if dist <= max_distance {
                    union(&mut parent, i, j);
                }
            }
        }
    }

    // Collect clusters
    let mut clusters: std::collections::HashMap<usize, Vec<usize>> =
        std::collections::HashMap::new();

    for i in 0..n {
        let root = find(&mut parent, i);
        clusters.entry(root).or_default().push(i);
    }

    let mut cluster_count = 0;

    for (_, members) in clusters {
        if members.len() < 2 {
            continue; // not a duplicate
        }

        let label = format!("Duplicate Cluster #{}", cluster_count + 1);
        let group_id = queries::insert_group(pool, "duplicate_cluster", &label).await?;

        let image_ids: Vec<i64> = members.iter().map(|&i| images[i].id).collect();
        queries::bulk_insert_group_members(pool, group_id, &image_ids).await?;

        // Set keeper = highest resolution image
        if let Some(&best_idx) = members.iter().max_by_key(|&&i| {
            let w = images[i].width_px.unwrap_or(0);
            let h = images[i].height_px.unwrap_or(0);
            w * h
        }) {
            queries::set_keeper(pool, group_id, images[best_idx].id).await?;
        }

        cluster_count += 1;
    }

    Ok(cluster_count)
}
