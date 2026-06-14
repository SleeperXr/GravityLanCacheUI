use std::sync::Arc;

use axum::{
    extract::{Query, State, WebSocketUpgrade},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;

use crate::AppState;

/// Build all API routes under /api/v1.
pub fn routes() -> Router<Arc<AppState>> {
    Router::new()
        .route("/health", get(health))
        .route("/dashboard", get(dashboard))
        .route("/downloads", get(downloads))
        .route("/setup/check", get(setup_check))
        .route("/config", get(get_config).put(update_config))
        .route("/logs", get(get_logs))
        .route("/prefill/status", get(prefill_status))
        .route("/prefill/run/{platform}", axum::routing::post(prefill_run))
        .route("/ws", get(ws_handler))
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
    match state.db.get_dashboard_stats().await {
        Ok(stats) => Json(serde_json::json!({
            "stats": stats,
        }))
        .into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
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
    Json(serde_json::json!({
        "lancache_logs_dir": config.lancache_logs_dir,
        "lancache_cache_dir": config.lancache_cache_dir,
        "cache_scan_interval_secs": config.cache_scan_interval_secs,
        "log_retention_days": config.log_retention_days,
        "excluded_ips": config.excluded_ips,
        "steam_api_key_set": config.steam_api_key.is_some(),
        "db_path": config.db_path,
    }))
}

#[derive(Deserialize)]
struct UpdateConfigInput {
    steam_api_key: Option<String>,
    cache_scan_interval_secs: u64,
    log_retention_days: u32,
    db_path: String,
    excluded_ips: Vec<String>,
}

async fn update_config(
    State(state): State<Arc<AppState>>,
    Json(body): Json<UpdateConfigInput>,
) -> impl IntoResponse {
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
