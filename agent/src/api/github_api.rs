use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};
use tracing::warn;

use crate::github::{GitHubClient, ProjectDetector};
use crate::state::AppContext;
use crate::infrastructure::logging::{TraceContext, Timer};
use crate::application::ports::repositories::SettingsRepository;

#[derive(Debug, Deserialize)]
pub struct SetPATRequest {
    pub github_pat: String,
}

/// Set GitHub PAT (global configuration)
pub async fn set_github_pat(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(payload): Json<SetPATRequest>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "POST", "/api/github/pat", "set_pat");

    // Validate PAT by getting user info
    let client = GitHubClient::new(payload.github_pat.clone());

    let github_user = match client.get_user().await {
        Ok(user) => user,
        Err(e) => {
            warn!("[{}] Invalid GitHub PAT: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "POST", "/api/github/pat", timer.elapsed_ms(), 400);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Invalid GitHub PAT: {}", e)
                })),
            );
        }
    };

    // Save PAT to settings
    if let Err(e) = ctx.settings_repo.set("github_pat", &payload.github_pat).await {
        warn!("[{}] Failed to save PAT: {}", trace_id, e);
        ctx.logger.api_exit(&trace_id, "POST", "/api/github/pat", timer.elapsed_ms(), 500);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to save PAT: {}", e)
            })),
        );
    }

    ctx.logger.api_exit(&trace_id, "POST", "/api/github/pat", timer.elapsed_ms(), 200);
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "github_username": github_user.login
        })),
    )
}

/// Get current GitHub PAT status
pub async fn get_github_pat_status(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", "/api/github/pat", "");

    match ctx.settings_repo.get("github_pat").await {
        Ok(Some(pat)) => {
            // Validate PAT
            let client = GitHubClient::new(pat);
            match client.get_user().await {
                Ok(user) => {
                    ctx.logger.api_exit(&trace_id, "GET", "/api/github/pat", timer.elapsed_ms(), 200);
                    (
                        StatusCode::OK,
                        Json(serde_json::json!({
                            "configured": true,
                            "github_username": user.login
                        })),
                    )
                }
                Err(e) => {
                    warn!("[{}] PAT validation failed: {}", trace_id, e);
                    ctx.logger.api_exit(&trace_id, "GET", "/api/github/pat", timer.elapsed_ms(), 200);
                    (
                        StatusCode::OK,
                        Json(serde_json::json!({
                            "configured": false,
                            "error": "PAT is invalid or expired"
                        })),
                    )
                }
            }
        }
        _ => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/github/pat", timer.elapsed_ms(), 200);
            (
                StatusCode::OK,
                Json(serde_json::json!({
                    "configured": false
                })),
            )
        }
    }
}

/// Delete GitHub PAT (sign out)
pub async fn delete_github_pat(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "DELETE", "/api/github/pat", "");

    if let Err(e) = ctx.settings_repo.delete("github_pat").await {
        warn!("[{}] Failed to delete PAT: {}", trace_id, e);
        ctx.logger.api_exit(&trace_id, "DELETE", "/api/github/pat", timer.elapsed_ms(), 500);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to delete PAT: {}", e)
            })),
        );
    }

    ctx.logger.api_exit(&trace_id, "DELETE", "/api/github/pat", timer.elapsed_ms(), 200);
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "message": "GitHub PAT deleted successfully"
        })),
    )
}

/// List user's GitHub repositories
pub async fn list_repositories(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", "/api/github/repositories", "");

    let pat = match ctx.settings_repo.get("github_pat").await {
        Ok(Some(pat)) => pat,
        _ => {
            warn!("[{}] No GitHub PAT configured", trace_id);
            ctx.logger.api_exit(&trace_id, "GET", "/api/github/repositories", timer.elapsed_ms(), 400);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "No GitHub PAT configured. Please set your PAT first.",
                    "repositories": []
                })),
            );
        }
    };

    let client = GitHubClient::new(pat);

    match client.list_repositories().await {
        Ok(repos) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/github/repositories", timer.elapsed_ms(), 200);
            (StatusCode::OK, Json(serde_json::json!({ "repositories": repos })))
        }
        Err(e) => {
            warn!("[{}] Failed to fetch repositories: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "GET", "/api/github/repositories", timer.elapsed_ms(), 500);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Failed to fetch repositories: {}", e),
                    "repositories": []
                })),
            )
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct BranchesQuery {
    pub owner: String,
    pub repo: String,
}

