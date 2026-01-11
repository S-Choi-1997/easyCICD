use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
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
use crate::state::AppContext;
use crate::infrastructure::logging::{TraceContext, Timer};
use crate::application::ports::repositories::{ProjectRepository, BuildRepository};

pub fn projects_routes() -> Router<AppContext> {
    Router::new()
        .route("/", get(list_projects).post(create_project))
        .route("/{id}", get(get_project).delete(delete_project))
        .route("/{id}/builds", post(trigger_build))
        .route("/{id}/rollback/{build_id}", post(rollback_build))
        .route("/{id}/runtime-logs", get(runtime_logs))
        .route("/{id}/containers/start", post(start_containers))
        .route("/{id}/containers/stop", post(stop_containers))
        .route("/{id}/containers/restart", post(restart_containers))
}

async fn list_projects(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", "/api/projects", "");

    match ctx.project_repo.list().await {
        Ok(projects) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/projects", timer.elapsed_ms(), 200);
            (StatusCode::OK, Json(projects))
        }
        Err(e) => {
            warn!("[{}] Failed to list projects: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "GET", "/api/projects", timer.elapsed_ms(), 500);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(vec![]))
        }
    }
}

async fn get_project(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", &format!("/api/projects/{}", id), "");

    match ctx.project_repo.get(id).await {
        Ok(Some(project)) => {
            ctx.logger.api_exit(&trace_id, "GET", &format!("/api/projects/{}", id), timer.elapsed_ms(), 200);
            (StatusCode::OK, Json(Some(project)))
        }
        Ok(None) => {
            ctx.logger.api_exit(&trace_id, "GET", &format!("/api/projects/{}", id), timer.elapsed_ms(), 404);
            (StatusCode::NOT_FOUND, Json(None))
        }
        Err(e) => {
            warn!("[{}] Failed to get project: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "GET", &format!("/api/projects/{}", id), timer.elapsed_ms(), 500);
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
    working_directory: Option<String>,
    runtime_image: String,
    runtime_command: String,
    health_check_url: String,
    runtime_port: i32,
}

async fn create_project(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(req): Json<CreateProjectRequest>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "POST", "/api/projects", &format!("name={}", req.name));

    let create_project = CreateProject {
        name: req.name,
        repo: req.repo,
        path_filter: req.path_filter,
        branch: req.branch,
        build_image: req.build_image,
        build_command: req.build_command,
        cache_type: req.cache_type,
        working_directory: req.working_directory,
        runtime_image: req.runtime_image,
        runtime_command: req.runtime_command,
        health_check_url: req.health_check_url,
        runtime_port: req.runtime_port,
    };

    match ctx.project_repo.create(create_project).await {
        Ok(project) => {
            ctx.logger.api_exit(&trace_id, "POST", "/api/projects", timer.elapsed_ms(), 201);
            (StatusCode::CREATED, Json(Some(project)))
        }
        Err(e) => {
            warn!("[{}] Failed to create project: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "POST", "/api/projects", timer.elapsed_ms(), 500);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(None))
        }
    }
}


async fn trigger_build(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "POST", &format!("/api/projects/{}/builds", id), &format!("project_id={}", id));

    // Get project
    let project = match ctx.project_repo.get(id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            ctx.logger.api_exit(&trace_id, "POST", &format!("/api/projects/{}/builds", id), timer.elapsed_ms(), 404);
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            )
        }
        Err(e) => {
            warn!("[{}] Failed to get project: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "POST", &format!("/api/projects/{}/builds", id), timer.elapsed_ms(), 500);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Database error"})),
            );
        }
    };

    // Get current commit hash from workspace
    let workspace_path = PathBuf::from("/data/workspace")
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

    let build = match ctx.build_repo.create(create_build).await {
        Ok(b) => b,
        Err(e) => {
            warn!("[{}] Failed to create build: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "POST", &format!("/api/projects/{}/builds", id), timer.elapsed_ms(), 500);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to create build"})),
            );
        }
    };

    // Enqueue build
    ctx.build_queue.enqueue(project.id, build.id).await;

    ctx.logger.api_exit(&trace_id, "POST", &format!("/api/projects/{}/builds", id), timer.elapsed_ms(), 201);

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
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "DELETE", &format!("/api/projects/{}", id), &format!("project_id={}", id));

    // Get project first
    let project = match ctx.project_repo.get(id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            ctx.logger.api_exit(&trace_id, "DELETE", &format!("/api/projects/{}", id), timer.elapsed_ms(), 404);
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            )
        }
        Err(e) => {
            warn!("[{}] Failed to get project: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "DELETE", &format!("/api/projects/{}", id), timer.elapsed_ms(), 500);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Database error"})),
            );
        }
    };

    // Stop and remove containers using context's docker
    for slot in [Slot::Blue, Slot::Green] {
        let container_id = match slot {
            Slot::Blue => &project.blue_container_id,
            Slot::Green => &project.green_container_id,
        };

        // Try to stop by container ID first
        if let Some(cid) = container_id {
            info!("[{}] Stopping container {} for project {}", trace_id, cid, project.name);
            if let Err(e) = ctx.docker.stop_container(cid).await {
                warn!("[{}] Failed to stop container {}: {}", trace_id, cid, e);
            }
        }

        // Also try to stop by container name (in case container_id is not in DB)
        let container_name = match slot {
            Slot::Blue => format!("project-{}-blue", project.id),
            Slot::Green => format!("project-{}-green", project.id),
        };

        info!("[{}] Attempting to stop container by name: {}", trace_id, container_name);
        if let Err(e) = ctx.docker.stop_container(&container_name).await {
            // This is expected to fail if container doesn't exist, so just debug log
            info!("[{}] Container {} not found or already stopped", trace_id, container_name);
        }
    }

    // Remove directories
    let workspace_path = PathBuf::from("/data/workspace").join(&project.name);
    let output_base = PathBuf::from("/data/output");
    let cache_path = PathBuf::from("/data/cache").join(&project.cache_type);
    let logs_path = PathBuf::from("/data/easycicd/logs").join(&project.name);

    for path in [workspace_path, cache_path, logs_path] {
        if path.exists() {
            info!("[{}] Removing directory: {:?}", trace_id, path);
            if let Err(e) = fs::remove_dir_all(&path).await {
                warn!("[{}] Failed to remove directory {:?}: {}", trace_id, path, e);
            }
        }
    }

    // Remove all build output directories for this project
    if output_base.exists() {
        if let Ok(mut entries) = fs::read_dir(&output_base).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                // Remove all build directories (they will be cascade deleted from DB anyway)
                if entry.path().is_dir() {
                    if let Err(e) = fs::remove_dir_all(entry.path()).await {
                        warn!("[{}] Failed to remove build output {:?}: {}", trace_id, entry.path(), e);
                    }
                }
            }
        }
    }

    // Delete from database (this will cascade delete builds due to ON DELETE CASCADE)
    if let Err(e) = ctx.project_repo.delete(id).await {
        warn!("[{}] Failed to delete project from database: {}", trace_id, e);
        ctx.logger.api_exit(&trace_id, "DELETE", &format!("/api/projects/{}", id), timer.elapsed_ms(), 500);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": "Failed to delete project"})),
        );
    }

    info!("[{}] Project {} deleted successfully", trace_id, project.name);
    ctx.logger.api_exit(&trace_id, "DELETE", &format!("/api/projects/{}", id), timer.elapsed_ms(), 200);

    (
        StatusCode::OK,
        Json(serde_json::json!({"message": "Project deleted successfully"})),
    )
}

