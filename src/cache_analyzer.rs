use std::sync::Arc;
use crate::db::CacheSnapshot;
use crate::AppState;

pub async fn run_cache_analyzer(state: Arc<AppState>) {
    tracing::info!("Cache analyzer started: background task active");

    loop {
        // Read configuration dynamically on each iteration
        let (cache_dir, interval_secs) = {
            let config = state.config.read().await;
            (
                std::path::PathBuf::from(&config.lancache_cache_dir),
                config.cache_scan_interval_secs,
            )
        };

        if interval_secs == 0 {
            tokio::time::sleep(std::time::Duration::from_secs(10)).await;
            continue;
        }

        if cache_dir.exists() {
            tracing::info!("Cache analyzer: starting scan of {}...", cache_dir.display());
            match scan_cache_directory(&cache_dir).await {
                Ok(snapshot) => {
                    tracing::info!(
                        "Cache snapshot: {} files, {} bytes total",
                        snapshot.total_files,
                        snapshot.total_size_bytes
                    );
                    if let Err(e) = state.db.insert_cache_snapshot(&snapshot).await {
                        tracing::error!("Failed to save cache snapshot: {}", e);
                    }

                    // Broadcast cache update
                    if let Ok(json) = serde_json::to_string(&serde_json::json!({
                        "type": "cache_update",
                        "total_size_bytes": snapshot.total_size_bytes,
                        "total_files": snapshot.total_files,
                    })) {
                        let _ = state.tx_broadcast.send(json);
                    }
                }
                Err(e) => {
                    tracing::warn!("Cache scan failed: {}", e);
                }
            }
        } else {
            tracing::warn!("Cache directory not found: {}", cache_dir.display());
        }

        tokio::time::sleep(std::time::Duration::from_secs(interval_secs)).await;
    }
}

/// Walk the cache directory tree and compute aggregate stats.
async fn scan_cache_directory(
    path: &std::path::Path,
) -> Result<CacheSnapshot, Box<dyn std::error::Error + Send + Sync>> {
    let path = path.to_path_buf();

    // Run the blocking directory walk on a dedicated thread
    let snapshot = tokio::task::spawn_blocking(move || {
        let mut total_size: i64 = 0;
        let mut total_files: i64 = 0;

        fn walk_dir(dir: &std::path::Path, size: &mut i64, count: &mut i64) {
            let entries = match std::fs::read_dir(dir) {
                Ok(e) => e,
                Err(_) => return,
            };

            for entry in entries.flatten() {
                let metadata = match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                if metadata.is_dir() {
                    walk_dir(&entry.path(), size, count);
                } else {
                    *size += metadata.len() as i64;
                    *count += 1;
                }
            }
        }

        walk_dir(&path, &mut total_size, &mut total_files);

        CacheSnapshot {
            total_size_bytes: total_size,
            total_files,
            details_json: None,
            taken_at: None,
        }
    })
    .await?;

    Ok(snapshot)
}