/// List repository branches
pub async fn list_branches(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Query(params): Query<BranchesQuery>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", "/api/github/branches", &format!("{}/{}", params.owner, params.repo));

    let pat = match ctx.settings_repo.get("github_pat").await {
        Ok(Some(pat)) => pat,
        _ => {
            warn!("[{}] No GitHub PAT configured", trace_id);
            ctx.logger.api_exit(&trace_id, "GET", "/api/github/branches", timer.elapsed_ms(), 400);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "No GitHub PAT configured",
                    "branches": []
                })),
            );
        }
    };

    let client = GitHubClient::new(pat);

    match client.list_branches(&params.owner, &params.repo).await {
        Ok(branches) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/github/branches", timer.elapsed_ms(), 200);
            (StatusCode::OK, Json(serde_json::json!({ "branches": branches })))
        }
        Err(e) => {
            warn!("[{}] Failed to fetch branches: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "GET", "/api/github/branches", timer.elapsed_ms(), 500);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Failed to fetch branches: {}", e),
                    "branches": []
                })),
            )
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct FoldersQuery {
    pub owner: String,
    pub repo: String,
    pub sha: String,
}

/// List repository folders
pub async fn list_folders(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Query(params): Query<FoldersQuery>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", "/api/github/folders", &format!("{}/{}/{}", params.owner, params.repo, params.sha));

    let pat = match ctx.settings_repo.get("github_pat").await {
        Ok(Some(pat)) => pat,
        _ => {
            warn!("[{}] No GitHub PAT configured", trace_id);
            ctx.logger.api_exit(&trace_id, "GET", "/api/github/folders", timer.elapsed_ms(), 400);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "No GitHub PAT configured",
                    "folders": []
                })),
            );
        }
    };

    let client = GitHubClient::new(pat);

    match client.get_tree(&params.owner, &params.repo, &params.sha).await {
        Ok(tree) => {
            // Filter only directories
            let folders: Vec<String> = tree
                .tree
                .iter()
                .filter(|item| item.item_type == "tree")
                .map(|item| item.path.clone())
                .collect();

            ctx.logger.api_exit(&trace_id, "GET", "/api/github/folders", timer.elapsed_ms(), 200);
            (StatusCode::OK, Json(serde_json::json!({ "folders": folders })))
        }
        Err(e) => {
            warn!("[{}] Failed to fetch folders: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "GET", "/api/github/folders", timer.elapsed_ms(), 500);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Failed to fetch folders: {}", e),
                    "folders": []
                })),
            )
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct DetectProjectQuery {
    pub owner: String,
    pub repo: String,
    pub branch: String,
    pub path_filter: Option<String>,
    pub workflow_path: Option<String>,
}

/// Detect project type and generate configuration
pub async fn detect_project(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Query(params): Query<DetectProjectQuery>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", "/api/github/detect", &format!("{}/{}/{}", params.owner, params.repo, params.branch));

    let pat = match ctx.settings_repo.get("github_pat").await {
        Ok(Some(pat)) => pat,
        _ => {
            warn!("[{}] No GitHub PAT configured", trace_id);
            ctx.logger.api_exit(&trace_id, "GET", "/api/github/detect", timer.elapsed_ms(), 400);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": "No GitHub PAT configured"
                })),
            );
        }
    };

    let client = GitHubClient::new(pat);
    let detector = ProjectDetector::new(client);

    match detector
        .detect(
            &params.owner,
            &params.repo,
            &params.branch,
            params.path_filter.as_deref(),
            params.workflow_path.as_deref(),
        )
        .await
    {
        Ok(config) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/github/detect", timer.elapsed_ms(), 200);
            (StatusCode::OK, Json(serde_json::json!(config)))
        }
        Err(e) => {
            warn!("[{}] Failed to detect project: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "GET", "/api/github/detect", timer.elapsed_ms(), 500);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": e
                })),
            )
        }
    }
}
