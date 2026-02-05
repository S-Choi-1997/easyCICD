use anyhow::Result;
use std::sync::Arc;
use tokio::sync::broadcast;
use tracing::{debug, error, info, warn};

use crate::application::events::Event;
use crate::application::ports::repositories::{BuildRepository, ProjectRepository};
use crate::db::models::BuildStatus;
use crate::infrastructure::notifications::DiscordClient;
use crate::infrastructure::database::{SqliteBuildRepository, SqliteProjectRepository};

/// Discord 알림 설정
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DiscordWebhookConfig {
    pub id: i64,
    pub label: String,
    pub webhook_url: String,
    pub enabled: bool,
    pub notify_on_build_start: bool,
    pub notify_on_build_success: bool,
    pub notify_on_build_failure: bool,
    pub notify_on_deploy_start: bool,
    pub notify_on_deploy_success: bool,
    pub notify_on_deploy_failure: bool,
    pub mention_user_ids: Vec<String>,
    pub mention_role_ids: Vec<String>,
    pub mention_on_failure_only: bool,
}

impl DiscordWebhookConfig {
    /// 멘션 문자열 생성
    pub fn get_mentions(&self, is_failure: bool) -> Vec<String> {
        if !is_failure && self.mention_on_failure_only {
            return Vec::new();
        }

        let mut mentions = Vec::new();

        // 사용자 멘션: <@user_id>
        for user_id in &self.mention_user_ids {
            mentions.push(format!("<@{}>", user_id));
        }

        // 역할 멘션: <@&role_id>
        for role_id in &self.mention_role_ids {
            mentions.push(format!("<@&{}>", role_id));
        }

        mentions
    }
}

/// Discord 알림 워커
pub async fn run_discord_notifier(
    webhook_repo: Arc<crate::infrastructure::database::SqliteDiscordWebhookRepository>,
    build_repo: Arc<SqliteBuildRepository>,
    project_repo: Arc<SqliteProjectRepository>,
    mut event_rx: broadcast::Receiver<Event>,
    base_url: Option<String>,
) -> Result<()> {
    info!("Starting Discord notifier");

    let discord_client = DiscordClient::new();
    let base_url = base_url.unwrap_or_else(|| "http://localhost:10000".to_string());

    loop {
        match event_rx.recv().await {
            Ok(event) => {
                // 이벤트 타입별 처리 - 프로젝트별 웹훅 필터링
                if let Err(e) = handle_event(
                    &discord_client,
                    &event,
                    &webhook_repo,
                    &build_repo,
                    &project_repo,
                    &base_url,
                )
                .await
                {
                    error!("Failed to send Discord notification: {}", e);
                }
            }
            Err(broadcast::error::RecvError::Lagged(skipped)) => {
                warn!("Discord notifier lagged, skipped {} events", skipped);
            }
            Err(broadcast::error::RecvError::Closed) => {
                info!("Event bus closed, Discord notifier stopping");
                break;
            }
        }
    }

    Ok(())
}

