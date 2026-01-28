mod db;
mod events;
mod state;
mod docker;
mod build;
mod api;
mod proxy;
mod ws_broadcaster;
mod github;
mod application;
mod infrastructure;
mod workers;

use anyhow::Result;
use axum::{routing::{get, post}, Router};
use tower_http::services::ServeDir;
use tower_http::trace::TraceLayer;
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use sqlx::SqlitePool;
use state::AppContext;
use build::run_build_worker;
use api::{api_routes, github_webhook, ws_handler};
use proxy::run_reverse_proxy;
use ws_broadcaster::run_ws_broadcaster;
use docker::DockerClient;
use application::ports::repositories::ProjectRepository;

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

    use sqlx::sqlite::SqliteConnectOptions;
    use std::str::FromStr;

    let options = SqliteConnectOptions::from_str(database_url)?
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await?;

    info!("Running database migrations");
    sqlx::migrate!("./migrations").run(&pool).await?;

    // Initialize webhook secret (generate if not exists)
    let webhook_secret: Option<String> = sqlx::query_scalar(
        "SELECT value FROM settings WHERE key = 'webhook_secret'"
    )
    .fetch_optional(&pool)
    .await?;

    if webhook_secret.is_none() {
        use rand::Rng;
        let secret: String = rand::thread_rng()
            .sample_iter(&rand::distributions::Alphanumeric)
            .take(64)
            .map(char::from)
            .collect();
        sqlx::query("INSERT INTO settings (key, value) VALUES ('webhook_secret', ?)")
            .bind(&secret)
            .execute(&pool)
            .await?;
        info!("Generated new webhook secret (check database or API)");
    }

    // Initialize Docker client to get gateway IP
    let docker = DockerClient::new_with_host_path_detection().await?;
    let gateway_ip = docker.gateway_ip().to_string();

    // Load base domain from database (defaults to albl.cloud)
    let base_domain: Option<String> = sqlx::query_scalar(
        "SELECT value FROM settings WHERE key = 'base_domain'"
    )
    .fetch_optional(&pool)
    .await?
    .or_else(|| Some("albl.cloud".to_string()));

    info!("Base domain: {:?}", base_domain);

    // Create application context (replaces AppState)
    let context = AppContext::new(pool.clone(), docker.clone(), gateway_ip, base_domain).await?;

    info!("Application context initialized");

    // Synchronize container states with database on startup
    info!("Synchronizing container states...");
    synchronize_container_states(&context, &docker).await?;
    info!("Container state synchronization complete");

    // Build API server routes
    let app = Router::new()
        .route("/webhook/github", post(github_webhook))
        .route("/ws", get(ws_handler))
        .nest("/api", api_routes())
        // Serve static files from /app/frontend directory
        .fallback_service(ServeDir::new("frontend"))
        .layer(TraceLayer::new_for_http())
        .with_state(context.clone());

    // Start API server (port 3000)
    let api_server = tokio::spawn({
        let context = context.clone();
        async move {
            let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
                .await
                .expect("Failed to bind port 3000");
            info!("API server listening on 0.0.0.0:3000");
            if let Err(e) = axum::serve(listener, app).await {
                error!("API server failed: {}", e);
            }
        }
    });

    // Start reverse proxy (port 8080)
    let reverse_proxy = tokio::spawn({
        let context = context.clone();
        async move {
            if let Err(e) = run_reverse_proxy(context).await {
                tracing::error!("Reverse proxy error: {}", e);
            }
        }
    });

    // Start build queue worker
    let build_worker = tokio::spawn({
        let context = context.clone();
        async move {
            if let Err(e) = run_build_worker(context).await {
                tracing::error!("Build worker error: {}", e);
            }
        }
    });

    // Start WebSocket broadcaster
    let ws_broadcaster = tokio::spawn({
        let context = context.clone();
        async move {
            if let Err(e) = run_ws_broadcaster(context).await {
                tracing::error!("WebSocket broadcaster error: {}", e);
            }
        }
    });

    // Start Port Scanner worker
    let port_scanner = tokio::spawn({
        let pool = pool.clone();
        async move {
            if let Err(e) = workers::run_port_scanner(pool).await {
                tracing::error!("Port scanner error: {}", e);
            }
        }
    });

    // Start Container Log Streamer worker
    let container_log_streamer = tokio::spawn({
        let context = context.clone();
        async move {
            if let Err(e) = workers::run_container_log_streamer(context).await {
                tracing::error!("Container log streamer error: {}", e);
            }
        }
    });

    // Start Container Cleanup worker
    let container_cleanup = tokio::spawn({
        let context = context.clone();
        async move {
            if let Err(e) = workers::run_container_cleanup(context).await {
                tracing::error!("Container cleanup worker error: {}", e);
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
        _ = port_scanner => {
            info!("Port scanner stopped");
        }
        _ = container_log_streamer => {
            info!("Container log streamer stopped");
        }
        _ = container_cleanup => {
            info!("Container cleanup worker stopped");
        }
    }

    info!("Shutting down...");

    Ok(())
}

/// Synchronize container states with database on startup
/// 1. Removes container IDs from DB if containers don't exist
/// 2. Removes orphan containers (containers without matching projects in DB)
async fn synchronize_container_states(context: &AppContext, docker: &DockerClient) -> Result<()> {
    let projects: Vec<crate::db::models::Project> = context.project_repo.list().await?;

    // Step 1: Check DB projects and clean up dead containers from DB
    for project in &projects {
        let mut needs_update = false;
        let mut new_blue_id = project.blue_container_id.clone();
        let mut new_green_id = project.green_container_id.clone();

        // Check Blue container
        if let Some(blue_id) = &project.blue_container_id {
            if !docker.is_container_running(blue_id).await {
                info!("Project '{}': Blue container {} not found, clearing from DB", project.name, blue_id);
                new_blue_id = None;
                needs_update = true;
            }
        }

        // Check Green container
        if let Some(green_id) = &project.green_container_id {
            if !docker.is_container_running(green_id).await {
                info!("Project '{}': Green container {} not found, clearing from DB", project.name, green_id);
                new_green_id = None;
                needs_update = true;
            }
        }

        // Update database if needed
        if needs_update {
            if new_blue_id != project.blue_container_id {
                context.project_repo.update_blue_container(project.id, new_blue_id).await?;
            }
            if new_green_id != project.green_container_id {
                context.project_repo.update_green_container(project.id, new_green_id).await?;
            }
            info!("Project '{}': Container states synchronized", project.name);
        }
    }

    // Step 2: Get standalone containers from DB
    use crate::application::ports::repositories::ContainerRepository;
    let db_containers: Vec<crate::db::models::Container> = context.container_repo.list().await?;

    // Build set of valid container names
    let valid_container_names: std::collections::HashSet<String> = db_containers.iter()
        .map(|c| c.name.clone())
        .collect();

    // Step 3: Clean up orphan containers (containers not in DB)
    #[allow(deprecated)]
    let all_containers = docker.docker_api()
        .list_containers(Some(bollard::container::ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        }))
        .await?;

    // Build set of valid project IDs
    let valid_project_ids: std::collections::HashSet<i64> = projects.iter()
        .map(|p| p.id)
        .collect();

    for container in all_containers {
        if let Some(names) = container.names {
            for name in names {
                let name = name.trim_start_matches('/');

                // Check project containers: project-{id}-{slot}
                let parts: Vec<&str> = name.split('-').collect();
                if parts.len() == 3 && parts[0] == "project" {
                    if let Ok(project_id) = parts[1].parse::<i64>() {
                        if parts[2] == "blue" || parts[2] == "green" {
                            // Valid project container - check if orphan
                            if !valid_project_ids.contains(&project_id) {
                                if let Some(container_id) = &container.id {
                                    info!("🧹 Cleaning up orphan project container: {} (project ID {} not in DB)", name, project_id);
                                    docker.stop_container(container_id).await.ok();
                                    docker.remove_container(container_id).await.ok();
                                }
                            }
                        }
                    }
                }
                // Check standalone containers: container-{name}
                else if parts.len() >= 2 && parts[0] == "container" {
                    let container_name = parts[1..].join("-");
                    if !valid_container_names.contains(&container_name) {
                        if let Some(container_id) = &container.id {
                            info!("🧹 Cleaning up orphan standalone container: {} (name '{}' not in DB)", name, container_name);
                            docker.stop_container(container_id).await.ok();
                            docker.remove_container(container_id).await.ok();
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
// force rebuild Wed Jan 21 01:57:39 PM KST 2026
// rebuild 1768971465
// rebuild 1768971470
// rebuild
