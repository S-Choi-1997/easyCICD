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

// ============================================================================
// Email Whitelist Settings
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct AddEmailRequest {
    pub email: String,
}

#[derive(Debug, Deserialize)]
pub struct RemoveEmailRequest {
    pub email: String,
}

#[derive(Serialize)]
pub struct AllowedEmailsResponse {
    pub emails: Vec<String>,
    pub enabled: bool,
}

/// Helper function to get allowed emails from settings
async fn get_allowed_emails_list(ctx: &AppContext) -> Vec<String> {
    match ctx.settings_repo.get("allowed_emails").await {
        Ok(Some(json_str)) => {
            serde_json::from_str(&json_str).unwrap_or_default()
        }
        _ => Vec::new(),
    }
}

/// Helper function to save allowed emails to settings
async fn save_allowed_emails_list(ctx: &AppContext, emails: &[String]) -> Result<(), anyhow::Error> {
    let json_str = serde_json::to_string(emails)?;
    ctx.settings_repo.set("allowed_emails", &json_str).await
}

/// Check if an email is in the whitelist (returns true if whitelist is empty or email is allowed)
pub async fn is_email_allowed(ctx: &AppContext, email: &str) -> bool {
    let emails = get_allowed_emails_list(ctx).await;

    // If whitelist is empty, allow all
    if emails.is_empty() {
        return true;
    }

    // Check if email is in the whitelist (case-insensitive)
    let email_lower = email.to_lowercase();
    emails.iter().any(|e| e.to_lowercase() == email_lower)
}

/// GET /api/settings/allowed-emails - List all allowed emails
pub async fn get_allowed_emails(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", "/api/settings/allowed-emails", "");

    let emails = get_allowed_emails_list(&ctx).await;
    let enabled = !emails.is_empty();

    ctx.logger.api_exit(&trace_id, "GET", "/api/settings/allowed-emails", timer.elapsed_ms(), 200);
    (
        StatusCode::OK,
        Json(AllowedEmailsResponse { emails, enabled }),
    )
}

/// POST /api/settings/allowed-emails - Add an email to whitelist
pub async fn add_allowed_email(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(payload): Json<AddEmailRequest>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "POST", "/api/settings/allowed-emails", &format!("email={}", payload.email));

    let email = payload.email.trim().to_lowercase();

    // Validate email format (basic validation)
    if !email.contains('@') || email.len() < 5 {
        ctx.logger.api_exit(&trace_id, "POST", "/api/settings/allowed-emails", timer.elapsed_ms(), 400);
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({
                "error": "Invalid email format"
            })),
        );
    }

    // Get current list and add email
    let mut emails = get_allowed_emails_list(&ctx).await;

    // Check if already exists
    if emails.iter().any(|e| e.to_lowercase() == email) {
        ctx.logger.api_exit(&trace_id, "POST", "/api/settings/allowed-emails", timer.elapsed_ms(), 409);
        return (
            StatusCode::CONFLICT,
            Json(serde_json::json!({
                "error": "Email already in whitelist"
            })),
        );
    }

    emails.push(email.clone());

    if let Err(e) = save_allowed_emails_list(&ctx, &emails).await {
        ctx.logger.api_exit(&trace_id, "POST", "/api/settings/allowed-emails", timer.elapsed_ms(), 500);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to save: {}", e)
            })),
        );
    }

    ctx.logger.api_exit(&trace_id, "POST", "/api/settings/allowed-emails", timer.elapsed_ms(), 200);
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "email": email,
            "emails": emails
        })),
    )
}

/// DELETE /api/settings/allowed-emails - Remove an email from whitelist
pub async fn remove_allowed_email(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(payload): Json<RemoveEmailRequest>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "DELETE", "/api/settings/allowed-emails", &format!("email={}", payload.email));

    let email = payload.email.trim().to_lowercase();

    // Get current list and remove email
    let mut emails = get_allowed_emails_list(&ctx).await;
    let original_len = emails.len();

    emails.retain(|e| e.to_lowercase() != email);

    if emails.len() == original_len {
        ctx.logger.api_exit(&trace_id, "DELETE", "/api/settings/allowed-emails", timer.elapsed_ms(), 404);
        return (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({
                "error": "Email not found in whitelist"
            })),
        );
    }

    if let Err(e) = save_allowed_emails_list(&ctx, &emails).await {
        ctx.logger.api_exit(&trace_id, "DELETE", "/api/settings/allowed-emails", timer.elapsed_ms(), 500);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({
                "error": format!("Failed to save: {}", e)
            })),
        );
    }

    ctx.logger.api_exit(&trace_id, "DELETE", "/api/settings/allowed-emails", timer.elapsed_ms(), 200);
    (
        StatusCode::OK,
        Json(serde_json::json!({
            "success": true,
            "removed": email,
            "emails": emails
        })),
    )
}

// ============================================================================
// Server IP Settings
// ============================================================================

#[derive(Serialize)]
pub struct ServerIpResponse {
    pub server_ip: String,
}

/// GET /api/settings/server-ip - Get server's public IP address
pub async fn get_server_ip(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", "/api/settings/server-ip", "");

    // Try to get server's public IP from external service
    let server_ip = match reqwest::blocking::get("https://api.ipify.org")
        .and_then(|resp| resp.text())
    {
        Ok(ip) => {
            let trimmed = ip.trim().to_string();
            if !trimmed.is_empty() && trimmed != "127.0.0.1" {
                trimmed
            } else {
                "localhost".to_string()
            }
        }
        Err(_) => {
            // Fallback: try hostname -I
            match std::process::Command::new("hostname")
                .arg("-I")
                .output()
            {
                Ok(output) => {
                    let ip_string = String::from_utf8_lossy(&output.stdout);
                    ip_string
                        .split_whitespace()
                        .find(|ip| !ip.starts_with("172.") && !ip.starts_with("127."))
                        .unwrap_or("localhost")
                        .to_string()
                }
                Err(_) => "localhost".to_string(),
            }
        }
    };

    ctx.logger.api_exit(&trace_id, "GET", "/api/settings/server-ip", timer.elapsed_ms(), 200);
    (
        StatusCode::OK,
        Json(ServerIpResponse { server_ip }),
    )
}
