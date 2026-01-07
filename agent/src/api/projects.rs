use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::{fs, process::Command};
use tracing::{info, warn};

use crate::db::models::{CreateBuild, CreateProject, Slot};
use crate::docker::client::DockerClient;
use crate::state::AppState;

pub fn projects_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_projects).post(create_project))
        .route("/{id}", get(get_project).delete(delete_project))
        .route("/{id}/builds", post(trigger_build))
}

async fn list_projects(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.list_projects().await {
        Ok(projects) => (StatusCode::OK, Json(projects)),
        Err(e) => {
            warn!("Failed to list projects: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(vec![]))
        }
    }
}

async fn get_project(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.db.get_project(id).await {
        Ok(Some(project)) => (StatusCode::OK, Json(Some(project))),
        Ok(None) => (StatusCode::NOT_FOUND, Json(None)),
        Err(e) => {
            warn!("Failed to get project: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(None))
        }
    }
}

#[derive(Deserialize)]
struct CreateProjectRequest {
    name: String,
    repo: String,
    path_filter: String,
    branch: String,
    build_image: String,
    build_command: String,
    cache_type: String,
    runtime_image: String,
    runtime_command: String,
    health_check_url: String,
}

async fn create_project(
    State(state): State<AppState>,
    Json(req): Json<CreateProjectRequest>,
) -> impl IntoResponse {
    let create_project = CreateProject {
        name: req.name,
        repo: req.repo,
        path_filter: req.path_filter,
        branch: req.branch,
        build_image: req.build_image,
        build_command: req.build_command,
        cache_type: req.cache_type,
        runtime_image: req.runtime_image,
        runtime_command: req.runtime_command,
        health_check_url: req.health_check_url,
    };

    match state.db.create_project(create_project).await {
        Ok(project) => (StatusCode::CREATED, Json(Some(project))),
        Err(e) => {
            warn!("Failed to create project: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(None))
        }
    }
}


async fn trigger_build(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    // Get project
    let project = match state.db.get_project(id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            )
        }
        Err(e) => {
            warn!("Failed to get project: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Database error"})),
            );
        }
    };

    // Get current commit hash from workspace
    let workspace_path = PathBuf::from("/workspace")
        .join("projects")
        .join(&project.name);

    let commit_hash = if workspace_path.exists() {
        // Get current commit hash
        match Command::new("git")
            .args(&["-C", workspace_path.to_str().unwrap(), "rev-parse", "HEAD"])
            .output()
            .await
        {
            Ok(output) if output.status.success() => {
                String::from_utf8_lossy(&output.stdout).trim().to_string()
            }
            _ => "HEAD".to_string(),
        }
    } else {
        "HEAD".to_string()
    };

    // Get commit message and author
    let (commit_message, author) = if workspace_path.exists() && commit_hash != "HEAD" {
        let message = Command::new("git")
            .args(&[
                "-C",
                workspace_path.to_str().unwrap(),
                "log",
                "-1",
                "--format=%s",
            ])
            .output()
            .await
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            });

        let author = Command::new("git")
            .args(&[
                "-C",
                workspace_path.to_str().unwrap(),
                "log",
                "-1",
                "--format=%an <%ae>",
            ])
            .output()
            .await
            .ok()
            .and_then(|o| {
                if o.status.success() {
                    Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                } else {
                    None
                }
            });

        (message, author)
    } else {
        (None, None)
    };

    // Create build
    let create_build = CreateBuild {
        project_id: project.id,
        commit_hash: commit_hash.clone(),
        commit_message,
        author,
    };

    let build = match state.db.create_build(create_build).await {
        Ok(b) => b,
        Err(e) => {
            warn!("Failed to create build: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to create build"})),
            );
        }
    };

    // Enqueue build
    state.build_queue.enqueue(project.id, build.id).await;

    (
        StatusCode::CREATED,
        Json(serde_json::json!({
            "build_id": build.id,
            "commit_hash": commit_hash,
            "message": "Build triggered successfully"
        })),
    )
}

async fn delete_project(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    // Get project first
    let project = match state.db.get_project(id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            )
        }
        Err(e) => {
            warn!("Failed to get project: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Database error"})),
            );
        }
    };

    // Create Docker client for cleanup
    let docker = match DockerClient::new() {
        Ok(d) => d,
        Err(e) => {
            warn!("Failed to create Docker client: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Docker error"})),
            );
        }
    };

    // Stop and remove containers
    for slot in [Slot::Blue, Slot::Green] {
        let container_id = match slot {
            Slot::Blue => &project.blue_container_id,
            Slot::Green => &project.green_container_id,
        };

        if let Some(cid) = container_id {
            info!("Stopping container {} for project {}", cid, project.name);
            if let Err(e) = docker.stop_container(cid).await {
                warn!("Failed to stop container {}: {}", cid, e);
            }
        }
    }

    // Remove directories
    let workspace_path = PathBuf::from("/workspace/projects").join(&project.name);
    let output_path = PathBuf::from("/workspace/outputs").join(&project.name);
    let cache_path = PathBuf::from("/workspace/cache").join(&project.name);

    for path in [workspace_path, output_path, cache_path] {
        if path.exists() {
            info!("Removing directory: {:?}", path);
            if let Err(e) = fs::remove_dir_all(&path).await {
                warn!("Failed to remove directory {:?}: {}", path, e);
            }
        }
    }

    // Delete from database
    if let Err(e) = state.db.delete_project(id).await {
        warn!("Failed to delete project from database: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "Failed to delete project"})),
        );
    }

    info!("Project {} deleted successfully", project.name);

    (
        StatusCode::OK,
        Json(serde_json::json!({"message": "Project deleted successfully"})),
    )
}
