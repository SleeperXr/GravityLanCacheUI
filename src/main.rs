use std::net::SocketAddr;

use axum::Router;
use tower_http::{
    cors::CorsLayer,
    services::ServeDir,
    trace::TraceLayer,
};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

mod api;
mod cache_analyzer;
mod config;
mod db;
mod game_resolver;
mod log_parser;
mod prefill;
mod setup_wizard;
mod network_monitor;

use std::collections::VecDeque;
use std::sync::Mutex;
use once_cell::sync::Lazy;
use std::io::Write;

/// Thread-safe in-memory circular buffer for storing backend logs
pub static BACKEND_LOGS: Lazy<Mutex<VecDeque<String>>> = Lazy::new(|| {
    Mutex::new(VecDeque::with_capacity(1000))
});

/// A custom writer for tracing that logs to both stdout (for Docker) and the BACKEND_LOGS buffer
struct LogWriter;

impl Write for LogWriter {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let msg = String::from_utf8_lossy(buf).to_string();
        
        // Print to standard output (so docker logs still work)
        std::io::stdout().write_all(buf)?;
        
        // Push to in-memory logs
        if let Ok(mut logs) = BACKEND_LOGS.lock() {
            if logs.len() >= 1000 {
                logs.pop_front();
            }
            logs.push_back(msg);
        }
        
        Ok(buf.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        std::io::stdout().flush()
    }
}

use config::AppConfig;
use db::Database;

/// Shared application state accessible by all handlers.
pub struct AppState {
    pub db: Database,
    pub config: tokio::sync::RwLock<AppConfig>,
    pub tx_broadcast: tokio::sync::broadcast::Sender<String>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "gravitylancacheui=info,tower_http=info".into()))
        .with(tracing_subscriber::fmt::layer().with_writer(|| LogWriter))
        .init();

    tracing::info!("🚀 GravityLancacheUI v{} starting...", env!("CARGO_PKG_VERSION"));

    let config = AppConfig::load();
    tracing::info!("Configuration loaded: {:?}", config);

    let db = Database::new(&config.db_path)
        .await
        .expect("Failed to initialize database");
    db.run_migrations()
        .await
        .expect("Failed to run database migrations");
    tracing::info!("Database initialized at {}", config.db_path);

    if let Ok(count) = db.get_game_mappings_count().await {
        tracing::info!("Loaded {} game mappings from database", count);
    }

    let (tx_broadcast, _) = tokio::sync::broadcast::channel::<String>(256);

    let state = std::sync::Arc::new(AppState {
        db,
        config: tokio::sync::RwLock::new(config.clone()),
        tx_broadcast: tx_broadcast.clone(),
    });

    // Automatically download Steam depot mappings if none exist
    let mapping_state = state.clone();
    tokio::spawn(async move {
        game_resolver::check_and_download_depot_mappings(mapping_state).await;
    });

    // Spawn background tasks
    let parser_state = state.clone();
    tokio::spawn(async move {
        if let Err(e) = log_parser::run_log_parser(parser_state).await {
            tracing::error!("Log parser failed: {}", e);
        }
    });

    let analyzer_state = state.clone();
    tokio::spawn(async move {
        cache_analyzer::run_cache_analyzer(analyzer_state).await;
    });

    let net_state = state.clone();
    tokio::spawn(async move {
        network_monitor::run_network_monitor(net_state).await;
    });

    let prefill_scheduler_state = state.clone();
    tokio::spawn(async move {
        prefill::run_prefill_scheduler(prefill_scheduler_state).await;
    });

    let cors = CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods([
            axum::http::Method::GET,
            axum::http::Method::POST,
            axum::http::Method::PUT,
            axum::http::Method::DELETE,
        ])
        .allow_headers([
            axum::http::header::CONTENT_TYPE,
            axum::http::header::AUTHORIZATION,
            axum::http::header::HeaderName::from_static("x-api-key"),
        ]);

    let app = Router::new()
        .nest("/api/v1", api::routes(state.clone()))
        .fallback_service(ServeDir::new("static").append_index_html_on_directories(true))
        .layer(cors)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let addr = SocketAddr::from(([0, 0, 0, 0], config.listen_port));
    tracing::info!("🌐 Listening on http://{}", addr);

    let listener = tokio::net::TcpListener::bind(addr)
        .await
        .expect("Failed to bind to address");

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
        .expect("Server error");
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C handler");
    tracing::info!("🛑 Shutdown signal received, gracefully stopping...");
}
