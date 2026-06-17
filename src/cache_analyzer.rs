use std::sync::Arc;
use std::collections::HashMap;
use std::io::Read;
use once_cell::sync::Lazy;
use regex::Regex;

use crate::db::CacheSnapshot;
use crate::AppState;

static STEAM_DEPOT_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"depot/(\d+)/").expect("Invalid depot regex")
});

struct ScanResult {
    pub total_size_bytes: i64,
    pub total_files: i64,
    pub breakdown: HashMap<(String, Option<String>), (i64, i64)>, // (service, download_id) -> (bytes, count)
}

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
                Ok(result) => {
                    // Resolve game names and build details JSON
                    let mut details_list = Vec::new();
                    
                    for ((service, download_id), (size_bytes, file_count)) in result.breakdown {
                        let game_name = if let Some(ref dl_id) = download_id {
                            state.db.get_game_name(service.clone(), dl_id.clone()).await.unwrap_or(None)
                        } else {
                            None
                        };
                        
                        let app_id = if service == "steam" {
                            if let Some(ref dl_id) = download_id {
                                state.db.get_steam_app_id(dl_id.clone()).await.unwrap_or(None)
                            } else {
                                None
                            }
                        } else {
                            None
                        };
                        
                        details_list.push(serde_json::json!({
                            "service": service,
                            "download_id": download_id,
                            "game_name": game_name,
                            "app_id": app_id,
                            "size_bytes": size_bytes,
                            "file_count": file_count,
                        }));
                    }
                    
                    let details_json = serde_json::to_string(&serde_json::json!({
                        "items": details_list
                    })).ok();
                    
                    let snapshot = CacheSnapshot {
                        total_size_bytes: result.total_size_bytes,
                        total_files: result.total_files,
                        details_json,
                        taken_at: None,
                    };
                    
                    tracing::info!(
                        "Cache snapshot completed: {} files, {} bytes total, {} categorized items",
                        snapshot.total_files,
                        snapshot.total_size_bytes,
                        details_list.len()
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

async fn scan_cache_directory(
    path: &std::path::Path,
) -> Result<ScanResult, Box<dyn std::error::Error + Send + Sync>> {
    let path = path.to_path_buf();

    let result = tokio::task::spawn_blocking(move || {
        let mut total_size: i64 = 0;
        let mut total_files: i64 = 0;
        let mut breakdown = HashMap::new();

        fn walk_dir(
            dir: &std::path::Path,
            size: &mut i64,
            count: &mut i64,
            breakdown: &mut HashMap<(String, Option<String>), (i64, i64)>,
        ) {
            let entries = match std::fs::read_dir(dir) {
                Ok(e) => e,
                Err(_) => return,
            };

            for entry in entries.flatten() {
                let entry_path = entry.path();
                let metadata = match entry.metadata() {
                    Ok(m) => m,
                    Err(_) => continue,
                };

                if metadata.is_dir() {
                    walk_dir(&entry_path, size, count, breakdown);
                } else {
                    let file_size = metadata.len() as i64;
                    *size += file_size;
                    *count += 1;

                    // Extract URL key to categorize
                    if file_size > 100 {
                        if let Some(url) = extract_url_from_nginx_cache_file(&entry_path) {
                            let (service, download_id) = parse_url_info(&url);
                            let entry = breakdown.entry((service, download_id)).or_insert((0, 0));
                            entry.0 += file_size;
                            entry.1 += 1;
                        } else {
                            let entry = breakdown.entry(("unknown".to_string(), None)).or_insert((0, 0));
                            entry.0 += file_size;
                            entry.1 += 1;
                        }
                    }
                }
            }
        }

        walk_dir(&path, &mut total_size, &mut total_files, &mut breakdown);

        ScanResult {
            total_size_bytes: total_size,
            total_files,
            breakdown,
        }
    })
    .await?;

    Ok(result)
}

fn extract_url_from_nginx_cache_file(path: &std::path::Path) -> Option<String> {
    let mut file = std::fs::File::open(path).ok()?;
    let mut buf = [0u8; 512];
    let bytes_read = file.read(&mut buf).ok()?;
    let content = String::from_utf8_lossy(&buf[..bytes_read]);
    
    if let Some(idx) = content.find("KEY: ") {
        let sub = &content[idx + 5..];
        let end_idx = sub.find(|c: char| c == '\0' || c == '\n' || c == '\r' || c.is_control() || c == '"')
            .unwrap_or(sub.len());
        let key = sub[..end_idx].trim().to_string();
        if !key.is_empty() {
            return Some(key);
        }
    }
    if let Some(idx) = content.find("http://") {
        let sub = &content[idx..];
        let end_idx = sub.find(|c: char| c == '\0' || c == '\n' || c == '\r' || c.is_control() || c == ' ' || c == '"')
            .unwrap_or(sub.len());
        return Some(sub[..end_idx].to_string());
    }
    if let Some(idx) = content.find("https://") {
        let sub = &content[idx..];
        let end_idx = sub.find(|c: char| c == '\0' || c == '\n' || c == '\r' || c.is_control() || c == ' ' || c == '"')
            .unwrap_or(sub.len());
        return Some(sub[..end_idx].to_string());
    }
    None
}

fn parse_url_info(url: &str) -> (String, Option<String>) {
    let url_sans_protocol = url.strip_prefix("http://")
        .or_else(|| url.strip_prefix("https://"))
        .unwrap_or(url);
    
    let (host, path) = url_sans_protocol.split_once('/')
        .unwrap_or((url_sans_protocol, ""));
        
    let host_lower = host.to_lowercase();
    let service = match host_lower.as_str() {
        "steam" | "epicgames" | "gog" | "origin" | "ubisoft" | "battlenet" | 
        "windowsupdate" | "xbox" | "riotgames" | "nintendo" | "playstation" | 
        "rockstar" | "arenanet" | "wargaming" => host_lower,
        _ => crate::log_parser::ServiceDetector::detect(host),
    };
    
    let download_id = match service.as_str() {
        "steam" => {
            STEAM_DEPOT_REGEX.captures(path)
                .and_then(|caps| caps.get(1))
                .map(|m| m.as_str().to_string())
        }
        "epicgames" => {
            path.split('/').next().map(|s| s.to_string())
        }
        _ => None,
    };
    
    (service, download_id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_url_info_with_protocol() {
        let (service, download_id) = parse_url_info("http://cache1.steamcontent.com/depot/228990/chunk/12345");
        assert_eq!(service, "steam");
        assert_eq!(download_id, Some("228990".to_string()));
    }

    #[test]
    fn test_parse_url_info_no_protocol() {
        let (service, download_id) = parse_url_info("steam/depot/228990/chunk/12345");
        assert_eq!(service, "steam");
        assert_eq!(download_id, Some("228990".to_string()));
    }

    #[test]
    fn test_parse_url_info_epicgames_no_protocol() {
        let (service, download_id) = parse_url_info("epicgames/apps/ids/12345");
        assert_eq!(service, "epicgames");
        assert_eq!(download_id, Some("apps".to_string()));
    }

    #[test]
    fn test_parse_url_info_other_service() {
        let (service, download_id) = parse_url_info("battlenet/tpr/sc2/data/abcd");
        assert_eq!(service, "battlenet");
        assert_eq!(download_id, None);
    }
}
