use axum::{
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::Deserialize;
use tracing::warn;

use crate::db::models::{CreateGitHubPat, GitHubPatSummary};
use crate::github::{GitHubClient, ProjectDetector};
use crate::state::AppContext;
use crate::infrastructure::logging::{TraceContext, Timer};
use crate::application::ports::repositories::{SettingsRepository, GitHubPatRepository, ProjectRepository};

// ============================================================================
// PAT CRUD Endpoints
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct CreatePatRequest {
    pub label: String,
    pub token: String,
}

/// Create a new GitHub PAT
pub async fn create_pat(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(payload): Json<CreatePatRequest>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "POST", "/api/github/pats", "create_pat");

    // Validate PAT by getting user info
    let client = GitHubClient::new(payload.token.clone());
    let github_user = match client.get_user().await {
        Ok(user) => user,
        Err(e) => {
            warn!("[{}] Invalid GitHub PAT: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "POST", "/api/github/pats", timer.elapsed_ms(), 400);
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Invalid GitHub PAT: {}", e)
                })),
            );
        }
    };

    let create = CreateGitHubPat {
        label: payload.label,
        token: payload.token,
        github_username: Some(github_user.login.clone()),
    };

    match ctx.github_pat_repo.create(create).await {
        Ok(pat) => {
            let summary: GitHubPatSummary = pat.into();
            ctx.logger.api_exit(&trace_id, "POST", "/api/github/pats", timer.elapsed_ms(), 201);
            (StatusCode::CREATED, Json(serde_json::json!(summary)))
        }
        Err(e) => {
            warn!("[{}] Failed to create PAT: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "POST", "/api/github/pats", timer.elapsed_ms(), 500);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Failed to create PAT: {}", e)
                })),
            )
        }
    }
}

/// List all GitHub PATs (masked tokens)
pub async fn list_pats(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", "/api/github/pats", "");

    match ctx.github_pat_repo.list().await {
        Ok(pats) => {
            let summaries: Vec<GitHubPatSummary> = pats.into_iter().map(|p| p.into()).collect();
            ctx.logger.api_exit(&trace_id, "GET", "/api/github/pats", timer.elapsed_ms(), 200);
            (StatusCode::OK, Json(serde_json::json!({ "pats": summaries })))
        }
        Err(e) => {
            warn!("[{}] Failed to list PATs: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "GET", "/api/github/pats", timer.elapsed_ms(), 500);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({
                    "error": format!("Failed to list PATs: {}", e),
                    "pats": []
                })),
            )
        }
    }
}

/// Get a single GitHub PAT (masked token)
pub async fn get_pat(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", &format!("/api/github/pats/{}", id), "");

    match ctx.github_pat_repo.get(id).await {
        Ok(Some(pat)) => {
            let summary: GitHubPatSummary = pat.into();
            ctx.logger.api_exit(&trace_id, "GET", &format!("/api/github/pats/{}", id), timer.elapsed_ms(), 200);
            (StatusCode::OK, Json(serde_json::json!(summary)))
        }
        Ok(None) => {
            ctx.logger.api_exit(&trace_id, "GET", &format!("/api/github/pats/{}", id), timer.elapsed_ms(), 404);
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "PAT not found"})),
            )
        }
        Err(e) => {
            warn!("[{}] Failed to get PAT: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "GET", &format!("/api/github/pats/{}", id), timer.elapsed_ms(), 500);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to get PAT: {}", e)})),
            )
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct UpdatePatRequest {
    pub label: Option<String>,
    pub token: Option<String>,
}

/// Update a GitHub PAT
pub async fn update_pat(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<UpdatePatRequest>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "PUT", &format!("/api/github/pats/{}", id), "");

    // If token is being updated, validate it
    let github_username = if let Some(ref token) = payload.token {
        let client = GitHubClient::new(token.clone());
        match client.get_user().await {
            Ok(user) => Some(user.login),
            Err(e) => {
                warn!("[{}] Invalid GitHub PAT: {}", trace_id, e);
                ctx.logger.api_exit(&trace_id, "PUT", &format!("/api/github/pats/{}", id), timer.elapsed_ms(), 400);
                return (
                    StatusCode::BAD_REQUEST,
                    Json(serde_json::json!({
                        "error": format!("Invalid GitHub PAT: {}", e)
                    })),
                );
            }
        }
    } else {
        None
    };

    match ctx.github_pat_repo.update(
        id,
        payload.label.as_deref(),
        payload.token.as_deref(),
        github_username.as_deref(),
    ).await {
        Ok(pat) => {
            let summary: GitHubPatSummary = pat.into();
            ctx.logger.api_exit(&trace_id, "PUT", &format!("/api/github/pats/{}", id), timer.elapsed_ms(), 200);
            (StatusCode::OK, Json(serde_json::json!(summary)))
        }
        Err(e) => {
            warn!("[{}] Failed to update PAT: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "PUT", &format!("/api/github/pats/{}", id), timer.elapsed_ms(), 500);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to update PAT: {}", e)})),
            )
        }
    }
}

