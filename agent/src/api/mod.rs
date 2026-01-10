mod webhook;
mod projects;
mod builds;
mod ws;
mod settings;
mod github_api;
pub mod middleware;

pub use webhook::github_webhook;
pub use projects::projects_routes;
pub use builds::builds_routes;
pub use ws::ws_handler;
pub use middleware::TraceIdLayer;

use axum::{routing::{get, post, delete}, Router};
use crate::state::AppState;

pub fn api_routes() -> Router<AppState> {
    Router::new()
        .nest("/projects", projects_routes())
        .nest("/builds", builds_routes())
        .route("/settings/webhook-secret", get(settings::get_webhook_secret))
        .route("/settings/domain", post(settings::set_domain))
        .route("/settings/domain", get(settings::get_domain))
        .route("/settings/github-pat", post(github_api::set_github_pat))
        .route("/settings/github-pat", delete(github_api::delete_github_pat))
        .route("/settings/github-pat-status", get(github_api::get_github_pat_status))
        .route("/github/repositories", get(github_api::list_repositories))
        .route("/github/branches", get(github_api::list_branches))
        .route("/github/folders", get(github_api::list_folders))
        .route("/github/detect-project", get(github_api::detect_project))
}