async fn start_containers(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "POST", &format!("/api/projects/{}/containers/start", id), &format!("project_id={}", id));

    // Get project
    let project = match ctx.project_repo.get(id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            ctx.logger.api_exit(&trace_id, "POST", &format!("/api/projects/{}/containers/start", id), timer.elapsed_ms(), 404);
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            )
        }
        Err(e) => {
            warn!("[{}] Failed to get project: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "POST", &format!("/api/projects/{}/containers/start", id), timer.elapsed_ms(), 500);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Database error"})),
            );
        }
    };

    let mut results = Vec::new();

    // Start both containers
    for slot in [Slot::Blue, Slot::Green] {
        let container_name = match slot {
            Slot::Blue => format!("project-{}-blue", project.id),
            Slot::Green => format!("project-{}-green", project.id),
        };

        match ctx.docker.start_container(&container_name).await {
            Ok(_) => {
                info!("[{}] Started container: {}", trace_id, container_name);
                results.push(serde_json::json!({
                    "slot": format!("{:?}", slot).to_lowercase(),
                    "status": "started",
                    "container": container_name
                }));
            }
            Err(e) => {
                warn!("[{}] Failed to start container {}: {}", trace_id, container_name, e);
                let error_msg: String = format!("{}", e);
                results.push(serde_json::json!({
                    "slot": format!("{:?}", slot).to_lowercase(),
                    "status": "error",
                    "error": error_msg
                }));
            }
        }
    }

    ctx.logger.api_exit(&trace_id, "POST", &format!("/api/projects/{}/containers/start", id), timer.elapsed_ms(), 200);
    (StatusCode::OK, Json(serde_json::json!({ "results": results })))
}

