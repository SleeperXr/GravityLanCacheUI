mod service_detector;

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::sync::Arc;

use chrono::NaiveDateTime;
use once_cell::sync::Lazy;
use regex::Regex;

use crate::db::{DownloadEvent, HourlyStats, ParseState};
use crate::AppState;

pub use service_detector::ServiceDetector;

/// Regex for the LanCache Monolithic custom 'cachelog' format:
/// [$cacheidentifier] $remote_addr / $http_x_forwarded_for - $remote_user [$time_local] "$request" $status $body_bytes_sent "$http_referer" "$http_user_agent" "$upstream_cache_status" "$host" "$http_range"
static CACHELOG_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"^\[([^\]]+)\]\s+(\S+)\s+/\s+\S+\s+-\s+\S+\s+\[([^\]]+)\]\s+"(\S+)\s+(\S+)\s+\S+"\s+(\d{3})\s+(\d+)\s+"[^"]*"\s+"[^"]*"\s+"([^"]*)"\s+"([^"]*)"(?:\s+"[^"]*")?"#
    )
    .expect("Invalid cachelog regex")
});

/// Fallback Regex for standard NGINX combined log format with custom fields:
/// $remote_addr - - [$time_local] "$request" $status $body_bytes_sent "$http_referer" "$http_user_agent" "$upstream_cache_status" "$host"
static COMBINED_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(
        r#"^(\S+)\s+-\s+\S+\s+\[([^\]]+)\]\s+"(\S+)\s+(\S+)\s+\S+"\s+(\d{3})\s+(\d+)\s+"[^"]*"\s+"[^"]*"\s+"([^"]*)"\s+"([^"]*)""#
    )
    .expect("Invalid combined regex")
});

/// Regex to extract Steam depot ID from URL path: /depot/XXXXX/
static STEAM_DEPOT_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"/depot/(\d+)/").expect("Invalid depot regex")
});

/// A single parsed log line.
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub struct LogEntry {
    pub client_ip: String,
    pub timestamp: String,
    pub method: String,
    pub url: String,
    pub status: u16,
    pub bytes_sent: i64,
    pub host: String,
    pub cache_status: String,
    pub service: String,
    pub download_id: Option<String>,
}

/// Parse a single NGINX access log line into a structured entry.
fn parse_log_line(line: &str) -> Option<LogEntry> {
    // 1. Try to parse with LanCache monolithic 'cachelog' regex
    if let Some(caps) = CACHELOG_REGEX.captures(line) {
        let service_ident = caps.get(1)?.as_str().to_string();
        let client_ip = caps.get(2)?.as_str().to_string();
        let timestamp = caps.get(3)?.as_str().to_string();
        let method = caps.get(4)?.as_str().to_string();
        let url = caps.get(5)?.as_str().to_string();
        let status: u16 = caps.get(6)?.as_str().parse().ok()?;
        let bytes_sent: i64 = caps.get(7)?.as_str().parse().ok()?;
        let cache_status = caps.get(8)?.as_str().to_uppercase();
        let host = caps.get(9)?.as_str().to_string();

        // Use the cache identifier (e.g. "steam", "blizzard") if it is specific, otherwise fallback to host detection
        let service = if service_ident == "generic" || service_ident.is_empty() {
            ServiceDetector::detect(&host)
        } else {
            service_ident
        };
        let download_id = extract_download_id(&service, &url);

        return Some(LogEntry {
            client_ip,
            timestamp,
            method,
            url,
            status,
            bytes_sent,
            host,
            cache_status,
            service,
            download_id,
        });
    }

    // 2. Try combined nginx format fallback
    if let Some(caps) = COMBINED_REGEX.captures(line) {
        let client_ip = caps.get(1)?.as_str().to_string();
        let timestamp = caps.get(2)?.as_str().to_string();
        let method = caps.get(3)?.as_str().to_string();
        let url = caps.get(4)?.as_str().to_string();
        let status: u16 = caps.get(5)?.as_str().parse().ok()?;
        let bytes_sent: i64 = caps.get(6)?.as_str().parse().ok()?;
        let cache_status = caps.get(7)?.as_str().to_uppercase();
        let host = caps.get(8)?.as_str().to_string();

        let service = ServiceDetector::detect(&host);
        let download_id = extract_download_id(&service, &url);

        return Some(LogEntry {
            client_ip,
            timestamp,
            method,
            url,
            status,
            bytes_sent,
            host,
            cache_status,
            service,
            download_id,
        });
    }

    None
}

