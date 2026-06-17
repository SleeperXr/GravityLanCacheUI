use std::sync::Arc;

use axum::{
    extract::{Query, State, WebSocketUpgrade, Request},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use crate::AppState;

/// Build all API routes under /api/v1.
pub fn routes(state: Arc<AppState>) -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health))
        .route("/dashboard", get(dashboard))
        .route("/downloads", get(downloads))
        .route("/setup/check", get(setup_check))
        .route("/config", get(get_config).put(update_config))
        .route("/logs", get(get_logs))
        .route("/prefill/status", get(prefill_status))
        .route("/prefill/run/{platform}", axum::routing::post(prefill_run))
        .route("/cache/latest", get(latest_cache_snapshot))
        .route("/tools/update_mappings", axum::routing::post(update_mappings))
        .route("/tools/reset_offset", axum::routing::post(reset_log_offset))
        .route("/ws", get(ws_handler))
        .layer(middleware::from_fn_with_state(state, auth_middleware))
}

// ── Health ───────────────────────────────────────────────────────────

async fn health() -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "status": "ok",
        "version": env!("CARGO_PKG_VERSION"),
    }))
}

// ── Dashboard ────────────────────────────────────────────────────────

async fn dashboard(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let stats = match state.db.get_dashboard_stats().await {
        Ok(stats) => stats,
        Err(e) => return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        ).into_response(),
    };

    let log_path = {
        let config = state.config.read().await;
        std::path::PathBuf::from(&config.lancache_logs_dir).join("access.log")
    };

    let parse_state = state.db.get_parse_state().await.unwrap_or_default();
    let current_offset = parse_state.last_offset;
    let total_size = log_path.metadata().map(|m| m.len() as i64).unwrap_or(current_offset);
    let is_catching_up = total_size - current_offset > 100 * 1024;
    let percentage = if total_size > 0 {
        (current_offset as f64 / total_size as f64) * 100.0
    } else {
        100.0
    };

    Json(serde_json::json!({
        "stats": stats,
        "parser_status": {
            "current_offset": current_offset,
            "total_size": total_size,
            "percentage": percentage,
            "is_catching_up": is_catching_up,
        }
    })).into_response()
}

// ── Downloads ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct DownloadQuery {
    limit: Option<u32>,
    offset: Option<u32>,
}

async fn downloads(
    State(state): State<Arc<AppState>>,
    Query(params): Query<DownloadQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50).min(200);
    let offset = params.offset.unwrap_or(0);

    match state.db.get_recent_downloads(limit, offset).await {
        Ok(events) => Json(serde_json::json!({
            "downloads": events,
            "limit": limit,
            "offset": offset,
        }))
        .into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ── Setup Wizard ─────────────────────────────────────────────────────

async fn setup_check(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let config = state.config.read().await;
    let checks = crate::setup_wizard::run_checks(&config).await;
    Json(serde_json::json!({ "checks": checks }))
}

// ── Config ───────────────────────────────────────────────────────────

async fn get_config(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let config = state.config.read().await;
    let mappings_count = state.db.get_game_mappings_count().await.unwrap_or(0);
    Json(serde_json::json!({
        "lancache_logs_dir": config.lancache_logs_dir,
        "lancache_cache_dir": config.lancache_cache_dir,
        "cache_scan_interval_secs": config.cache_scan_interval_secs,
        "log_retention_days": config.log_retention_days,
        "excluded_ips": config.excluded_ips,
        "steam_api_key_set": config.steam_api_key.is_some(),
        "db_path": config.db_path,
        "steam_mappings_count": mappings_count,
        "log_scan_days": config.log_scan_days,
    }))
}

#[derive(Deserialize)]
struct UpdateConfigInput {
    steam_api_key: Option<String>,
    cache_scan_interval_secs: u64,
    log_retention_days: u32,
    log_scan_days: u32,
    db_path: String,
    excluded_ips: Vec<String>,
}

async fn update_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdateConfigInput>,
) -> impl IntoResponse {
    if !is_safe_db_path(&body.db_path) {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid db_path: Path traversal components (..) are not allowed"})),
        )
            .into_response();
    }

    let mut config = state.config.write().await;
    
    // If the Steam API key is sent and isn't just the mask (••••••••) or empty, update it.
    if let Some(key) = body.steam_api_key {
        let trimmed_key = key.trim();
        if !trimmed_key.is_empty() && !trimmed_key.chars().all(|c| c == '•') {
            config.steam_api_key = Some(trimmed_key.to_string());
        } else if trimmed_key.is_empty() {
            config.steam_api_key = None;
        }
    }
    
    config.cache_scan_interval_secs = body.cache_scan_interval_secs;
    config.log_retention_days = body.log_retention_days;
    config.log_scan_days = body.log_scan_days;
    config.db_path = body.db_path;
    config.excluded_ips = body.excluded_ips;

    if let Err(e) = config.save_persisted() {
        return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response();
    }

    Json(serde_json::json!({"status": "saved"})).into_response()
}

// ── Prefill ──────────────────────────────────────────────────────────

async fn prefill_status(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    let cache_dir = {
        let config = state.config.read().await;
        config.lancache_cache_dir.clone()
    };
    let manager = crate::prefill::PrefillManager::new(&cache_dir);
    let statuses = manager.get_status().await;
    Json(serde_json::json!({ "platforms": statuses }))
}

