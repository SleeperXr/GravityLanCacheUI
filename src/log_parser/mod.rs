mod service_detector;

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
    let log_path = std::path::PathBuf::from(&state.config.lancache_logs_dir).join("access.log");
    tracing::info!("Log parser watching: {}", log_path.display());

    loop {
        if !log_path.exists() {
            tracing::warn!("access.log not found at {}, waiting...", log_path.display());
            tokio::time::sleep(std::time::Duration::from_secs(5)).await;
            continue;
        }

        let parse_state = state.db.get_parse_state().await.unwrap_or_default();
        tracing::info!(
            "Resuming from offset {} bytes",
            parse_state.last_offset
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
async fn process_log_file(
    path: &std::path::Path,
    state: &Arc<AppState>,
    mut parse_state: ParseState,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let file = std::fs::File::open(path)?;
    let file_len = file.metadata()?.len() as i64;

    // If file is smaller than last offset, it was rotated — restart from beginning
    let start_offset = if file_len < parse_state.last_offset {
        tracing::info!("Log file rotated (size {} < offset {}), restarting", file_len, parse_state.last_offset);
        0
    } else {
        parse_state.last_offset
    };

    let mut reader = BufReader::new(file);
    reader.seek(SeekFrom::Start(start_offset as u64))?;

    let mut line_buf = String::new();
    let mut lines_processed: u64 = 0;
    let mut current_offset = start_offset;

    loop {
        line_buf.clear();
        let bytes_read = reader.read_line(&mut line_buf)?;

        if bytes_read == 0 {
            // Save state and wait for new lines
            parse_state.last_offset = current_offset;
            state.db.save_parse_state(&parse_state).await?;

            if lines_processed > 0 {
                tracing::info!("Processed {} lines, offset at {}", lines_processed, current_offset);
                lines_processed = 0;
            }

            tokio::time::sleep(std::time::Duration::from_secs(1)).await;

            // Reopen file to check for new content (handles SMB caching issues)
            drop(reader);
            let file = std::fs::File::open(path)?;
            let new_len = file.metadata()?.len() as i64;

            if new_len < current_offset {
                tracing::info!("Log rotated during tail, restarting");
                parse_state.last_offset = 0;
                state.db.save_parse_state(&parse_state).await?;
                return Ok(());
            }

            reader = BufReader::new(file);
            reader.seek(SeekFrom::Start(current_offset as u64))?;
            continue;
        }

        current_offset += bytes_read as i64;
        let trimmed = line_buf.trim();

        if trimmed.is_empty() {
            continue;
        }

        // Skip excluded IPs
        if state
            .config
            .excluded_ips
            .iter()
            .any(|ip| trimmed.starts_with(ip.as_str()))
        {
            continue;
        }

        if let Some(entry) = parse_log_line(trimmed) {
            if let Err(e) = process_entry(&entry, state).await {
                tracing::warn!("Failed to process log entry: {}", e);
            }
        }

        lines_processed += 1;

        // Periodically save state every 1000 lines
        if lines_processed % 1000 == 0 {
            parse_state.last_offset = current_offset;
            state.db.save_parse_state(&parse_state).await?;
        }
    }
}

/// Process a single log entry: update or create a download event.
async fn process_entry(
    entry: &LogEntry,
    state: &Arc<AppState>,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let ts = parse_timestamp(&entry.timestamp);
    let hour = hour_key(&ts);
    let is_hit = entry.cache_status == "HIT";

    // Upsert hourly stats
    let stats = HourlyStats {
        hour,
        service: entry.service.clone(),
        total_bytes: entry.bytes_sent,
        hit_bytes: if is_hit { entry.bytes_sent } else { 0 },
        miss_bytes: if is_hit { 0 } else { entry.bytes_sent },
        request_count: 1,
        unique_clients: 0,
    };
    state.db.upsert_hourly_stats(&stats).await?;

    // Group into download events (5 minute window)
    let cutoff_dt = chrono::NaiveDateTime::parse_from_str(&ts, "%Y-%m-%dT%H:%M:%S")
        .unwrap_or_else(|_| chrono::Utc::now().naive_utc())
        - chrono::Duration::minutes(5);
    let cutoff = cutoff_dt.format("%Y-%m-%dT%H:%M:%S").to_string();

    let existing = state
        .db
        .find_active_download(
            entry.client_ip.clone(),
            entry.service.clone(),
            entry.download_id.clone(),
            cutoff,
        )
        .await?;

    match existing {
        Some(mut event) => {
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
            state.db.update_download_event(&event).await?;
        }
        None => {
            let event = DownloadEvent {
                id: 0,
                client_ip: entry.client_ip.clone(),
                service: entry.service.clone(),
                download_id: entry.download_id.clone(),
                game_name: None,
                started_at: ts.clone(),
                ended_at: ts,
                total_bytes: entry.bytes_sent,
                hit_bytes: if is_hit { entry.bytes_sent } else { 0 },
                miss_bytes: if is_hit { 0 } else { entry.bytes_sent },
                request_count: 1,
                hit_rate: if is_hit { 100.0 } else { 0.0 },
            };
            state.db.insert_download_event(&event).await?;

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

    Ok(())
}
