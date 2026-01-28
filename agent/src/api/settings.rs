use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use serde::{Deserialize, Serialize};

use crate::state::AppContext;
use crate::infrastructure::logging::{TraceContext, Timer};
use crate::application::ports::repositories::SettingsRepository;

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
            ctx.logger.api_exit(&trace_id, "GET", "/api/settings/webhook-secret", timer.elapsed_ms(), 200);
            (
                StatusCode::OK,
                Json(WebhookSecretResponse {
                    webhook_secret: secret,
                }),
            )
        }
        Ok(None) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/settings/webhook-secret", timer.elapsed_ms(), 404);
            (
                StatusCode::NOT_FOUND,
                Json(WebhookSecretResponse {
                    webhook_secret: "Not configured".to_string(),
                }),
            )
        }
        Err(e) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/settings/webhook-secret", timer.elapsed_ms(), 500);
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
        ctx.logger.api_exit(&trace_id, "POST", "/api/settings/domain", timer.elapsed_ms(), 400);
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Domain cannot be empty"
            })),
        );
    }

    // Save domain to settings
    if let Err(e) = ctx.settings_repo.set("base_domain", domain).await {
        ctx.logger.api_exit(&trace_id, "POST", "/api/settings/domain", timer.elapsed_ms(), 500);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to save domain: {}", e)
            })),
        );
    }

    ctx.logger.api_exit(&trace_id, "POST", "/api/settings/domain", timer.elapsed_ms(), 200);
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
            ctx.logger.api_exit(&trace_id, "GET", "/api/settings/domain", timer.elapsed_ms(), 200);
            (
                StatusCode::OK,
                Json(DomainResponse {
                    configured: true,
                    domain: Some(domain),
                }),
            )
        }
        Ok(None) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/settings/domain", timer.elapsed_ms(), 200);
            (
                StatusCode::OK,
                Json(DomainResponse {
                    configured: false,
                    domain: None,
                }),
            )
        }
        Err(e) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/settings/domain", timer.elapsed_ms(), 500);
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

// TCP Domain settings
#[derive(Debug, Deserialize)]
pub struct SetTcpDomainRequest {
    pub tcp_domain: String,
}

#[derive(Serialize)]
pub struct TcpDomainResponse {
    pub configured: bool,
    pub tcp_domain: Option<String>,
}

/// Set TCP domain configuration (for direct TCP access like Redis, MySQL)
pub async fn set_tcp_domain(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(payload): Json<SetTcpDomainRequest>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "POST", "/api/settings/tcp-domain", &format!("tcp_domain={}", payload.tcp_domain));

    let tcp_domain = payload.tcp_domain.trim();
    if tcp_domain.is_empty() {
        ctx.logger.api_exit(&trace_id, "POST", "/api/settings/tcp-domain", timer.elapsed_ms(), 400);
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "TCP domain cannot be empty"
            })),
        );
    }

    if let Err(e) = ctx.settings_repo.set("tcp_domain", tcp_domain).await {
        ctx.logger.api_exit(&trace_id, "POST", "/api/settings/tcp-domain", timer.elapsed_ms(), 500);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to save TCP domain: {}", e)
            })),
        );
    }

    ctx.logger.api_exit(&trace_id, "POST", "/api/settings/tcp-domain", timer.elapsed_ms(), 200);
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "tcp_domain": tcp_domain
        })),
    )
}

/// Get current TCP domain configuration
pub async fn get_tcp_domain(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", "/api/settings/tcp-domain", "");

    match ctx.settings_repo.get("tcp_domain").await {
        Ok(Some(tcp_domain)) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/settings/tcp-domain", timer.elapsed_ms(), 200);
            (
                StatusCode::OK,
                Json(TcpDomainResponse {
                    configured: true,
                    tcp_domain: Some(tcp_domain),
                }),
            )
        }
        Ok(None) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/settings/tcp-domain", timer.elapsed_ms(), 200);
            (
                StatusCode::OK,
                Json(TcpDomainResponse {
                    configured: false,
                    tcp_domain: None,
                }),
            )
        }
        Err(e) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/settings/tcp-domain", timer.elapsed_ms(), 500);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(TcpDomainResponse {
                    configured: false,
                    tcp_domain: Some(format!("Error: {}", e)),
                }),
            )
        }
    }
}

// Webhook URL settings
#[derive(Debug, Deserialize)]
pub struct SetWebhookUrlRequest {
    pub webhook_url: String,
}

#[derive(Serialize)]
pub struct WebhookUrlResponse {
    pub configured: bool,
    pub webhook_url: Option<String>,
}

/// Set webhook URL configuration (for GitHub webhook registration)
pub async fn set_webhook_url(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(payload): Json<SetWebhookUrlRequest>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "POST", "/api/settings/webhook-url", &format!("webhook_url={}", payload.webhook_url));

    let webhook_url = payload.webhook_url.trim();
    if webhook_url.is_empty() {
        ctx.logger.api_exit(&trace_id, "POST", "/api/settings/webhook-url", timer.elapsed_ms(), 400);
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Webhook URL cannot be empty"
            })),
        );
    }

    if let Err(e) = ctx.settings_repo.set("webhook_url", webhook_url).await {
        ctx.logger.api_exit(&trace_id, "POST", "/api/settings/webhook-url", timer.elapsed_ms(), 500);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to save webhook URL: {}", e)
            })),
        );
    }

    ctx.logger.api_exit(&trace_id, "POST", "/api/settings/webhook-url", timer.elapsed_ms(), 200);
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "webhook_url": webhook_url
        })),
    )
}

/// Get current webhook URL configuration
pub async fn get_webhook_url(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", "/api/settings/webhook-url", "");

    match ctx.settings_repo.get("webhook_url").await {
        Ok(Some(webhook_url)) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/settings/webhook-url", timer.elapsed_ms(), 200);
            (
                StatusCode::OK,
                Json(WebhookUrlResponse {
                    configured: true,
                    webhook_url: Some(webhook_url),
                }),
            )
        }
        Ok(None) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/settings/webhook-url", timer.elapsed_ms(), 200);
            (
                StatusCode::OK,
                Json(WebhookUrlResponse {
                    configured: false,
                    webhook_url: None,
                }),
            )
        }
        Err(e) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/settings/webhook-url", timer.elapsed_ms(), 500);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(WebhookUrlResponse {
                    configured: false,
                    webhook_url: Some(format!("Error: {}", e)),
                }),
            )
        }
    }
}
