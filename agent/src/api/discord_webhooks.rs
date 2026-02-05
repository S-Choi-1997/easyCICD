use axum::{
    extract::{Path, State},
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json, Router,
    routing::{get, post, put, delete},
};
use tracing::warn;

use crate::infrastructure::database::{CreateDiscordWebhook, UpdateDiscordWebhook};
use crate::state::AppContext;
use crate::infrastructure::logging::{TraceContext, Timer};

pub fn discord_webhooks_routes() -> Router<AppContext> {
    Router::new()
        .route("/", get(list_webhooks))
        .route("/", post(create_webhook))
        .route("/{id}", get(get_webhook))
        .route("/{id}", put(update_webhook))
        .route("/{id}", delete(delete_webhook))
}

/// List all Discord webhooks
async fn list_webhooks(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", "/api/discord-webhooks", "");

    match ctx.discord_webhook_repo.list_all().await {
        Ok(webhooks) => {
            ctx.logger.api_exit(&trace_id, "GET", "/api/discord-webhooks", timer.elapsed_ms(), 200);
            (StatusCode::OK, Json(serde_json::json!({"webhooks": webhooks})))
        }
        Err(e) => {
            warn!("[{}] Failed to list Discord webhooks: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "GET", "/api/discord-webhooks", timer.elapsed_ms(), 500);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to list webhooks: {}", e)})),
            )
        }
    }
}

/// Get a Discord webhook by ID
async fn get_webhook(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "GET", &format!("/api/discord-webhooks/{}", id), "");

    match ctx.discord_webhook_repo.get(id).await {
        Ok(Some(webhook)) => {
            ctx.logger.api_exit(&trace_id, "GET", &format!("/api/discord-webhooks/{}", id), timer.elapsed_ms(), 200);
            (StatusCode::OK, Json(serde_json::json!(webhook)))
        }
        Ok(None) => {
            ctx.logger.api_exit(&trace_id, "GET", &format!("/api/discord-webhooks/{}", id), timer.elapsed_ms(), 404);
            (
                StatusCode::NOT_FOUND,
                Json(serde_json::json!({"error": "Discord webhook not found"})),
            )
        }
        Err(e) => {
            warn!("[{}] Failed to get Discord webhook: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "GET", &format!("/api/discord-webhooks/{}", id), timer.elapsed_ms(), 500);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to get webhook: {}", e)})),
            )
        }
    }
}

/// Create a new Discord webhook
async fn create_webhook(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Json(payload): Json<CreateDiscordWebhook>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "POST", "/api/discord-webhooks", &format!("label={}", payload.label));

    match ctx.discord_webhook_repo.create(payload).await {
        Ok(webhook) => {
            ctx.logger.api_exit(&trace_id, "POST", "/api/discord-webhooks", timer.elapsed_ms(), 201);
            (StatusCode::CREATED, Json(serde_json::json!(webhook)))
        }
        Err(e) => {
            warn!("[{}] Failed to create Discord webhook: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "POST", "/api/discord-webhooks", timer.elapsed_ms(), 500);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to create webhook: {}", e)})),
            )
        }
    }
}

/// Update a Discord webhook
async fn update_webhook(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(payload): Json<UpdateDiscordWebhook>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "PUT", &format!("/api/discord-webhooks/{}", id), "");

    match ctx.discord_webhook_repo.update(id, payload).await {
        Ok(webhook) => {
            ctx.logger.api_exit(&trace_id, "PUT", &format!("/api/discord-webhooks/{}", id), timer.elapsed_ms(), 200);
            (StatusCode::OK, Json(serde_json::json!(webhook)))
        }
        Err(e) => {
            warn!("[{}] Failed to update Discord webhook: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "PUT", &format!("/api/discord-webhooks/{}", id), timer.elapsed_ms(), 500);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to update webhook: {}", e)})),
            )
        }
    }
}

/// Delete a Discord webhook
async fn delete_webhook(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "DELETE", &format!("/api/discord-webhooks/{}", id), "");

    match ctx.discord_webhook_repo.delete(id).await {
        Ok(_) => {
            ctx.logger.api_exit(&trace_id, "DELETE", &format!("/api/discord-webhooks/{}", id), timer.elapsed_ms(), 204);
            (StatusCode::NO_CONTENT, Json(serde_json::json!({})))
        }
        Err(e) => {
            warn!("[{}] Failed to delete Discord webhook: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "DELETE", &format!("/api/discord-webhooks/{}", id), timer.elapsed_ms(), 500);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to delete webhook: {}", e)})),
            )
        }
    }
}

/// Update project's Discord webhook association
pub async fn set_project_discord_webhook(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    Path(project_id): Path<i64>,
    Json(payload): Json<serde_json::Value>,
) -> impl IntoResponse {
    use crate::application::ports::repositories::ProjectRepository;

    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "POST", &format!("/api/projects/{}/discord-webhook", project_id), "");

    let webhook_id = payload.get("webhook_id").and_then(|v| v.as_i64());

    match ctx.project_repo.update_discord_webhook_id(project_id, webhook_id).await {
        Ok(_) => {
            ctx.logger.api_exit(&trace_id, "POST", &format!("/api/projects/{}/discord-webhook", project_id), timer.elapsed_ms(), 200);
            (StatusCode::OK, Json(serde_json::json!({"success": true})))
        }
        Err(e) => {
            warn!("[{}] Failed to update project Discord webhook: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "POST", &format!("/api/projects/{}/discord-webhook", project_id), timer.elapsed_ms(), 500);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to update project webhook: {}", e)})),
            )
        }
    }
}
