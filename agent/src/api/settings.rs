use axum::{
    extract::State,
    http::StatusCode,
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

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

#[derive(Debug, Deserialize)]
pub struct SetDomainRequest {
    pub domain: String,
}

#[derive(Serialize)]
pub struct DomainResponse {
    pub configured: bool,
    pub domain: Option<String>,
}

/// Set domain configuration
pub async fn set_domain(
    State(state): State<AppState>,
    Json(payload): Json<SetDomainRequest>,
) -> impl IntoResponse {
    // Validate domain format (basic validation)
    let domain = payload.domain.trim();
    if domain.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Domain cannot be empty"
            })),
        );
    }

    // Save domain to settings
    if let Err(e) = state.db.set_domain(domain).await {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to save domain: {}", e)
            })),
        );
    }

    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "domain": domain
        })),
    )
}

/// Get current domain configuration
pub async fn get_domain(State(state): State<AppState>) -> impl IntoResponse {
    match state.db.get_domain().await {
        Ok(Some(domain)) => (
            StatusCode::OK,
            Json(DomainResponse {
                configured: true,
                domain: Some(domain),
            }),
        ),
        Ok(None) => (
            StatusCode::OK,
            Json(DomainResponse {
                configured: false,
                domain: None,
            }),
        ),
        Err(e) => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(DomainResponse {
                configured: false,
                domain: Some(format!("Error: {}", e)),
            }),
        ),
    }
}
