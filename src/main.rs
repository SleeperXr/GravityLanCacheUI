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

use config::AppConfig;
use db::Database;

/// Shared application state accessible by all handlers.
pub struct AppState {
    pub db: Database,
    pub config: AppConfig,
    pub tx_broadcast: tokio::sync::broadcast::Sender<String>,
}

#[tokio::main]
async fn main() {
    tracing_subscriber::registry()
        .with(tracing_subscriber::EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| "gravitylancacheui=info,tower_http=info".into()))
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("🚀 GravityLancacheUI starting...");

    let config = AppConfig::load();
    tracing::info!("Configuration loaded: {:?}", config);

    let db = Database::new(&config.db_path)
        .await
        .expect("Failed to initialize database");
    db.run_migrations()
        .await
        .expect("Failed to run database migrations");
    tracing::info!("Database initialized at {}", config.db_path);

    let (tx_broadcast, _) = tokio::sync::broadcast::channel::<String>(256);

    let state = std::sync::Arc::new(AppState {
        db,
        config: config.clone(),
        tx_broadcast: tx_broadcast.clone(),
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

    let app = Router::new()
        .nest("/api/v1", api::routes())
        .fallback_service(ServeDir::new("static").append_index_html_on_directories(true))
        .layer(CorsLayer::permissive())
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
