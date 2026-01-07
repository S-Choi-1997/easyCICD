mod db;
mod events;
mod state;
mod docker;
mod build;
mod api;
mod proxy;
mod ws_broadcaster;

use anyhow::Result;
use axum::{routing::{get, post}, Router};
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use db::Database;
use state::AppState;
use build::run_build_worker;
use api::{api_routes, github_webhook, ws_handler};
use proxy::run_reverse_proxy;
use ws_broadcaster::run_ws_broadcaster;

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "lightweight_ci=debug,tower_http=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Lightweight CI/CD Agent");

    // Initialize database
    let database_url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "sqlite:///data/db/ci.db".to_string());

    info!("Connecting to database: {}", database_url);
    let db = Database::connect(&database_url).await?;

    info!("Running database migrations");
    db.migrate().await?;

    // Create application state
    let state = AppState::new(db);

    info!("Application state initialized");

    // Build API server routes
    let app = Router::new()
        .route("/webhook/github", post(github_webhook))
        .route("/ws", get(ws_handler))
        .nest("/api", api_routes())
        .with_state(state.clone());

    // Start API server (port 3000)
    let api_server = tokio::spawn({
        let state = state.clone();
        async move {
            let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
                .await
                .expect("Failed to bind port 3000");
            info!("API server listening on 0.0.0.0:3000");
            axum::serve(listener, app)
                .await
                .expect("API server failed");
        }
    });

    // Start reverse proxy (port 8080)
    let reverse_proxy = tokio::spawn({
        let state = state.clone();
        async move {
            if let Err(e) = run_reverse_proxy(state).await {
                tracing::error!("Reverse proxy error: {}", e);
            }
        }
    });

    // Start build queue worker
    let build_worker = tokio::spawn({
        let state = state.clone();
        async move {
            if let Err(e) = run_build_worker(state).await {
                tracing::error!("Build worker error: {}", e);
            }
        }
    });

    // Start WebSocket broadcaster
    let ws_broadcaster = tokio::spawn({
        let state = state.clone();
        async move {
            if let Err(e) = run_ws_broadcaster(state).await {
                tracing::error!("WebSocket broadcaster error: {}", e);
            }
        }
    });

    info!("All services started successfully");

    // Keep the application running
    tokio::select! {
        _ = tokio::signal::ctrl_c() => {
            info!("Received shutdown signal");
        }
        _ = api_server => {
            info!("API server stopped");
        }
        _ = reverse_proxy => {
            info!("Reverse proxy stopped");
        }
        _ = build_worker => {
            info!("Build worker stopped");
        }
        _ = ws_broadcaster => {
            info!("WebSocket broadcaster stopped");
        }
    }

    info!("Shutting down...");

    Ok(())
}
