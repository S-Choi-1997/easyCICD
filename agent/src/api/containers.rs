use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::{get, post, delete},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tracing::error;

use crate::state::AppContext;
use crate::infrastructure::logging::{TraceContext, Timer};
use crate::db::models::CreateContainer;

pub fn containers_routes() -> Router<AppContext> {
    Router::new()
        .route("/", get(list_containers).post(create_container))
        .route("/{id}", get(get_container).delete(delete_container))
        .route("/{id}/start", post(start_container))
        .route("/{id}/stop", post(stop_container))
        .route("/{id}/logs", get(get_logs))
}

#[derive(Debug, Deserialize)]
pub struct CreateContainerRequest {
    pub name: String,
    pub image: String,
    pub container_port: i32,
    pub env_vars: Option<serde_json::Value>,
    pub command: Option<String>,
    pub persist_data: Option<bool>,
}

#[derive(Debug, Serialize)]
pub struct ContainerResponse {
    pub id: i64,
    pub name: String,
    pub container_id: Option<String>,
    pub port: i32,
    pub container_port: Option<i32>,
    pub image: String,
    pub env_vars: Option<serde_json::Value>,
    pub command: Option<String>,
    pub persist_data: bool,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<crate::db::models::Container> for ContainerResponse {
    fn from(c: crate::db::models::Container) -> Self {
        Self {
            id: c.id,
            name: c.name,
            container_id: c.container_id,
            port: c.port,
            container_port: c.container_port,
            image: c.image,
            env_vars: c.env_vars.and_then(|s| serde_json::from_str(&s).ok()),
            command: c.command,
            persist_data: c.persist_data != 0,
            status: c.status.to_string(),
            created_at: c.created_at,
            updated_at: c.updated_at,
        }
    }
}

/// GET /api/containers
async fn list_containers(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();
    ctx.logger.api_entry(&trace_id, "GET", "/api/containers", "");

    match ctx.container_service.list_containers(&trace_id).await {
        Ok(containers) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/containers", timer.elapsed_ms(), 200);
            let response: Vec<ContainerResponse> = containers.into_iter().map(|c| c.into()).collect();
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            error!("[{}] Failed to list containers: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "GET", "/api/containers", timer.elapsed_ms(), 500);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// POST /api/containers
async fn create_container(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(req): Json<CreateContainerRequest>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();
    ctx.logger.api_entry(&trace_id, "POST", "/api/containers", &req.name);

    let create_req = CreateContainer {
        name: req.name,
        image: req.image,
        container_port: req.container_port,
        env_vars: req.env_vars.map(|v| v.to_string()),
        command: req.command,
        persist_data: req.persist_data.unwrap_or(false),
    };

    match ctx.container_service.create_container(&trace_id, create_req).await {
        Ok(container) => {
            ctx.logger.api_exit(&trace_id, "POST", "/api/containers", timer.elapsed_ms(), 201);
            let response: ContainerResponse = container.into();
            (StatusCode::CREATED, Json(response)).into_response()
        }
        Err(e) => {
            error!("[{}] Failed to create container: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "POST", "/api/containers", timer.elapsed_ms(), 500);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// GET /api/containers/:id
async fn get_container(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();
    ctx.logger.api_entry(&trace_id, "GET", "/api/containers/:id", &id.to_string());

    match ctx.container_service.get_container(&trace_id, id).await {
        Ok(Some(container)) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/containers/:id", timer.elapsed_ms(), 200);
            let response: ContainerResponse = container.into();
            (StatusCode::OK, Json(response)).into_response()
        }
        Ok(None) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/containers/:id", timer.elapsed_ms(), 404);
            (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Container not found"}))).into_response()
        }
        Err(e) => {
            error!("[{}] Failed to get container: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "GET", "/api/containers/:id", timer.elapsed_ms(), 500);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// DELETE /api/containers/:id
async fn delete_container(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();
    ctx.logger.api_entry(&trace_id, "DELETE", "/api/containers/:id", &id.to_string());

    match ctx.container_service.delete_container(&trace_id, id).await {
        Ok(()) => {
            ctx.logger.api_exit(&trace_id, "DELETE", "/api/containers/:id", timer.elapsed_ms(), 204);
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            error!("[{}] Failed to delete container: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "DELETE", "/api/containers/:id", timer.elapsed_ms(), 500);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// POST /api/containers/:id/start
async fn start_container(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();
    ctx.logger.api_entry(&trace_id, "POST", "/api/containers/:id/start", &id.to_string());

    match ctx.container_service.start_container(&trace_id, id).await {
        Ok(container) => {
            ctx.logger.api_exit(&trace_id, "POST", "/api/containers/:id/start", timer.elapsed_ms(), 200);
            let response: ContainerResponse = container.into();
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            error!("[{}] Failed to start container: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "POST", "/api/containers/:id/start", timer.elapsed_ms(), 500);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// POST /api/containers/:id/stop
async fn stop_container(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();
    ctx.logger.api_entry(&trace_id, "POST", "/api/containers/:id/stop", &id.to_string());

    match ctx.container_service.stop_container(&trace_id, id).await {
        Ok(container) => {
            ctx.logger.api_exit(&trace_id, "POST", "/api/containers/:id/stop", timer.elapsed_ms(), 200);
            let response: ContainerResponse = container.into();
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(e) => {
            error!("[{}] Failed to stop container: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "POST", "/api/containers/:id/stop", timer.elapsed_ms(), 500);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}

/// GET /api/containers/:id/logs
async fn get_logs(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();
    ctx.logger.api_entry(&trace_id, "GET", "/api/containers/:id/logs", &id.to_string());

    match ctx.container_service.get_logs(&trace_id, id, Some(200)).await {
        Ok(logs) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/containers/:id/logs", timer.elapsed_ms(), 200);
            (StatusCode::OK, Json(serde_json::json!({"logs": logs}))).into_response()
        }
        Err(e) => {
            error!("[{}] Failed to get container logs: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "GET", "/api/containers/:id/logs", timer.elapsed_ms(), 500);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": e.to_string()}))).into_response()
        }
    }
}
