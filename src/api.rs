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
        .route("/prefill/stop/{platform}", axum::routing::post(prefill_stop))
        .route("/prefill/config", get(get_prefill_config).put(update_prefill_config))
        .route("/prefill/log/{platform}", get(prefill_log))
        .route("/prefill/interactive/{platform}", get(prefill_interactive_ws))
        .route("/prefill/select/{platform}", get(get_selected_apps).post(save_selected_apps))
        .route("/cache/latest", get(latest_cache_snapshot))
        .route("/cache/scan", axum::routing::post(trigger_cache_scan))
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
        "prefill_dir": config.prefill_dir,
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
    prefill_dir: String,
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

    if !is_safe_db_path(&body.prefill_dir) {
        return (
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Invalid prefill_dir: Path traversal components (..) are not allowed"})),
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
    config.prefill_dir = body.prefill_dir;

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
    let prefill_dir = {
        let config = state.config.read().await;
        config.prefill_dir.clone()
    };
    let db_parent = {
        let config = state.config.read().await;
        std::path::Path::new(&config.db_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/data/gravitylancacheui".to_string())
    };
    let manager = crate::prefill::PrefillManager::new(&prefill_dir);
    let statuses = manager.get_status(&db_parent).await;
    Json(serde_json::json!({ "platforms": statuses }))
}

async fn prefill_run(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(platform): axum::extract::Path<String>,
) -> impl IntoResponse {
    let prefill_dir = {
        let config = state.config.read().await;
        config.prefill_dir.clone()
    };
    let db_parent = {
        let config = state.config.read().await;
        std::path::Path::new(&config.db_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/data/gravitylancacheui".to_string())
    };

    let manager = crate::prefill::PrefillManager::new(&prefill_dir);

    match manager.run_prefill_async(&platform, &db_parent).await {
        Ok(_) => Json(serde_json::json!({
            "status": "started",
            "message": format!("Prefill for {} started in background", platform),
        }))
        .into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e.to_string()})),
        )
            .into_response(),
    }
}

async fn prefill_stop(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(platform): axum::extract::Path<String>,
) -> impl IntoResponse {
    let prefill_dir = {
        let config = state.config.read().await;
        config.prefill_dir.clone()
    };
    let manager = crate::prefill::PrefillManager::new(&prefill_dir);

    match manager.stop_prefill(&platform) {
        Ok(_) => Json(serde_json::json!({
            "status": "stopped",
            "message": format!("Prefill for {} stopped successfully", platform),
        }))
        .into_response(),
        Err(e) => (
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": e})),
        )
            .into_response(),
    }
}

async fn get_prefill_config(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    let db_parent = {
        let config = state.config.read().await;
        std::path::Path::new(&config.db_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/data/gravitylancacheui".to_string())
    };
    let config = crate::prefill::PrefillManager::load_config(&db_parent);
    Json(config)
}

async fn update_prefill_config(
    State(state): State<Arc<AppState>>,
    Json(new_config): Json<crate::prefill::PrefillConfig>,
) -> impl IntoResponse {
    let db_parent = {
        let config = state.config.read().await;
        std::path::Path::new(&config.db_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/data/gravitylancacheui".to_string())
    };
    
    match crate::prefill::PrefillManager::save_config(&db_parent, &new_config) {
        Ok(_) => Json(serde_json::json!({ "status": "saved" })).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({ "error": format!("Failed to save config: {}", e) })),
        )
            .into_response(),
    }
}

async fn prefill_log(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(platform): axum::extract::Path<String>,
) -> impl IntoResponse {
    let db_parent = {
        let config = state.config.read().await;
        std::path::Path::new(&config.db_path)
            .parent()
            .map(|p| p.to_string_lossy().to_string())
            .unwrap_or_else(|| "/data/gravitylancacheui".to_string())
    };

    let log_file_path = std::path::Path::new(&db_parent)
        .join(format!("prefill_{}_last.log", platform));

    if log_file_path.exists() {
        match std::fs::read_to_string(&log_file_path) {
            Ok(content) => Json(serde_json::json!({
                "platform": platform,
                "log": content,
            }))
            .into_response(),
            Err(e) => (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to read log: {}", e)})),
            )
                .into_response(),
        }
    } else {
        Json(serde_json::json!({
            "platform": platform,
            "log": "No log file found. Run a prefill first.",
        }))
        .into_response()
    }
}

