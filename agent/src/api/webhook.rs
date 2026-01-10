use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::IntoResponse,
    Json,
};
use globset::{Glob, GlobSetBuilder};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use tracing::{info, warn};

use crate::db::models::{BuildStatus, CreateBuild};
use crate::events::Event;
use crate::state::AppContext;
use crate::infrastructure::logging::{TraceContext, Timer};

type HmacSha256 = Hmac<Sha256>;

#[derive(Debug, Deserialize)]
pub struct GithubWebhook {
    #[serde(rename = "ref")]
    pub git_ref: Option<String>,
    pub repository: Repository,
    pub head_commit: Option<Commit>,
    pub commits: Option<Vec<Commit>>,
}

#[derive(Debug, Deserialize)]
pub struct Repository {
    pub full_name: String,
}

#[derive(Debug, Deserialize)]
pub struct Commit {
    pub id: String,
    pub message: String,
    pub author: Author,
    pub added: Vec<String>,
    pub modified: Vec<String>,
    pub removed: Vec<String>,
}

#[derive(Debug, Deserialize)]
pub struct Author {
    pub name: String,
    pub email: String,
}

#[derive(Serialize)]
pub struct WebhookResponse {
    message: String,
    build_id: Option<i64>,
}

pub async fn github_webhook(
    State(ctx): State<AppContext>,
    headers: HeaderMap,
    body: String,
) -> impl IntoResponse {
    let trace_id = TraceContext::extract_or_generate(&headers);
    let timer = Timer::start();

    ctx.logger.api_entry(&trace_id, "POST", "/webhook/github", "webhook_received");

    // Verify signature
    if let Err(e) = verify_signature(&ctx, &headers, &body).await {
        warn!("[{}] Webhook signature verification failed: {}", trace_id, e);
        ctx.logger.api_exit(&trace_id, "POST", "/webhook/github", timer.elapsed_ms(), "401");
        return (
            StatusCode::UNAUTHORIZED,
            Json(WebhookResponse {
                message: "Invalid signature".to_string(),
                build_id: None,
            }),
        );
    }

    // Parse webhook payload
    let webhook: GithubWebhook = match serde_json::from_str(&body) {
        Ok(w) => w,
        Err(e) => {
            warn!("[{}] Failed to parse webhook payload: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "POST", "/webhook/github", timer.elapsed_ms(), "400");
            return (
                StatusCode::BAD_REQUEST,
                Json(WebhookResponse {
                    message: format!("Invalid payload: {}", e),
                    build_id: None,
                }),
            );
        }
    };

    info!("[{}] Received webhook for repo: {}", trace_id, webhook.repository.full_name);

    // Extract branch from ref (refs/heads/main -> main)
    let branch = webhook
        .git_ref
        .as_ref()
        .and_then(|r| r.strip_prefix("refs/heads/"))
        .unwrap_or("main");

    // Find matching project
    let projects = match ctx.project_repo.list().await {
        Ok(p) => p,
        Err(e) => {
            warn!("[{}] Failed to list projects: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "POST", "/webhook/github", timer.elapsed_ms(), "500");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(WebhookResponse {
                    message: "Internal error".to_string(),
                    build_id: None,
                }),
            );
        }
    };

    let matching_project = projects.iter().find(|p| {
        p.repo == webhook.repository.full_name && p.branch == branch
    });

    let project = match matching_project {
        Some(p) => p,
        None => {
            info!(
                "[{}] No matching project found for repo {} branch {}",
                trace_id, webhook.repository.full_name, branch
            );
            ctx.logger.api_exit(&trace_id, "POST", "/webhook/github", timer.elapsed_ms(), "200");
            return (
                StatusCode::OK,
                Json(WebhookResponse {
                    message: "No matching project".to_string(),
                    build_id: None,
                }),
            );
        }
    };

    // Check if files match path_filter
    let head_commit = match webhook.head_commit {
        Some(c) => c,
        None => {
            info!("[{}] No head commit in webhook", trace_id);
            ctx.logger.api_exit(&trace_id, "POST", "/webhook/github", timer.elapsed_ms(), "200");
            return (
                StatusCode::OK,
                Json(WebhookResponse {
                    message: "No commits".to_string(),
                    build_id: None,
                }),
            );
        }
    };

    // Glob pattern path filter check
    let files_changed: Vec<String> = head_commit
        .added
        .iter()
        .chain(head_commit.modified.iter())
        .chain(head_commit.removed.iter())
        .cloned()
        .collect();

    let matches_filter = match_path_filter(&project.path_filter, &files_changed);

    if !matches_filter {
        info!(
            "[{}] Changed files do not match path filter: {}",
            trace_id, project.path_filter
        );
        ctx.logger.api_exit(&trace_id, "POST", "/webhook/github", timer.elapsed_ms(), "200");
        return (
            StatusCode::OK,
            Json(WebhookResponse {
                message: "Files do not match filter".to_string(),
                build_id: None,
            }),
        );
    }

    // Create build
    let create_build = CreateBuild {
        project_id: project.id,
        commit_hash: head_commit.id.clone(),
        commit_message: Some(head_commit.message.clone()),
        author: Some(format!("{} <{}>", head_commit.author.name, head_commit.author.email)),
    };

    let build = match ctx.build_repo.create(create_build).await {
        Ok(b) => b,
        Err(e) => {
            warn!("[{}] Failed to create build: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "POST", "/webhook/github", timer.elapsed_ms(), "500");
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(WebhookResponse {
                    message: "Failed to create build".to_string(),
                    build_id: None,
                }),
            );
        }
    };

    info!(
        "[{}] Created build #{} for project {}",
        trace_id, build.build_number, project.name
    );

    // Enqueue build
    ctx.build_queue.enqueue(project.id, build.id).await;

    // Emit event
    ctx.event_bus.emit(Event::BuildStatus {
        build_id: build.id,
        project_id: project.id,
        status: BuildStatus::Queued,
        timestamp: Event::now(),
    }).await;

    ctx.logger.api_exit(&trace_id, "POST", "/webhook/github", timer.elapsed_ms(), "200");

    (
        StatusCode::OK,
        Json(WebhookResponse {
            message: format!("Build #{} queued", build.build_number),
            build_id: Some(build.id),
        }),
    )
}

