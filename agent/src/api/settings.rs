use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::state::AppContext;
use crate::infrastructure::logging::{TraceContext, Timer};

#[derive(Serialize)]
pub struct WebhookSecretResponse {
    webhook_secret: String,
}

pub async fn get_webhook_secret(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", "/api/settings/webhook-secret", "");

    match ctx.settings_repo.get("webhook_secret").await {
        Ok(Some(secret)) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/settings/webhook-secret", timer.elapsed_ms(), "200");
            (
                StatusCode::OK,
                Json(WebhookSecretResponse {
                    webhook_secret: secret,
                }),
            )
        }
        Ok(None) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/settings/webhook-secret", timer.elapsed_ms(), "404");
            (
                StatusCode::NOT_FOUND,
                Json(WebhookSecretResponse {
                    webhook_secret: "Not configured".to_string(),
                }),
            )
        }
        Err(e) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/settings/webhook-secret", timer.elapsed_ms(), "500");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(WebhookSecretResponse {
                    webhook_secret: format!("Error: {}", e),
                }),
            )
        }
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
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(payload): Json<SetDomainRequest>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "POST", "/api/settings/domain", &format!("domain={}", payload.domain));

    // Validate domain format (basic validation)
    let domain = payload.domain.trim();
    if domain.is_empty() {
        ctx.logger.api_exit(&trace_id, "POST", "/api/settings/domain", timer.elapsed_ms(), "400");
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Domain cannot be empty"
            })),
        );
    }

    // Save domain to settings
    if let Err(e) = ctx.settings_repo.set("base_domain", domain).await {
        ctx.logger.api_exit(&trace_id, "POST", "/api/settings/domain", timer.elapsed_ms(), "500");
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to save domain: {}", e)
            })),
        );
    }

    ctx.logger.api_exit(&trace_id, "POST", "/api/settings/domain", timer.elapsed_ms(), "200");
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "domain": domain
        })),
    )
}

/// Get current domain configuration
pub async fn get_domain(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", "/api/settings/domain", "");

    match ctx.settings_repo.get("base_domain").await {
        Ok(Some(domain)) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/settings/domain", timer.elapsed_ms(), "200");
            (
                StatusCode::OK,
                Json(DomainResponse {
                    configured: true,
                    domain: Some(domain),
                }),
            )
        }
        Ok(None) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/settings/domain", timer.elapsed_ms(), "200");
            (
                StatusCode::OK,
                Json(DomainResponse {
                    configured: false,
                    domain: None,
                }),
            )
        }
        Err(e) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/settings/domain", timer.elapsed_ms(), "500");
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(DomainResponse {
                    configured: false,
                    domain: Some(format!("Error: {}", e)),
                }),
            )
        }
    }
}
