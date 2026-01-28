mod webhook;
mod projects;
mod builds;
mod containers;
mod ws;
mod settings;
mod github_api;
mod auth;
pub mod terminal;
pub mod middleware;

pub use webhook::github_webhook;
pub use projects::projects_routes;
pub use builds::builds_routes;
pub use containers::containers_routes;
pub use ws::ws_handler;
pub use middleware::TraceIdLayer;
pub use auth::auth_routes;

use axum::{routing::{get, post, delete}, Router};
use crate::state::AppContext;

/// Admin routes - no auth required (for initial setup like whitelist)
pub fn admin_routes() -> Router<AppContext> {
    Router::new()
        .route("/allowed-emails", get(settings::get_allowed_emails))
        .route("/allowed-emails", post(settings::add_allowed_email))
        .route("/allowed-emails", delete(settings::remove_allowed_email))
}

pub fn api_routes() -> Router<AppContext> {
    Router::new()
        .nest("/projects", projects_routes())
        .nest("/builds", builds_routes())
        .nest("/containers", containers_routes())
        .route("/settings/webhook-secret", get(settings::get_webhook_secret))
        .route("/settings/domain", post(settings::set_domain))
        .route("/settings/domain", get(settings::get_domain))
        .route("/settings/tcp-domain", post(settings::set_tcp_domain))
        .route("/settings/tcp-domain", get(settings::get_tcp_domain))
        .route("/settings/webhook-url", post(settings::set_webhook_url))
        .route("/settings/webhook-url", get(settings::get_webhook_url))
        .route("/settings/server-ip", get(settings::get_server_ip))
        .route("/settings/github-pat", post(github_api::set_github_pat))
        .route("/settings/github-pat", delete(github_api::delete_github_pat))
        .route("/settings/github-pat-status", get(github_api::get_github_pat_status))
        .route("/github/repositories", get(github_api::list_repositories))
        .route("/github/branches", get(github_api::list_branches))
        .route("/github/folders", get(github_api::list_folders))
        .route("/github/detect-project", get(github_api::detect_project))
}
