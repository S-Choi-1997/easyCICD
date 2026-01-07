mod webhook;
mod projects;
mod builds;
mod ws;

pub use webhook::github_webhook;
pub use projects::projects_routes;
pub use builds::builds_routes;
pub use ws::ws_handler;

use axum::Router;
use crate::state::AppState;

pub fn api_routes() -> Router<AppState> {
    Router::new()
        .nest("/projects", projects_routes())
        .nest("/builds", builds_routes())
}
