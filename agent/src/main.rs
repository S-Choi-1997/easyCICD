mod db;
mod events;
mod state;

use anyhow::Result;
use tracing::info;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use db::Database;
use state::AppState;

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
    let _state = AppState::new(db);

    info!("Application state initialized");

    // TODO: Start API server on port 3000
    // TODO: Start reverse proxy on port 8080
    // TODO: Start build queue worker
    // TODO: Start WebSocket broadcaster

    info!("Lightweight CI/CD Agent started successfully");

    // Keep the application running
    tokio::signal::ctrl_c().await?;
    info!("Shutting down...");

    Ok(())
}