async fn get_selected_apps(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(platform): axum::extract::Path<String>,
) -> impl IntoResponse {
    let prefill_dir = {
        let config = state.config.read().await;
        config.prefill_dir.clone()
    };
    let manager = crate::prefill::PrefillManager::new(&prefill_dir);

    match manager.get_selected_apps_raw(&platform) {
        Ok(ids) => Json(serde_json::json!({
            "platform": platform,
            "app_ids": ids,
        })).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e})),
        ).into_response(),
    }
}

async fn save_selected_apps(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(platform): axum::extract::Path<String>,
    Json(body): Json<serde_json::Value>,
) -> impl IntoResponse {
    let prefill_dir = {
        let config = state.config.read().await;
        config.prefill_dir.clone()
    };
    let manager = crate::prefill::PrefillManager::new(&prefill_dir);

    let app_ids: Vec<serde_json::Value> = match body.get("app_ids").and_then(|v| v.as_array()) {
        Some(arr) => {
            let mut ids = Vec::new();
            for val in arr {
                if platform == "steam" {
                    if let Some(id) = val.as_u64() {
                        ids.push(serde_json::Value::Number(id.into()));
                    } else if let Some(s) = val.as_str() {
                        if let Ok(id) = s.parse::<u64>() {
                            ids.push(serde_json::Value::Number(id.into()));
                        } else {
                            return (
                                axum::http::StatusCode::BAD_REQUEST,
                                Json(serde_json::json!({"error": format!("Steam App ID must be a positive integer: {}", s)})),
                            ).into_response();
                        }
                    } else {
                        return (
                            axum::http::StatusCode::BAD_REQUEST,
                            Json(serde_json::json!({"error": "Steam App IDs must be integers or numeric strings"})),
                        ).into_response();
                    }
                } else {
                    if let Some(s) = val.as_str() {
                        let trimmed = s.trim().to_string();
                        if !trimmed.is_empty() {
                            ids.push(serde_json::Value::String(trimmed));
                        }
                    } else if let Some(id) = val.as_u64() {
                        ids.push(serde_json::Value::Number(id.into()));
                    } else {
                        return (
                            axum::http::StatusCode::BAD_REQUEST,
                            Json(serde_json::json!({"error": "App IDs must be strings or integers"})),
                        ).into_response();
                    }
                }
            }
            ids
        }
        None => {
            return (
                axum::http::StatusCode::BAD_REQUEST,
                Json(serde_json::json!({"error": "Missing or invalid 'app_ids' array in request body"})),
            ).into_response();
        }
    };

    match manager.save_selected_apps(&platform, &app_ids) {
        Ok(_) => Json(serde_json::json!({
            "status": "saved",
            "count": app_ids.len(),
        })).into_response(),
        Err(e) => (
            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": e})),
        ).into_response(),
    }
}

async fn prefill_interactive_ws(
    State(state): State<Arc<AppState>>,
    axum::extract::Path(platform): axum::extract::Path<String>,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_prefill_interactive_socket(socket, platform, state))
}