async fn stop_containers(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "POST", &format!("/api/projects/{}/containers/stop", id), &format!("project_id={}", id));

    // Get project
    let project = match ctx.project_repo.get(id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            ctx.logger.api_exit(&trace_id, "POST", &format!("/api/projects/{}/containers/stop", id), timer.elapsed_ms(), 404);
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            )
        }
        Err(e) => {
            warn!("[{}] Failed to get project: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "POST", &format!("/api/projects/{}/containers/stop", id), timer.elapsed_ms(), 500);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Database error"})),
            );
        }
    };

    let mut results = Vec::new();

    // Stop both containers
    for slot in [Slot::Blue, Slot::Green] {
        let container_name = match slot {
            Slot::Blue => format!("project-{}-blue", project.id),
            Slot::Green => format!("project-{}-green", project.id),
        };

        match ctx.docker.stop_container(&container_name).await {
            Ok(_) => {
                info!("[{}] Stopped container: {}", trace_id, container_name);
                results.push(serde_json::json!({
                    "slot": format!("{:?}", slot).to_lowercase(),
                    "status": "stopped",
                    "container": container_name
                }));
            }
            Err(e) => {
                warn!("[{}] Failed to stop container {}: {}", trace_id, container_name, e);
                let error_msg: String = format!("{}", e);
                results.push(serde_json::json!({
                    "slot": format!("{:?}", slot).to_lowercase(),
                    "status": "error",
                    "error": error_msg
                }));
            }
        }
    }

    ctx.logger.api_exit(&trace_id, "POST", &format!("/api/projects/{}/containers/stop", id), timer.elapsed_ms(), 200);
    (StatusCode::OK, Json(serde_json::json!({ "results": results })))
}

async fn restart_containers(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "POST", &format!("/api/projects/{}/containers/restart", id), &format!("project_id={}", id));

    // Get project
    let project = match ctx.project_repo.get(id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            ctx.logger.api_exit(&trace_id, "POST", &format!("/api/projects/{}/containers/restart", id), timer.elapsed_ms(), 404);
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            )
        }
        Err(e) => {
            warn!("[{}] Failed to get project: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "POST", &format!("/api/projects/{}/containers/restart", id), timer.elapsed_ms(), 500);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Database error"})),
            );
        }
    };

    let mut results = Vec::new();

    // Restart both containers
    for slot in [Slot::Blue, Slot::Green] {
        let container_name = match slot {
            Slot::Blue => format!("project-{}-blue", project.id),
            Slot::Green => format!("project-{}-green", project.id),
        };

        match ctx.docker.restart_container(&container_name).await {
            Ok(_) => {
                info!("[{}] Restarted container: {}", trace_id, container_name);
                results.push(serde_json::json!({
                    "slot": format!("{:?}", slot).to_lowercase(),
                    "status": "restarted",
                    "container": container_name
                }));
            }
            Err(e) => {
                warn!("[{}] Failed to restart container {}: {}", trace_id, container_name, e);
                let error_msg: String = format!("{}", e);
                results.push(serde_json::json!({
                    "slot": format!("{:?}", slot).to_lowercase(),
                    "status": "error",
                    "error": error_msg
                }));
            }
        }
    }

    ctx.logger.api_exit(&trace_id, "POST", &format!("/api/projects/{}/containers/restart", id), timer.elapsed_ms(), 200);
    (StatusCode::OK, Json(serde_json::json!({ "results": results })))
}

