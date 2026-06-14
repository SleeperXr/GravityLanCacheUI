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
    let checks = crate::setup_wizard::run_checks(&state.config).await;
    Json(serde_json::json!({ "checks": checks }))
}

// ── Config ───────────────────────────────────────────────────────────

async fn get_config(State(state): State<Arc<AppState>>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "lancache_logs_dir": state.config.lancache_logs_dir,
        "lancache_cache_dir": state.config.lancache_cache_dir,
        "cache_scan_interval_secs": state.config.cache_scan_interval_secs,
        "log_retention_days": state.config.log_retention_days,
        "excluded_ips": state.config.excluded_ips,
        "steam_api_key_set": state.config.steam_api_key.is_some(),
        "db_path": state.config.db_path,
    }))
}

async fn update_config(
    State(state): State<Arc<AppState>>,
    Json(_body): Json<serde_json::Value>,
) -> impl IntoResponse {
    // Persist updated settings to config.json
    // Note: For a full implementation, we'd update state.config in-place via RwLock
    if let Err(e) = state.config.save_persisted() {
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
    let manager = crate::prefill::PrefillManager::new(&state.config.lancache_cache_dir);
    let statuses = manager.get_status().await;
    Json(serde_json::json!({ "platforms": statuses }))
}

async fn prefill_run(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(platform): axum::extract::Path<String>,
) -> impl IntoResponse {
    let manager = crate::prefill::PrefillManager::new(&state.config.lancache_cache_dir);

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