/// Delete a GitHub PAT
pub async fn delete_pat(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "DELETE", &format!("/api/github/pats/{}", id), "");

    // Check if any projects reference this PAT
    if let Ok(projects) = ctx.project_repo.list().await {
        let referencing: Vec<&str> = projects.iter()
            .filter(|p| p.github_pat_id == Some(id))
            .map(|p| p.name.as_str())
            .collect();
        if !referencing.is_empty() {
            ctx.logger.api_exit(&trace_id, "DELETE", &format!("/api/github/pats/{}", id), timer.elapsed_ms(), 409);
            return (
                StatusCode::CONFLICT,
                Json(serde_json::json!({
                    "error": format!("PAT is used by projects: {}. Remove the PAT from these projects first.", referencing.join(", "))
                })),
            );
        }
    }

    match ctx.github_pat_repo.delete(id).await {
        Ok(_) => {
            ctx.logger.api_exit(&trace_id, "DELETE", &format!("/api/github/pats/{}", id), timer.elapsed_ms(), 200);
            (
                StatusCode::OK,
                Json(serde_json::json!({"success": true, "message": "PAT deleted successfully"})),
            )
        }
        Err(e) => {
            warn!("[{}] Failed to delete PAT: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "DELETE", &format!("/api/github/pats/{}", id), timer.elapsed_ms(), 500);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to delete PAT: {}", e)})),
            )
        }
    }
}

// ============================================================================
// Helper: Resolve PAT token from pat_id or legacy settings
// ============================================================================

async fn resolve_pat(ctx: &AppContext, pat_id: Option<i64>) -> Result<String, (StatusCode, Json<serde_json::Value>)> {
    if let Some(id) = pat_id {
        match ctx.github_pat_repo.get(id).await {
            Ok(Some(pat)) => return Ok(pat.token),
            Ok(None) => return Err((
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "PAT not found"})),
            )),
            Err(e) => return Err((
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to get PAT: {}", e)})),
            )),
        }
    }
    // Fallback: legacy global PAT from settings
    match ctx.settings_repo.get("github_pat").await {
        Ok(Some(pat)) => Ok(pat),
        _ => Err((
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "No GitHub PAT configured. Please add a PAT first."})),
        )),
    }
}

// ============================================================================
// Legacy PAT Endpoints (kept for backward compatibility)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct SetPATRequest {
    pub github_pat: String,
}

/// Set GitHub PAT (global configuration - legacy)
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

    // Save PAT to settings (legacy)
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

/// Get current GitHub PAT status (legacy)
pub async fn get_github_pat_status(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", "/api/github/pat", "");

    // First check if there are any PATs in the new table
    let pats_exist = match ctx.github_pat_repo.list().await {
        Ok(pats) => !pats.is_empty(),
        Err(_) => false,
    };

    if pats_exist {
        // Use the first PAT to check status
        match ctx.github_pat_repo.list().await {
            Ok(pats) if !pats.is_empty() => {
                let pat = &pats[0];
                let client = GitHubClient::new(pat.token.clone());
                match client.get_user().await {
                    Ok(user) => {
                        ctx.logger.api_exit(&trace_id, "GET", "/api/github/pat", timer.elapsed_ms(), 200);
                        return (
                            StatusCode::OK,
                            Json(serde_json::json!({
                                "configured": true,
                                "github_username": user.login
                            })),
                        );
                    }
                    Err(e) => {
                        warn!("[{}] PAT validation failed: {}", trace_id, e);
                    }
                }
            }
            _ => {}
        }
    }

    // Fallback to legacy settings
    match ctx.settings_repo.get("github_pat").await {
        Ok(Some(pat)) => {
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

/// Delete GitHub PAT (legacy - sign out)
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

// ============================================================================
// GitHub API Endpoints (with pat_id support)
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct PatIdQuery {
    pub pat_id: Option<i64>,
}

/// List user's GitHub repositories
pub async fn list_repositories(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Query(params): Query<PatIdQuery>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", "/api/github/repositories", "");

    let pat = match resolve_pat(&ctx, params.pat_id).await {
        Ok(pat) => pat,
        Err((status, json)) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/github/repositories", timer.elapsed_ms(), status.as_u16());
            return (status, json);
        }
    };

    let client = GitHubClient::new(pat);

    match client.list_repositories().await {
        Ok(repos) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/github/repositories", timer.elapsed_ms(), 200);
            (StatusCode::OK, Json(serde_json::json!({ "repositories": repos })))
        }
        Err(e) => {
            let error_msg = e.to_string();
            // Check if it's an authentication error
            let (status, user_msg) = if error_msg.contains("401") || error_msg.contains("Unauthorized") {
                warn!("[{}] Invalid or expired GitHub PAT: {}", trace_id, e);
                (StatusCode::UNAUTHORIZED, "Invalid or expired GitHub PAT. Please check your token.")
            } else if error_msg.contains("403") || error_msg.contains("Forbidden") {
                warn!("[{}] Insufficient GitHub PAT permissions: {}", trace_id, e);
                (StatusCode::FORBIDDEN, "Insufficient permissions. Please check your PAT scopes.")
            } else {
                warn!("[{}] Failed to fetch repositories: {}", trace_id, e);
                (StatusCode::INTERNAL_SERVER_ERROR, "Failed to fetch repositories from GitHub.")
            };

            ctx.logger.api_exit(&trace_id, "GET", "/api/github/repositories", timer.elapsed_ms(), status.as_u16());
            (
                status,
                Json(serde_json::json!({
                    "error": user_msg,
                    "detail": error_msg,
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
    pub pat_id: Option<i64>,
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

    let pat = match resolve_pat(&ctx, params.pat_id).await {
        Ok(pat) => pat,
        Err((status, json)) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/github/branches", timer.elapsed_ms(), status.as_u16());
            return (status, json);
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
    pub pat_id: Option<i64>,
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

    let pat = match resolve_pat(&ctx, params.pat_id).await {
        Ok(pat) => pat,
        Err((status, json)) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/github/folders", timer.elapsed_ms(), status.as_u16());
            return (status, json);
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
    pub pat_id: Option<i64>,
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

    let pat = match resolve_pat(&ctx, params.pat_id).await {
        Ok(pat) => pat,
        Err((status, json)) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/github/detect", timer.elapsed_ms(), status.as_u16());
            return (status, json);
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