async fn prefill_run(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(platform): axum::extract::Path<String>,
) -> impl IntoResponse {
    let cache_dir = {
        let config = state.config.read().await;
        config.lancache_cache_dir.clone()
    };
    let manager = crate::prefill::PrefillManager::new(&cache_dir);

    match manager.run_prefill(&platform).await {
        Ok(output) => Json(serde_json::json!({
            "status": "completed",
            "output": output,
        }))
        .into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

// ── WebSocket ────────────────────────────────────────────────────────

async fn ws_handler(
    State(state): State<Arc<AppState>>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_ws(socket, state))
}

async fn handle_ws(
    mut socket: axum::extract::ws::WebSocket,
    state: Arc<AppState>,
) {
    let mut rx = state.tx_broadcast.subscribe();

    // Send initial dashboard state
    if let Ok(stats) = state.db.get_dashboard_stats().await {
        if let Ok(json) = serde_json::to_string(&serde_json::json!({
            "type": "initial_state",
            "stats": stats,
        })) {
            let _ = socket
                .send(axum::extract::ws::Message::Text(json.into()))
                .await;
        }
    }

    // Forward broadcast messages to this WebSocket client
    loop {
        tokio::select! {
            msg = rx.recv() => {
                match msg {
                    Ok(text) => {
                        if socket.send(axum::extract::ws::Message::Text(text.into())).await.is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
            msg = socket.recv() => {
                match msg {
                    Some(Ok(_)) => {} // Ignore client messages for now
                    _ => break,
                }
            }
        }
    }
}

// ── Backend Logs ─────────────────────────────────────────────────────

async fn get_logs() -> impl IntoResponse {
    if let Ok(logs) = crate::BACKEND_LOGS.lock() {
        let list: Vec<String> = logs.iter().cloned().collect();
        Json(list)
    } else {
        Json(vec![])
    }
}

// ── Maintenance & Cache Tools ────────────────────────────────────────

async fn latest_cache_snapshot(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let snapshot = match state.db.get_latest_cache_snapshot().await {
        Ok(s) => s,
        Err(e) => return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ).into_response(),
    };

    let games = match state.db.get_cached_games_summary().await {
        Ok(g) => g,
        Err(e) => return (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        ).into_response(),
    };

    Json(serde_json::json!({
        "snapshot": snapshot,
        "games": games
    })).into_response()
}

async fn update_mappings(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let state_clone = state.clone();
    tokio::spawn(async move {
        if let Err(e) = crate::game_resolver::force_download_depot_mappings(state_clone).await {
            tracing::error!("Manual mapping update failed: {}", e);
        }
    });

    Json(serde_json::json!({
        "status": "ok",
        "message": "Steam depot mappings update started in background"
    })).into_response()
}

async fn reset_log_offset(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let parse_state = crate::db::ParseState {
        last_offset: 0,
        last_inode: 0,
    };
    match state.db.save_parse_state(&parse_state).await {
        Ok(()) => {
            tracing::info!("Log parser offset reset to 0 by user request");
            Json(serde_json::json!({
                "status": "ok",
                "message": "Log parser offset reset. Rewinding to beginning of log..."
            }))
            .into_response()
        }
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": e.to_string() })),
        )
            .into_response(),
    }
}

// ── Helpers & Middleware ─────────────────────────────────────────────

fn is_safe_db_path(path_str: &str) -> bool {
    let path = std::path::Path::new(path_str);
    for component in path.components() {
        if let std::path::Component::ParentDir = component {
            return false;
        }
    }
    true
}

async fn auth_middleware(
    State(state): State<Arc<AppState>>,
    req: Request,
    next: Next,
) -> Result<Response, impl IntoResponse> {
    let method = req.method();
    
    // Only verify auth for mutating requests (POST, PUT, DELETE, etc.)
    if method != axum::http::Method::GET && method != axum::http::Method::HEAD && method != axum::http::Method::OPTIONS {
        let config = state.config.read().await;
        if let Some(ref api_key) = config.api_key {
            let mut provided_key = None;
            
            if let Some(auth_header) = req.headers().get(axum::http::header::AUTHORIZATION) {
                if let Ok(auth_str) = auth_header.to_str() {
                    if auth_str.starts_with("Bearer ") {
                        provided_key = Some(auth_str[7..].trim());
                    } else {
                        provided_key = Some(auth_str.trim());
                    }
                }
            } else if let Some(key_header) = req.headers().get("X-API-Key") {
                if let Ok(key_str) = key_header.to_str() {
                    provided_key = Some(key_str.trim());
                }
            }

            match provided_key {
                Some(key) if key == api_key => {}
                _ => {
                    return Err((
                        axum::http::StatusCode::UNAUTHORIZED,
                        Json(serde_json::json!({"error": "Unauthorized: Invalid or missing API key"})),
                    ).into_response());
                }
            }
        }
    }
    
    Ok(next.run(req).await)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_safe_db_path() {
        assert!(is_safe_db_path("db.sqlite"));
        assert!(is_safe_db_path("/data/db.sqlite"));
        assert!(is_safe_db_path("/data/gravity/db.sqlite"));
        assert!(!is_safe_db_path("../db.sqlite"));
        assert!(!is_safe_db_path("/data/../db.sqlite"));
        assert!(!is_safe_db_path("foo/bar/../../db.sqlite"));
    }
}
