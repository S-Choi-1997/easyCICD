use axum::{
    extract::{Request, State},
    http::StatusCode,
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use tower_cookies::Cookies;

use crate::state::AppContext;
use crate::application::ports::repositories::SessionRepository;

const SESSION_COOKIE: &str = "easycicd_session";

/// Auth middleware - validates session for all /api/* routes
/// Use with axum::middleware::from_fn_with_state
pub async fn require_auth(
    State(ctx): State<AppContext>,
    cookies: Cookies,
    request: Request,
    next: Next,
) -> Response {
    // Get session cookie
    let session_cookie = match cookies.get(SESSION_COOKIE) {
        Some(c) => c,
        None => {
            return (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "Authentication required",
                    "code": "UNAUTHORIZED"
                })),
            ).into_response();
        }
    };

    let session_id = session_cookie.value();

    // Validate session
    match ctx.session_repo.get(session_id).await {
        Ok(Some(_session)) => {
            // Session valid, proceed
            next.run(request).await
        }
        _ => {
            // Session invalid or expired
            (
                StatusCode::UNAUTHORIZED,
                Json(serde_json::json!({
                    "error": "Session expired or invalid",
                    "code": "SESSION_EXPIRED"
                })),
            ).into_response()
        }
    }
}