async fn rollback_build(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path((project_id, build_id)): Path<(i64, i64)>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "POST", &format!("/api/projects/{}/rollback/{}", project_id, build_id), &format!("project_id={}, build_id={}", project_id, build_id));

    // Get project
    let project = match ctx.project_repo.get(project_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            ctx.logger.api_exit(&trace_id, "POST", &format!("/api/projects/{}/rollback/{}", project_id, build_id), timer.elapsed_ms(), 404);
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Project not found"})),
            )
        }
        Err(e) => {
            warn!("[{}] Failed to get project: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "POST", &format!("/api/projects/{}/rollback/{}", project_id, build_id), timer.elapsed_ms(), 500);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Database error"})),
            );
        }
    };

    // Get target build
    let target_build = match ctx.build_repo.get(build_id).await {
        Ok(Some(b)) => b,
        Ok(None) => {
            ctx.logger.api_exit(&trace_id, "POST", &format!("/api/projects/{}/rollback/{}", project_id, build_id), timer.elapsed_ms(), 404);
            return (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Build not found"})),
            )
        }
        Err(e) => {
            warn!("[{}] Failed to get build: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "POST", &format!("/api/projects/{}/rollback/{}", project_id, build_id), timer.elapsed_ms(), 500);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Database error"})),
            );
        }
    };

    // Verify build belongs to project
    if target_build.project_id != project_id {
        ctx.logger.api_exit(&trace_id, "POST", &format!("/api/projects/{}/rollback/{}", project_id, build_id), timer.elapsed_ms(), 400);
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Build does not belong to this project"})),
        );
    }

    // Execute rollback via DeploymentService
    match ctx.deployment_service.rollback(&trace_id, &project, &target_build).await {
        Ok(_) => {
            ctx.logger.api_exit(&trace_id, "POST", &format!("/api/projects/{}/rollback/{}", project_id, build_id), timer.elapsed_ms(), 200);
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "message": "Rollback completed successfully",
                    "build_id": build_id,
                    "build_number": target_build.build_number
                })),
            )
        }
        Err(e) => {
            warn!("[{}] Rollback failed: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "POST", &format!("/api/projects/{}/rollback/{}", project_id, build_id), timer.elapsed_ms(), 500);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Rollback failed: {}", e)})),
            )
        }
    }
}

use axum::extract::ws::{WebSocket, WebSocketUpgrade};
use axum::response::Response;
use futures_util::{SinkExt, StreamExt};

async fn runtime_logs(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(project_id): Path<i64>,
    ws: WebSocketUpgrade,
) -> Response {
    let trace_id = TraceContext::extract_or_generate(&headers);

    ctx.logger.api_entry(&trace_id, "GET", &format!("/api/projects/{}/runtime-logs", project_id), &format!("project_id={}", project_id));

    ws.on_upgrade(move |socket| runtime_logs_stream(socket, ctx, trace_id, project_id))
}

async fn runtime_logs_stream(
    mut socket: WebSocket,
    ctx: AppContext,
    trace_id: String,
    project_id: i64,
) {
    info!("[{}] WebSocket connected for runtime logs of project {}", trace_id, project_id);

    // Get project
    let project = match ctx.project_repo.get(project_id).await {
        Ok(Some(p)) => p,
        Ok(None) => {
            let _ = socket.send(axum::extract::ws::Message::Text(
                serde_json::json!({"error": "Project not found"}).to_string().into()
            )).await;
            return;
        }
        Err(e) => {
            warn!("[{}] Failed to get project: {}", trace_id, e);
            let _ = socket.send(axum::extract::ws::Message::Text(
                serde_json::json!({"error": "Database error"}).to_string().into()
            )).await;
            return;
        }
    };

    // Get active container ID
    let container_id = match project.active_slot {
        Slot::Blue => project.blue_container_id,
        Slot::Green => project.green_container_id,
    };

    let container_id = match container_id {
        Some(id) => id,
        None => {
            let _ = socket.send(axum::extract::ws::Message::Text(
                serde_json::json!({"error": "No active container"}).to_string().into()
            )).await;
            return;
        }
    };

    info!("[{}] Streaming logs from container: {}", trace_id, container_id);

    // Stream Docker logs
    match ctx.docker.stream_container_logs(&container_id).await {
        Ok(mut stream) => {
            let (mut ws_sender, mut ws_receiver) = socket.split();

            loop {
                tokio::select! {
                    // Docker 로그 수신
                    log_chunk = stream.next() => {
                        match log_chunk {
                            Some(Ok(chunk)) => {
                                let log_text = String::from_utf8_lossy(&chunk).to_string();
                                if ws_sender.send(axum::extract::ws::Message::Text(log_text.into())).await.is_err() {
                                    break;
                                }
                            }
                            Some(Err(e)) => {
                                warn!("[{}] Error streaming logs: {}", trace_id, e);
                                break;
                            }
                            None => {
                                info!("[{}] Log stream ended", trace_id);
                                break;
                            }
                        }
                    }
                    // WebSocket close 감지
                    msg = ws_receiver.next() => {
                        if msg.is_none() {
                            info!("[{}] WebSocket closed by client", trace_id);
                            break;
                        }
                    }
                }
            }
        }
        Err(e) => {
            warn!("[{}] Failed to stream container logs: {}", trace_id, e);
            let _ = socket.send(axum::extract::ws::Message::Text(
                serde_json::json!({"error": format!("Failed to stream logs: {}", e)}).to_string().into()
            )).await;
        }
    }

    info!("[{}] Runtime logs stream ended for project {}", trace_id, project_id);
}