async fn handle_prefill_interactive_socket(
    mut socket: axum::extract::ws::WebSocket,
    platform: String,
    state: Arc<AppState>,
) {
    use futures::{SinkExt, StreamExt};
    
    let prefill_dir = {
        let config = state.config.read().await;
        config.prefill_dir.clone()
    };
    
    let (dir_name, binary) = match platform.as_str() {
        "steam" => ("SteamPrefill", "SteamPrefill"),
        "battlenet" => ("BattleNetPrefill", "BattleNetPrefill"),
        "epic" => ("EpicPrefill", "EpicPrefill"),
        _ => {
            let _ = socket.close().await;
            return;
        }
    };

    let binary_path = format!("{}/{}/{}", prefill_dir, dir_name, binary);
    let working_dir = format!("{}/{}", prefill_dir, dir_name);

    if !std::path::Path::new(&binary_path).exists() {
        let _ = socket.send(axum::extract::ws::Message::Text(format!("Error: Prefill binary not found at {}", binary_path).into())).await;
        let _ = socket.close().await;
        return;
    }

    let mut cmd = if cfg!(target_os = "linux") {
        let mut c = tokio::process::Command::new("socat");
        c.args(&["-", &format!("EXEC:\"{} select-apps\",pty,stderr,setsid,sigint,sane", binary_path)]);
        c
    } else {
        let mut c = tokio::process::Command::new(&binary_path);
        c.arg("select-apps");
        c
    };

    let mut child = match cmd
        .current_dir(&working_dir)
        .stdin(std::process::Stdio::piped())
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .env("TERM", "xterm") 
        .spawn() 
    {
        Ok(c) => c,
        Err(e) => {
            let _ = socket.send(axum::extract::ws::Message::Text(format!("Error spawning process: {}", e).into())).await;
            let _ = socket.close().await;
            return;
        }
    };

    let mut stdin = child.stdin.take().unwrap();
    let stdout = child.stdout.take().unwrap();
    let stderr = child.stderr.take().unwrap();

    let (ws_tx, mut ws_rx) = tokio::sync::mpsc::channel::<axum::extract::ws::Message>(100);
    let (mut ws_sender, mut ws_receiver) = socket.split();

    let stdout_tx_chan = ws_tx.clone();
    let stdout_tx = async move {
        let mut stdout_reader = tokio::io::BufReader::new(stdout);
        let mut stdout_buf = vec![0u8; 1024];
        loop {
            use tokio::io::AsyncReadExt;
            match stdout_reader.read(&mut stdout_buf).await {
                Ok(0) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&stdout_buf[..n]);
                    if stdout_tx_chan.send(axum::extract::ws::Message::Text(text.into_owned().into())).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    };

    let stderr_tx_chan = ws_tx.clone();
    let stderr_tx = async move {
        let mut stderr_reader = tokio::io::BufReader::new(stderr);
        let mut stderr_buf = vec![0u8; 1024];
        loop {
            use tokio::io::AsyncReadExt;
            match stderr_reader.read(&mut stderr_buf).await {
                Ok(0) => break,
                Ok(n) => {
                    let text = String::from_utf8_lossy(&stderr_buf[..n]);
                    if stderr_tx_chan.send(axum::extract::ws::Message::Text(text.into_owned().into())).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    };

    let stdin_rx = async move {
        while let Some(Ok(msg)) = ws_receiver.next().await {
            if let axum::extract::ws::Message::Text(text) = msg {
                use tokio::io::AsyncWriteExt;
                let mut input_str = text.to_string();
                if !input_str.ends_with('\n') {
                    input_str.push('\n');
                }
                if stdin.write_all(input_str.as_bytes()).await.is_err() {
                    break;
                }
                let _ = stdin.flush().await;
            }
        }
    };

    let ws_writer = async move {
        while let Some(msg) = ws_rx.recv().await {
            if ws_sender.send(msg).await.is_err() {
                break;
            }
        }
    };

    let wait_proc = async {
        let _ = child.wait().await;
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
    };

    tokio::select! {
        _ = stdout_tx => {},
        _ = stderr_tx => {},
        _ = stdin_rx => {},
        _ = ws_writer => {},
        _ = wait_proc => {},
    }

    let _ = child.kill().await;
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

async fn trigger_cache_scan(State(state): State<Arc<AppState>>) -> impl IntoResponse {
    match crate::cache_analyzer::trigger_single_scan(state).await {
        Ok(msg) => Json(serde_json::json!({
            "status": "ok",
            "message": msg
        }))
        .into_response(),
        Err(e) => (
            axum::http::StatusCode::BAD_REQUEST,
            Json(serde_json::json!({ "error": e })),
        )
            .into_response(),
    }
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