/// Extract a platform-specific download identifier from the URL.
fn extract_download_id(service: &str, url: &str) -> Option<String> {
    match service {
        "steam" => STEAM_DEPOT_REGEX
            .captures(url)
            .and_then(|c| c.get(1))
            .map(|m| m.as_str().to_string()),
        "epicgames" => url.split('/').nth(1).map(|s| s.to_string()),
        _ => None,
    }
}

/// Parse NGINX timestamp "14/Jun/2026:13:05:22 +0200" to ISO string.
fn parse_timestamp(raw: &str) -> String {
    NaiveDateTime::parse_from_str(
        raw.split_once(' ').map(|(d, _)| d).unwrap_or(raw),
        "%d/%b/%Y:%H:%M:%S",
    )
    .map(|dt| dt.format("%Y-%m-%dT%H:%M:%S").to_string())
    .unwrap_or_else(|_| chrono::Utc::now().format("%Y-%m-%dT%H:%M:%S").to_string())
}

/// Extract the hour portion "2026-06-14T13" from an ISO timestamp.
fn hour_key(ts: &str) -> String {
    ts.get(..13).unwrap_or(ts).to_string()
}

/// Main log parser loop: reads existing lines, then tails for new ones.
pub async fn run_log_parser(
    state: Arc<AppState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    tracing::info!("Log parser task started");

    // If the database has 0 download events, reset the parser offset to 0
    // so we parse historical logs to populate the initial dashboard.
    if let Ok(count) = state.db.get_download_events_count().await {
        if count == 0 {
            tracing::info!("Database has 0 download events, resetting log parser offset to 0 to parse history");
            let parse_state = ParseState { last_offset: 0, last_inode: 0 };
            let _ = state.db.save_parse_state(&parse_state).await;
        }
    }

    loop {
        let log_path = {
            let config = state.config.read().await;
            std::path::PathBuf::from(&config.lancache_logs_dir).join("access.log")
        };

        if !log_path.exists() {
            tracing::warn!("access.log not found at {}, waiting...", log_path.display());
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            continue;
        }

        let parse_state = state.db.get_parse_state().await.unwrap_or_default();
        tracing::info!(
            "Resuming from offset {} bytes for {}",
            parse_state.last_offset,
            log_path.display()
        );

        match process_log_file(&log_path, &state, parse_state).await {
            Ok(()) => {}
            Err(e) => {
                tracing::error!("Log processing error: {}, retrying in 5s", e);
                tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            }
        }
    }
}

/// Process the log file: seek to last known offset, read lines, then tail.
struct ActiveDownload {
    event: DownloadEvent,
    dirty: bool,
}

async fn flush_to_db(
    active_downloads: &mut HashMap<(String, String, Option<String>), ActiveDownload>,
    hourly_stats: &mut HashMap<(String, String), HourlyStats>,
    state: &Arc<AppState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    // 1. Flush hourly stats (additive)
    for stats in hourly_stats.values() {
        state.db.upsert_hourly_stats(stats).await?;
    }
    hourly_stats.clear();

    // 2. Flush dirty active downloads
    for ad in active_downloads.values_mut() {
        if ad.dirty {
            state.db.update_download_event(&ad.event).await?;
            ad.dirty = false;
        }
    }

    Ok(())
}

