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
use crate::application::ports::repositories::{ProjectRepository, BuildRepository, SettingsRepository};
use crate::application::events::event_bus::EventBus;
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
        ctx.logger.api_exit(&trace_id, "POST", "/webhook/github", timer.elapsed_ms(), 401);
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
            ctx.logger.api_exit(&trace_id, "POST", "/webhook/github", timer.elapsed_ms(), 400);
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

    // Find matching projects (can be multiple for same repo)
    let projects: Vec<crate::db::models::Project> = match ctx.project_repo.list().await {
        Ok(p) => p,
        Err(e) => {
            warn!("[{}] Failed to list projects: {}", trace_id, e);
            ctx.logger.api_exit(&trace_id, "POST", "/webhook/github", timer.elapsed_ms(), 500);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(WebhookResponse {
                    message: "Internal error".to_string(),
                    build_id: None,
                }),
            );
        }
    };

    let matching_projects: Vec<&crate::db::models::Project> = projects.iter().filter(|p| {
        // Extract owner/repo from stored URL (e.g., https://github.com/owner/repo.git -> owner/repo)
        let repo_path = p.repo
            .trim_end_matches(".git")
            .trim_end_matches('/')
            .split("github.com/")
            .nth(1)
            .unwrap_or("");

        !repo_path.is_empty() && repo_path == webhook.repository.full_name && p.branch == branch
    }).collect();

    if matching_projects.is_empty() {
        info!(
            "[{}] No matching project found for repo {} branch {}",
            trace_id, webhook.repository.full_name, branch
        );
        ctx.logger.api_exit(&trace_id, "POST", "/webhook/github", timer.elapsed_ms(), 200);
        return (
            StatusCode::OK,
            Json(WebhookResponse {
                message: "No matching project".to_string(),
                build_id: None,
            }),
        );
    }

    // Check if files match path_filter
    let head_commit = match webhook.head_commit {
        Some(c) => c,
        None => {
            info!("[{}] No head commit in webhook", trace_id);
            ctx.logger.api_exit(&trace_id, "POST", "/webhook/github", timer.elapsed_ms(), 200);
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

    // Process each matching project
    let mut build_ids = Vec::new();
    let mut project_names = Vec::new();

    for project in matching_projects {
        let matches_filter = match_path_filter(&project.path_filter, &files_changed);

        if !matches_filter {
            info!(
                "[{}] Changed files do not match path filter for project {}: {}",
                trace_id, project.name, project.path_filter
            );
            continue;
        }

        // Create build for this project
        let create_build = CreateBuild {
            project_id: project.id,
            commit_hash: head_commit.id.clone(),
            commit_message: Some(head_commit.message.clone()),
            author: Some(format!("{} <{}>", head_commit.author.name, head_commit.author.email)),
        };

        let build = match ctx.build_repo.create(create_build).await {
            Ok(b) => b,
            Err(e) => {
                warn!("[{}] Failed to create build for project {}: {}", trace_id, project.name, e);
                continue;
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

        build_ids.push(build.id);
        project_names.push(project.name.clone());
    }

    ctx.logger.api_exit(&trace_id, "POST", "/webhook/github", timer.elapsed_ms(), 200);

    // Return response
    if build_ids.is_empty() {
        (
            StatusCode::OK,
            Json(WebhookResponse {
                message: "No matching path filters".to_string(),
                build_id: None,
            }),
        )
    } else if build_ids.len() == 1 {
        (
            StatusCode::OK,
            Json(WebhookResponse {
                message: format!("Build queued for {}", project_names[0]),
                build_id: Some(build_ids[0]),
            }),
        )
    } else {
        (
            StatusCode::OK,
            Json(WebhookResponse {
                message: format!("Builds queued for: {}", project_names.join(", ")),
                build_id: Some(build_ids[0]), // Return first build ID for backward compatibility
            }),
        )
    }
}

fn match_path_filter(pattern: &str, files: &[String]) -> bool {
    // Empty pattern or "*" means match all files
    if pattern.is_empty() || pattern.trim() == "*" {
        return true;
    }

    // Support comma-separated patterns: "src/**,tests/**"
    let patterns: Vec<&str> = pattern.split(',').map(|s| s.trim()).collect();

    // Build GlobSet from patterns
    let mut builder = GlobSetBuilder::new();
    for pat in patterns {
        // "frontend" 같은 디렉토리 패턴은 "frontend/**"로 자동 변환
        let expanded_pat = if !pat.contains('*') && !pat.contains('?') {
            format!("{}/**", pat.trim_end_matches('/'))
        } else {
            pat.to_string()
        };

        if let Ok(glob) = Glob::new(&expanded_pat) {
            builder.add(glob);
        } else {
            warn!("Invalid glob pattern: {}", expanded_pat);
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
    let secret_opt: Option<String> = ctx.settings_repo.get("webhook_secret")
        .await
        .map_err(|e| format!("Failed to get webhook secret: {}", e))?;
    let secret = secret_opt.ok_or("Webhook secret not configured")?;

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