async fn handle_event(
    client: &DiscordClient,
    event: &Event,
    webhook_repo: &crate::infrastructure::database::SqliteDiscordWebhookRepository,
    build_repo: &SqliteBuildRepository,
    project_repo: &SqliteProjectRepository,
    base_url: &str,
) -> Result<()> {
    match event {
        Event::BuildStatus {
            build_id,
            project_id,
            status,
            ..
        } => {
            // Repository를 통해 빌드 및 프로젝트 정보 가져오기
            let build = build_repo
                .get(*build_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Build not found"))?;

            let project = project_repo
                .get(*project_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Project not found"))?;

            // 프로젝트에 Discord webhook이 설정되어 있지 않으면 스킵
            let webhook_id = match project.discord_webhook_id {
                Some(id) => id,
                None => {
                    debug!("Project {} has no Discord webhook configured", project.name);
                    return Ok(());
                }
            };

            // Webhook 설정 로드
            let config = match webhook_repo.get(webhook_id).await? {
                Some(c) => c,
                None => {
                    warn!("Discord webhook {} not found for project {}", webhook_id, project.name);
                    return Ok(());
                }
            };

            // Webhook이 비활성화되어 있으면 스킵
            if !config.enabled {
                debug!("Discord webhook '{}' is disabled", config.label);
                return Ok(());
            }

            let build_url = format!("{}/builds/{}", base_url, build_id);

            match status {
                BuildStatus::Building => {
                    if config.notify_on_build_start {
                        let message = client.build_started_message(
                            &project.name,
                            build.build_number,
                            &project.branch,
                            &build.commit_hash,
                            build.author.as_deref(),
                        );
                        client.send_message(&config.webhook_url, message).await?;
                    }
                }
                BuildStatus::Success => {
                    if config.notify_on_build_success {
                        let duration = build
                            .finished_at
                            .as_deref()
                            .and_then(|finished| {
                                let started =
                                    chrono::DateTime::parse_from_rfc3339(&build.started_at).ok()?;
                                let finished = chrono::DateTime::parse_from_rfc3339(finished).ok()?;
                                Some((finished - started).num_seconds() as u64)
                            })
                            .unwrap_or(0);

                        let message = client.build_success_message(
                            &project.name,
                            build.build_number,
                            &project.branch,
                            duration,
                            Some(&build_url),
                        );
                        client.send_message(&config.webhook_url, message).await?;
                    }
                }
                BuildStatus::Failed => {
                    if config.notify_on_build_failure {
                        let mentions = config.get_mentions(true);
                        let message = client.build_failure_message(
                            &project.name,
                            build.build_number,
                            &project.branch,
                            Some("빌드 실패 - 로그를 확인하세요"),
                            Some(&build_url),
                            mentions,
                        );
                        client.send_message(&config.webhook_url, message).await?;
                    }
                }
                _ => {}
            }
        }

        Event::Deployment {
            project_id,
            project_name,
            build_id,
            status,
            slot,
            url,
            ..
        } => {
            // 프로젝트 정보 가져오기
            let project = project_repo
                .get(*project_id)
                .await?
                .ok_or_else(|| anyhow::anyhow!("Project not found"))?;

            // 프로젝트에 Discord webhook이 설정되어 있지 않으면 스킵
            let webhook_id = match project.discord_webhook_id {
                Some(id) => id,
                None => {
                    debug!("Project {} has no Discord webhook configured", project.name);
                    return Ok(());
                }
            };

            // Webhook 설정 로드
            let config = match webhook_repo.get(webhook_id).await? {
                Some(c) => c,
                None => {
                    warn!("Discord webhook {} not found for project {}", webhook_id, project.name);
                    return Ok(());
                }
            };

            // Webhook이 비활성화되어 있으면 스킵
            if !config.enabled {
                debug!("Discord webhook '{}' is disabled", config.label);
                return Ok(());
            }

            match status.as_str() {
                "deploying" => {
                    if config.notify_on_deploy_start {
                        // 배포 시작 알림 (선택적)
                    }
                }
                "Success" => {
                    if config.notify_on_deploy_success {
                        let build = build_repo
                            .get(*build_id)
                            .await?
                            .ok_or_else(|| anyhow::anyhow!("Build not found"))?;

                        let message = client.deployment_success_message(
                            project_name,
                            build.build_number,
                            &slot.to_string(),
                            Some(url),
                        );
                        client.send_message(&config.webhook_url, message).await?;
                    }
                }
                status if status.contains("fail") || status.contains("Fail") => {
                    if config.notify_on_deploy_failure {
                        let build = build_repo
                            .get(*build_id)
                            .await?
                            .ok_or_else(|| anyhow::anyhow!("Build not found"))?;

                        let mentions = config.get_mentions(true);
                        let message = client.deployment_failure_message(
                            project_name,
                            build.build_number,
                            Some("배포 실패 - 로그를 확인하세요"),
                            mentions,
                        );
                        client.send_message(&config.webhook_url, message).await?;
                    }
                }
                _ => {}
            }
        },

        _ => {
            // 다른 이벤트는 무시
        }
    }

    Ok(())
}
