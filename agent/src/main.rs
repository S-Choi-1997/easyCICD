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
use docker::DockerClient;

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

    // Initialize Docker client to get gateway IP
    let docker = DockerClient::new_with_host_path_detection().await?;
    let gateway_ip = docker.gateway_ip().to_string();

    // Load base domain from database (defaults to albl.cloud)
    let base_domain = db.get_domain().await
        .ok()
        .flatten()
        .or_else(|| Some("albl.cloud".to_string()));

    info!("Base domain: {:?}", base_domain);

    // Create application state
    let state = AppState::new(db, gateway_ip, base_domain);

    info!("Application state initialized");

    // Synchronize container states with database on startup
    info!("Synchronizing container states...");
    synchronize_container_states(&state, &docker).await?;
    info!("Container state synchronization complete");

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

/// Synchronize container states with database on startup
/// 1. Removes container IDs from DB if containers don't exist
/// 2. Removes orphan containers (containers without matching projects in DB)
async fn synchronize_container_states(state: &AppState, docker: &DockerClient) -> Result<()> {
    let projects = state.db.list_projects().await?;

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
            state.db.update_project_blue_container(project.id, new_blue_id).await?;
            state.db.update_project_green_container(project.id, new_green_id).await?;
            info!("Project '{}': Container states synchronized", project.name);
        }
    }

    // Step 2: Clean up orphan containers (containers without projects in DB)
    use bollard::container::ListContainersOptions;
    use std::collections::HashMap;

    let mut filters = HashMap::new();
    filters.insert("name".to_string(), vec!["project-".to_string()]);

    let containers = docker.docker_api()
        .list_containers(Some(ListContainersOptions {
            all: true,
            filters,
            ..Default::default()
        }))
        .await?;

    // Build set of valid project IDs
    let valid_project_ids: std::collections::HashSet<i64> = projects.iter()
        .map(|p| p.id)
        .collect();

    for container in containers {
        if let Some(names) = container.names {
            for name in names {
                // Container name format: /project-{id}-{slot}
                // Strict matching: must be exactly "project-{number}-{blue|green}"
                let name = name.trim_start_matches('/');

                // Parse container name with strict format check
                let parts: Vec<&str> = name.split('-').collect();
                if parts.len() == 3 && parts[0] == "project" {
                    // Check if second part is a number (project ID)
                    if let Ok(project_id) = parts[1].parse::<i64>() {
                        // Check if third part is valid slot name
                        if parts[2] == "blue" || parts[2] == "green" {
                            // Valid project container format - check if orphan
                            if !valid_project_ids.contains(&project_id) {
                                // Orphan container found - remove it
                                if let Some(container_id) = &container.id {
                                    info!("🧹 Cleaning up orphan container: {} (project ID {} not in DB)", name, project_id);
                                    docker.stop_container(container_id).await.ok();
                                    docker.remove_container(container_id).await.ok();
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