async fn process_log_file(
    path: &std::path::Path,
    state: &Arc<AppState>,
    mut parse_state: ParseState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let file = std::fs::File::open(path)?;
    let mut file_len = file.metadata()?.len() as i64;

    // If file is smaller than last offset, it was rotated — restart from beginning
    let mut start_offset = if file_len < parse_state.last_offset {
        tracing::info!("Log file rotated (size {} < offset {}), restarting", file_len, parse_state.last_offset);
        0
    } else {
        parse_state.last_offset
    };

    // Cap the initial scan size to avoid hanging on massive files.
    // If we are starting from 0 (first run or reset) and the file is larger than 250 MB,
    // skip to the last 200 MB of the log to compile recent history quickly.
    const MAX_INITIAL_SCAN_BYTES: i64 = 200_000_000; // 200 MB
    if start_offset == 0 && file_len > (MAX_INITIAL_SCAN_BYTES + 50_000_000) {
        let skip_to = file_len - MAX_INITIAL_SCAN_BYTES;
        tracing::info!(
            "Log file is extremely large ({:.2} GB). Skipping to last {} MB to parse recent history quickly.",
            file_len as f64 / 1_000_000_000.0,
            MAX_INITIAL_SCAN_BYTES / 1_000_000
        );
        start_offset = skip_to;
    }

    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(start_offset as u64))?;

    let mut line_buf = String::new();
    let mut lines_processed: u64 = 0;
    let mut current_offset = start_offset;
    let mut last_update_time = std::time::Instant::now();

    // In-memory aggregates for performance (avoids slamming SQLite for every log line)
    let mut active_downloads = HashMap::<(String, String, Option<String>), ActiveDownload>::new();
    let mut hourly_stats = HashMap::<(String, String), HourlyStats>::new();

    loop {
        line_buf.clear();
        let bytes_read = reader.read_line(&mut line_buf)?;

        if bytes_read == 0 {
            // Flush remaining aggregates to DB before sleeping
            flush_to_db(&mut active_downloads, &mut hourly_stats, state).await?;

            // Only write state and broadcast if we actually processed lines since the last idle check
            if lines_processed > 0 {
                parse_state.last_offset = current_offset;
                state.db.save_parse_state(&parse_state).await?;
                tracing::info!("Processed {} lines, offset at {}", lines_processed, current_offset);
                lines_processed = 0;

                // Broadcast that parser is caught up
                if let Ok(json) = serde_json::to_string(&serde_json::json!({
                    "type": "parser_status",
                    "current_offset": current_offset,
                    "total_size": current_offset,
                    "percentage": 100.0,
                    "is_catching_up": false,
                })) {
                    let _ = state.tx_broadcast.send(json);
                }
            }

            tokio::time::sleep(std::time::Duration::from_secs(1)).await;

            // Check if offset was reset in database
            if let Ok(db_state) = state.db.get_parse_state().await {
                if db_state.last_offset < current_offset {
                    // Flush before returning so we don't lose anything in-memory
                    flush_to_db(&mut active_downloads, &mut hourly_stats, state).await?;
                    tracing::info!(
                        "Log parser offset reset detected in DB (current={} > DB={}). Rewinding parser...",
                        current_offset,
                        db_state.last_offset
                    );
                    return Ok(()); // returns to run_log_parser, which will reload offset from DB
                }
            }

            // Reopen file to check for new content (handles SMB caching issues)
            drop(reader);
            let file = std::fs::File::open(path)?;
            let new_len = file.metadata()?.len() as i64;

            if new_len < current_offset {
                // Flush before returning
                flush_to_db(&mut active_downloads, &mut hourly_stats, state).await?;
                tracing::info!("Log rotated during tail, restarting");
                parse_state.last_offset = 0;
                state.db.save_parse_state(&parse_state).await?;
                return Ok(());
            }

            file_len = new_len; // Update file_len so that our percentages remain correct
            reader = BufReader::new(file);
            reader.seek(SeekFrom::Start(current_offset as u64))?;
            continue;
        }

        current_offset += bytes_read as i64;
        let trimmed = line_buf.trim();

        if trimmed.is_empty() {
            continue;
        }

        if let Some(entry) = parse_log_line(trimmed) {
            // Skip local loopback and empty (0-byte) requests
            if entry.client_ip == "127.0.0.1" || entry.client_ip == "::1" || entry.client_ip == "localhost" || entry.bytes_sent <= 0 {
                continue;
            }

            // Skip excluded IPs based on parsed client IP
            let is_excluded = {
                let config = state.config.read().await;
                config.excluded_ips.iter().any(|ip| entry.client_ip == *ip || entry.client_ip.starts_with(ip))
            };
            if is_excluded {
                continue;
            }

            let ts = parse_timestamp(&entry.timestamp);
            let hour = hour_key(&ts);
            let is_hit = entry.cache_status == "HIT";

            // 1. Update hourly stats in-memory (delta accumulation)
            let stats_key = (hour.clone(), entry.service.clone());
            let stats = hourly_stats.entry(stats_key).or_insert_with(|| HourlyStats {
                hour,
                service: entry.service.clone(),
                total_bytes: 0,
                hit_bytes: 0,
                miss_bytes: 0,
                request_count: 0,
                unique_clients: 0,
            });
            stats.total_bytes += entry.bytes_sent;
            if is_hit {
                stats.hit_bytes += entry.bytes_sent;
            } else {
                stats.miss_bytes += entry.bytes_sent;
            }
            stats.request_count += 1;

            // 2. Group into download events (5 minute window)
            let download_key = (entry.client_ip.clone(), entry.service.clone(), entry.download_id.clone());
            let mut needs_new = true;

            if let Some(ad) = active_downloads.get_mut(&download_key) {
                let ts_dt = NaiveDateTime::parse_from_str(&ts, "%Y-%m-%dT%H:%M:%S")
                    .unwrap_or_else(|_| chrono::Utc::now().naive_utc());
                let ended_dt = NaiveDateTime::parse_from_str(&ad.event.ended_at, "%Y-%m-%dT%H:%M:%S")
                    .unwrap_or_else(|_| chrono::Utc::now().naive_utc());

                if ts_dt.signed_duration_since(ended_dt).num_minutes() < 5 {
                    // Update existing in-memory event
                    ad.event.ended_at = ts.clone();
                    ad.event.total_bytes += entry.bytes_sent;
                    if is_hit {
                        ad.event.hit_bytes += entry.bytes_sent;
                    } else {
                        ad.event.miss_bytes += entry.bytes_sent;
                    }
                    ad.event.request_count += 1;
                    ad.event.hit_rate = if ad.event.total_bytes > 0 {
                        (ad.event.hit_bytes as f64 / ad.event.total_bytes as f64) * 100.0
                    } else {
                        0.0
                    };

                    // Resolve game name if it is still None
                    if ad.event.game_name.is_none() {
                        if let Some(ref dl_id) = ad.event.download_id {
                            ad.event.game_name = crate::game_resolver::GameResolver::resolve(state, &ad.event.service, dl_id).await;
                        }
                    }

                    ad.dirty = true;
                    needs_new = false;
                } else {
                    // Timed out! Flush the old event to the DB if it was dirty
                    if ad.dirty {
                        state.db.update_download_event(&ad.event).await?;
                    }
                    active_downloads.remove(&download_key);
                }
            }

            if needs_new {
                // Check DB for active download first (could be from previous runs)
                let cutoff_dt = NaiveDateTime::parse_from_str(&ts, "%Y-%m-%dT%H:%M:%S")
                    .unwrap_or_else(|_| chrono::Utc::now().naive_utc())
                    - chrono::Duration::minutes(5);
                let cutoff = cutoff_dt.format("%Y-%m-%dT%H:%M:%S").to_string();

                let existing = state.db.find_active_download(
                    entry.client_ip.clone(),
                    entry.service.clone(),
                    entry.download_id.clone(),
                    cutoff,
                ).await?;

                match existing {
                    Some(mut event) => {
                        // Found active in DB, load and update in-memory
                        event.ended_at = ts;
                        event.total_bytes += entry.bytes_sent;
                        if is_hit {
                            event.hit_bytes += entry.bytes_sent;
                        } else {
                            event.miss_bytes += entry.bytes_sent;
                        }
                        event.request_count += 1;
                        event.hit_rate = if event.total_bytes > 0 {
                            (event.hit_bytes as f64 / event.total_bytes as f64) * 100.0
                        } else {
                            0.0
                        };

                        if event.game_name.is_none() {
                            if let Some(ref dl_id) = event.download_id {
                                event.game_name = crate::game_resolver::GameResolver::resolve(state, &event.service, dl_id).await;
                            }
                        }

                        active_downloads.insert(download_key, ActiveDownload {
                            event,
                            dirty: true,
                        });
                    }
                    None => {
                        // Create a brand new download event
                        let game_name = if let Some(ref dl_id) = entry.download_id {
                            crate::game_resolver::GameResolver::resolve(state, &entry.service, dl_id).await
                        } else {
                            None
                        };

                        let display_name = game_name.clone().unwrap_or_else(|| entry.download_id.clone().unwrap_or_else(|| "Unknown".to_string()));
                        tracing::info!(
                            "📥 New download detected: service={}, game/id='{}', client_ip={}",
                            entry.service,
                            display_name,
                            entry.client_ip
                        );

                        let mut event = DownloadEvent {
                            id: 0,
                            client_ip: entry.client_ip.clone(),
                            service: entry.service.clone(),
                            download_id: entry.download_id.clone(),
                            game_name,
                            started_at: ts.clone(),
                            ended_at: ts,
                            total_bytes: entry.bytes_sent,
                            hit_bytes: if is_hit { entry.bytes_sent } else { 0 },
                            miss_bytes: if is_hit { 0 } else { entry.bytes_sent },
                            request_count: 1,
                            hit_rate: if is_hit { 100.0 } else { 0.0 },
                            app_id: None,
                        };

                        // Insert immediately to get its database ID
                        let id = state.db.insert_download_event(&event).await?;
                        event.id = id;

                        active_downloads.insert(download_key, ActiveDownload {
                            event,
                            dirty: false, // clean since it was just inserted
                        });

                        // Broadcast new download event via WebSocket
                        if let Ok(json) = serde_json::to_string(&serde_json::json!({
                            "type": "new_download",
                            "service": entry.service,
                            "client_ip": entry.client_ip,
                            "download_id": entry.download_id,
                            "bytes": entry.bytes_sent,
                            "cache_status": entry.cache_status,
                        })) {
                            let _ = state.tx_broadcast.send(json);
                        }
                    }
                }
            }
        }

        lines_processed += 1;

        // Periodically save state and broadcast progress every 1000 lines or 1 second
        let now_instant = std::time::Instant::now();
        if lines_processed % 1000 == 0 || now_instant.duration_since(last_update_time).as_secs() >= 1 {
            last_update_time = now_instant;
            
            // Flush aggregates to the database!
            flush_to_db(&mut active_downloads, &mut hourly_stats, state).await?;

            parse_state.last_offset = current_offset;
            state.db.save_parse_state(&parse_state).await?;

            let percentage = if file_len > 0 {
                (current_offset as f64 / file_len as f64) * 100.0
            } else {
                100.0
            };

            let is_catching_up = file_len - current_offset > 100 * 1024; // > 100 KB behind
            if let Ok(json) = serde_json::to_string(&serde_json::json!({
                "type": "parser_status",
                "current_offset": current_offset,
                "total_size": file_len,
                "percentage": percentage,
                "is_catching_up": is_catching_up,
            })) {
                let _ = state.tx_broadcast.send(json);
            }
        }
    }
}
