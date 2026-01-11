use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use tracing::warn;

use crate::state::AppContext;
use crate::infrastructure::logging::{TraceContext, Timer};
use crate::application::ports::repositories::BuildRepository;

pub fn builds_routes() -> Router<AppContext> {
    Router::new()
        .route("/", get(list_builds))
        .route("/{id}", get(get_build))
        .route("/{id}/logs", get(get_build_logs))
        .route("/{id}/build-logs", get(get_build_logs_only))
        .route("/{id}/deploy-logs", get(get_deploy_logs))
}

#[derive(Deserialize)]
struct ListBuildsQuery {
    project_id: Option<i64>,
    limit: Option<i64>,
}

async fn list_builds(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Query(params): Query<ListBuildsQuery>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();
    let limit = params.limit.unwrap_or(50);

    ctx.logger.api_entry(&trace_id, "GET", "/api/builds", &format!("limit={}", limit));

    let builds = if let Some(project_id) = params.project_id {
        ctx.build_repo.list_by_project(project_id, limit).await
    } else {
        ctx.build_repo.list_recent(limit).await
    };

    match builds {
        Ok(builds) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/builds", timer.elapsed_ms(), 200);
            (StatusCode::OK, Json(builds))
        }
        Err(e) => {
            warn!("[{}] Failed to list builds: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "GET", "/api/builds", timer.elapsed_ms(), 500);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(vec![]))
        }
    }
}

async fn get_build(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", &format!("/api/builds/{}", id), "");

    match ctx.build_repo.get(id).await {
        Ok(Some(build)) => {
            ctx.logger.api_exit(&trace_id, "GET", &format!("/api/builds/{}", id), timer.elapsed_ms(), 200);
            (StatusCode::OK, Json(Some(build)))
        }
        Ok(None) => {
            ctx.logger.api_exit(&trace_id, "GET", &format!("/api/builds/{}", id), timer.elapsed_ms(), 404);
            (StatusCode::NOT_FOUND, Json(None))
        }
        Err(e) => {
            warn!("[{}] Failed to get build: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "GET", &format!("/api/builds/{}", id), timer.elapsed_ms(), 500);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(None))
        }
    }
}

async fn get_build_logs(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", &format!("/api/builds/{}/logs", id), "");

    match ctx.build_repo.get(id).await {
        Ok(Some(build)) => {
            // Read log file (backward compatibility: only build logs)
            match tokio::fs::read_to_string(&build.log_path).await {
                Ok(content) => {
                    ctx.logger.api_exit(&trace_id, "GET", &format!("/api/builds/{}/logs", id), timer.elapsed_ms(), 200);
                    (StatusCode::OK, content)
                }
                Err(_) => {
                    ctx.logger.api_exit(&trace_id, "GET", &format!("/api/builds/{}/logs", id), timer.elapsed_ms(), 404);
                    (StatusCode::NOT_FOUND, String::from("Log file not found"))
                }
            }
        }
        Ok(None) => {
            ctx.logger.api_exit(&trace_id, "GET", &format!("/api/builds/{}/logs", id), timer.elapsed_ms(), 404);
            (StatusCode::NOT_FOUND, String::from("Build not found"))
        }
        Err(e) => {
            warn!("[{}] Failed to get build logs: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "GET", &format!("/api/builds/{}/logs", id), timer.elapsed_ms(), 500);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", e))
        }
    }
}

async fn get_build_logs_only(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", &format!("/api/builds/{}/build-logs", id), "");

    match ctx.build_repo.get(id).await {
        Ok(Some(build)) => {
            // Read build log file only
            match tokio::fs::read_to_string(&build.log_path).await {
                Ok(content) => {
                    ctx.logger.api_exit(&trace_id, "GET", &format!("/api/builds/{}/build-logs", id), timer.elapsed_ms(), 200);
                    (StatusCode::OK, content)
                }
                Err(_) => {
                    ctx.logger.api_exit(&trace_id, "GET", &format!("/api/builds/{}/build-logs", id), timer.elapsed_ms(), 404);
                    (StatusCode::NOT_FOUND, String::from("Build log file not found"))
                }
            }
        }
        Ok(None) => {
            ctx.logger.api_exit(&trace_id, "GET", &format!("/api/builds/{}/build-logs", id), timer.elapsed_ms(), 404);
            (StatusCode::NOT_FOUND, String::from("Build not found"))
        }
        Err(e) => {
            warn!("[{}] Failed to get build logs: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "GET", &format!("/api/builds/{}/build-logs", id), timer.elapsed_ms(), 500);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", e))
        }
    }
}

async fn get_deploy_logs(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", &format!("/api/builds/{}/deploy-logs", id), "");

    match ctx.build_repo.get(id).await {
        Ok(Some(build)) => {
            // Read deploy log file
            if let Some(deploy_log_path) = &build.deploy_log_path {
                match tokio::fs::read_to_string(deploy_log_path).await {
                    Ok(content) => {
                        ctx.logger.api_exit(&trace_id, "GET", &format!("/api/builds/{}/deploy-logs", id), timer.elapsed_ms(), 200);
                        (StatusCode::OK, content)
                    }
                    Err(_) => {
                        ctx.logger.api_exit(&trace_id, "GET", &format!("/api/builds/{}/deploy-logs", id), timer.elapsed_ms(), 200);
                        (StatusCode::OK, String::from("")) // Return empty if no deploy logs yet
                    }
                }
            } else {
                ctx.logger.api_exit(&trace_id, "GET", &format!("/api/builds/{}/deploy-logs", id), timer.elapsed_ms(), 200);
                (StatusCode::OK, String::from("")) // No deploy log path configured
            }
        }
        Ok(None) => {
            ctx.logger.api_exit(&trace_id, "GET", &format!("/api/builds/{}/deploy-logs", id), timer.elapsed_ms(), 404);
            (StatusCode::NOT_FOUND, String::from("Build not found"))
        }
        Err(e) => {
            warn!("[{}] Failed to get deploy logs: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "GET", &format!("/api/builds/{}/deploy-logs", id), timer.elapsed_ms(), 500);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", e))
        }
    }
}