fn match_path_filter(pattern: &str, files: &[String]) -> bool {
    // Support comma-separated patterns: "src/**,tests/**"
    let patterns: Vec<&str> = pattern.split(',').map(|s| s.trim()).collect();

    // Build GlobSet from patterns
    let mut builder = GlobSetBuilder::new();
    for pat in patterns {
        if let Ok(glob) = Glob::new(pat) {
            builder.add(glob);
        } else {
            warn!("Invalid glob pattern: {}", pat);
            return false;
        }
    }

    let globset = match builder.build() {
        Ok(gs) => gs,
        Err(e) => {
            warn!("Failed to build GlobSet: {}", e);
            return false;
        }
    };

    // Check if any file matches any pattern
    files.iter().any(|f| globset.is_match(f))
}

async fn verify_signature(ctx: &AppContext, headers: &HeaderMap, body: &str) -> Result<(), String> {
    // Get webhook secret from database
    let secret = ctx.settings_repo.get("webhook_secret")
        .await
        .map_err(|e| format!("Failed to get webhook secret: {}", e))?
        .ok_or("Webhook secret not configured")?;

    // Get signature from header
    let signature_header = headers
        .get("x-hub-signature-256")
        .and_then(|v| v.to_str().ok())
        .ok_or("Missing signature header")?;

    // Extract signature (format: sha256=...)
    let signature = signature_header
        .strip_prefix("sha256=")
        .ok_or("Invalid signature format")?;

    // Compute HMAC
    let mut mac = HmacSha256::new_from_slice(secret.as_bytes())
        .map_err(|e| format!("Invalid key: {}", e))?;
    mac.update(body.as_bytes());

    // Compare
    let expected = hex::encode(mac.finalize().into_bytes());

    if signature != expected {
        return Err("Signature mismatch".to_string());
    }

    Ok(())
}
