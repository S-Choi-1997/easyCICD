use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::Serialize;

use crate::state::AppState;

#[derive(Serialize)]
pub struct WebhookSecretResponse {
    webhook_secret: String,
}

pub async fn get_webhook_secret(
    State(state): State<AppState>,
) -> impl IntoResponse {
    match state.db.get_setting("webhook_secret").await {
        Ok(Some(secret)) => (
            StatusCode::OK,
            Json(WebhookSecretResponse {
                webhook_secret: secret,
            }),
        ),
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(WebhookSecretResponse {
                webhook_secret: "Not configured".to_string(),
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(WebhookSecretResponse {
                webhook_secret: format!("Error: {}", e),
            }),
        ),
    }
}
