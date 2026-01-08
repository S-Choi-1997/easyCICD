use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Json, Router,
};
use serde::Deserialize;
use tracing::warn;

use crate::state::AppState;

pub fn builds_routes() -> Router<AppState> {
    Router::new()
        .route("/", get(list_builds))
        .route("/{id}", get(get_build))
        .route("/{id}/logs", get(get_build_logs))
}

#[derive(Deserialize)]
struct ListBuildsQuery {
    project_id: Option<i64>,
    limit: Option<i64>,
}

async fn list_builds(
    State(state): State<AppState>,
    Query(params): Query<ListBuildsQuery>,
) -> impl IntoResponse {
    let limit = params.limit.unwrap_or(50);

    let builds = if let Some(project_id) = params.project_id {
        state.db.list_builds_by_project(project_id, limit).await
    } else {
        state.db.list_recent_builds(limit).await
    };

    match builds {
        Ok(builds) => (StatusCode::OK, Json(builds)),
        Err(e) => {
            warn!("Failed to list builds: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(vec![]))
        }
    }
}

async fn get_build(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.db.get_build(id).await {
        Ok(Some(build)) => (StatusCode::OK, Json(Some(build))),
        Ok(None) => (StatusCode::NOT_FOUND, Json(None)),
        Err(e) => {
            warn!("Failed to get build: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, Json(None))
        }
    }
}

async fn get_build_logs(
    State(state): State<AppState>,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    match state.db.get_build(id).await {
        Ok(Some(build)) => {
            // Read log file
            match tokio::fs::read_to_string(&build.log_path).await {
                Ok(content) => (StatusCode::OK, content),
                Err(_) => (StatusCode::NOT_FOUND, String::from("Log file not found")),
            }
        }
        Ok(None) => (StatusCode::NOT_FOUND, String::from("Build not found")),
        Err(e) => {
            warn!("Failed to get build logs: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, format!("Error: {}", e))
        }
    }
}
