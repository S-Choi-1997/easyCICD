use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::github::{GitHubClient, ProjectDetector};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct SetPATRequest {
    pub github_pat: String,
}

/// Set GitHub PAT (global configuration)
pub async fn set_github_pat(
    State(state): State<AppState>,
    Json(payload): Json<SetPATRequest>,
) -> impl IntoResponse {
    // Validate PAT by getting user info
    let client = GitHubClient::new(payload.github_pat.clone());

    let github_user = match client.get_user().await {
        Ok(user) => user,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(serde_json::json!({
                    "error": format!("Invalid GitHub PAT: {}", e)
                })),
            );
        }
    };

    // Save PAT to settings
    if let Err(e) = state.db.set_github_pat(&payload.github_pat).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to save PAT: {}", e)
            })),
        );
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "github_username": github_user.login
        })),
    )
}

/// Get current GitHub PAT status
pub async fn get_github_pat_status(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.get_github_pat().await {
        Ok(Some(pat)) => {
            // Validate PAT
            let client = GitHubClient::new(pat);
            match client.get_user().await {
                Ok(user) => (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "configured": true,
                        "github_username": user.login
                    })),
                ),
                Err(_) => (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "configured": false,
                        "error": "PAT is invalid or expired"
                    })),
                ),
            }
        }
        _ => (
            StatusCode::OK,
            Json(serde_json::json!({
                "configured": false
            })),
        ),
    }
}

/// List user's GitHub repositories
pub async fn list_repositories(State(state): State<AppState>) -> impl IntoResponse {
    let pat = match state.db.get_github_pat().await {
        Ok(Some(pat)) => pat,
        _ => {
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
        Ok(repos) => (StatusCode::OK, Json(serde_json::json!({ "repositories": repos }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to fetch repositories: {}", e),
                "repositories": []
            })),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct BranchesQuery {
    pub owner: String,
    pub repo: String,
}

/// List repository branches
pub async fn list_branches(
    State(state): State<AppState>,
    Query(params): Query<BranchesQuery>,
) -> impl IntoResponse {
    let pat = match state.db.get_github_pat().await {
        Ok(Some(pat)) => pat,
        _ => {
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
        Ok(branches) => (StatusCode::OK, Json(serde_json::json!({ "branches": branches }))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to fetch branches: {}", e),
                "branches": []
            })),
        ),
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
    State(state): State<AppState>,
    Query(params): Query<FoldersQuery>,
) -> impl IntoResponse {
    let pat = match state.db.get_github_pat().await {
        Ok(Some(pat)) => pat,
        _ => {
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

            (StatusCode::OK, Json(serde_json::json!({ "folders": folders })))
        }
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to fetch folders: {}", e),
                "folders": []
            })),
        ),
    }
}

#[derive(Debug, Deserialize)]
pub struct DetectProjectQuery {
    pub owner: String,
    pub repo: String,
    pub branch: String,
    pub path_filter: Option<String>,
}

/// Detect project type and generate configuration
pub async fn detect_project(
    State(state): State<AppState>,
    Query(params): Query<DetectProjectQuery>,
) -> impl IntoResponse {
    let pat = match state.db.get_github_pat().await {
        Ok(Some(pat)) => pat,
        _ => {
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
        )
        .await
    {
        Ok(config) => (StatusCode::OK, Json(serde_json::json!(config))),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": e
            })),
        ),
    }
}
