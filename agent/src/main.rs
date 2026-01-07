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
use tower_http::services::ServeDir;
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
                .unwrap_or_else(|_| "info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("Starting Easy CI/CD Agent");

    // Create data directory structure
    std::fs::create_dir_all("/data/easycicd")?;
    info!("Data directory initialized");

    // Initialize database
    let database_url = "sqlite:///data/easycicd/db.sqlite";
    info!("Connecting to database: {}", database_url);
    let db = Database::connect(database_url).await?;

    info!("Running database migrations");
    db.migrate().await?;

    // Initialize webhook secret (generate if not exists)
    if let Ok(None) = db.get_setting("webhook_secret").await {
        use rand::Rng;
        let secret: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();
        db.set_setting("webhook_secret", &secret).await?;
        info!("Generated new webhook secret (check database or API)");
    }

    // Create application state
    let state = AppState::new(db);

    info!("Application state initialized");

    // Build API server routes
    let app = Router::new()
        .route("/webhook/github", post(github_webhook))
        .route("/ws", get(ws_handler))
        .nest("/api", api_routes())
        // Serve static files from /app/frontend directory
        .fallback_service(ServeDir::new("frontend"))
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
