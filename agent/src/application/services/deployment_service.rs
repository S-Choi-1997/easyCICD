use anyhow::{Context, Result};
use reqwest::Client;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::application::ports::repositories::{BuildRepository, ProjectRepository};
use crate::application::events::{EventBus, Event};
use crate::db::models::{BuildStatus, Project, Build, Slot};
use crate::docker::DockerClient;
use crate::infrastructure::logging::{BoundaryLogger, Timer};

/// DeploymentService - 배포 및 헬스체크를 담당하는 서비스
///
/// 책임:
/// - Blue-Green 배포 전략 실행
/// - 컨테이너 시작 및 헬스체크
/// - 배포 로그 기록
/// - 슬롯 전환 관리
/// - 이벤트 발행
pub struct DeploymentService<BR, PR, EB>
where
    BR: BuildRepository,
    PR: ProjectRepository,
    EB: EventBus,
{
    build_repo: Arc<BR>,
    project_repo: Arc<PR>,
    event_bus: EB,
    docker: DockerClient,
    logger: Arc<BoundaryLogger>,
}

impl<BR, PR, EB> DeploymentService<BR, PR, EB>
where
    BR: BuildRepository,
    PR: ProjectRepository,
    EB: EventBus,
{
    pub fn new(
        build_repo: Arc<BR>,
        project_repo: Arc<PR>,
        event_bus: EB,
        docker: DockerClient,
        logger: Arc<BoundaryLogger>,
    ) -> Self {
        Self {
            build_repo,
            project_repo,
            event_bus,
            docker,
            logger,
        }
    }

    /// Blue-Green 배포 실행
    pub async fn deploy(&self, trace_id: &str, project: &Project, build: &Build, output_path: PathBuf) -> Result<()> {
        let timer = Timer::start();
        self.logger.service_entry(trace_id, "API", "DeploymentService", "deploy", &build.id);

        info!(
            "[{}] Deploying build #{} for project {}",
            trace_id, build.build_number, project.name
        );

        // Open deploy log file
        let deploy_log_path = build.deploy_log_path.as_ref()
            .context("Deploy log path not set")?;
        let deploy_log_path_buf = PathBuf::from(deploy_log_path);

        // Create log directory if needed
        if let Some(parent) = deploy_log_path_buf.parent() {
            fs::create_dir_all(parent).await
                .context("Failed to create deploy log directory")?;
        }

        let mut log_file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&deploy_log_path_buf)
            .await
            .context("Failed to open deploy log file")?;

        // Helper macro to write log
        macro_rules! write_log {
            ($msg:expr) => {
                {
                    let message = format!("[DEPLOY] {}\n", $msg);
                    log_file.write_all(message.as_bytes()).await.ok();
                    log_file.flush().await.ok();
                }
            };
        }

        write_log!(format!("Starting deployment for build #{}", build.build_number));

        // Build is already successful at this point, now starting deployment
        // Emit deployment status event
        self.logger.event_emit(trace_id, "DeploymentService", "Deployment::Deploying");
        self.event_bus.emit(Event::Deployment {
            project_id: project.id,
            project_name: project.name.clone(),
            build_id: build.id,
            status: "deploying".to_string(),
            slot: project.get_inactive_slot(),
            url: format!("http://{}:{}", "localhost", project.get_inactive_port()),
            timestamp: Event::now(),
        }).await;

        // Determine target slot (opposite of active)
        let target_slot = match project.active_slot {
            Slot::Blue => Slot::Green,
            Slot::Green => Slot::Blue,
        };

        let target_port = match target_slot {
            Slot::Blue => project.blue_port as u16,
            Slot::Green => project.green_port as u16,
        };

        info!(
            "[{}] Deploying to {} slot on port {}",
            trace_id, target_slot, target_port
        );
        write_log!(format!("Deploying to {} slot on port {}", target_slot, target_port));

        // Clean up target slot's old container if it exists
        let old_container_id = match target_slot {
            Slot::Blue => project.blue_container_id.as_ref(),
            Slot::Green => project.green_container_id.as_ref(),
        };

        if let Some(old_id) = old_container_id {
            self.logger.external_call(trace_id, "DeploymentService", "Docker", "is_container_running");
            if self.docker.is_container_running(old_id).await {
                info!("[{}] Stopping old {} container: {}", trace_id, target_slot, old_id);
                write_log!(format!("Stopping old {} container: {}", target_slot, old_id));

                self.logger.external_call(trace_id, "DeploymentService", "Docker", "stop_container");
                self.docker.stop_container(old_id).await.ok();
            } else {
                info!("[{}] Old {} container {} not found, skipping cleanup", trace_id, target_slot, old_id);
                write_log!(format!("Old {} container {} not found, skipping cleanup", target_slot, old_id));
            }

            self.logger.external_call(trace_id, "DeploymentService", "Docker", "remove_container");
            self.docker.remove_container(old_id).await.ok();

            // Clear from database
            self.logger.repo_call(trace_id, "DeploymentService", "ProjectRepo", &format!("update_{}_container", target_slot.to_string().to_lowercase()));
            match target_slot {
                Slot::Blue => {
                    self.project_repo.update_blue_container(project.id, None).await?;
                }
                Slot::Green => {
                    self.project_repo.update_green_container(project.id, None).await?;
                }
            }
        }

        // Start runtime container
        write_log!(format!("Starting runtime container with image: {}", project.runtime_image));

        self.logger.external_call(trace_id, "DeploymentService", "Docker", "run_runtime_container");
        let docker_timer = Timer::start();

        let container_id = self
            .docker
            .run_runtime_container(
                &project.runtime_image,
                &project.runtime_command,
                output_path,
                target_port,
                project.runtime_port as u16,
                project.id,
                &target_slot.to_string().to_lowercase(),
                project.runtime_env_vars.as_deref(),
            )
            .await
            .context("Failed to start runtime container")?;

        self.logger.external_done(trace_id, "DeploymentService", "Docker", "run_runtime_container", docker_timer.elapsed_ms());

        info!("[{}] Runtime container started: {}", trace_id, container_id);
        write_log!(format!("Runtime container started: {}", container_id));

        // Update container ID in database
        self.logger.repo_call(trace_id, "DeploymentService", "ProjectRepo", &format!("update_{}_container", target_slot.to_string().to_lowercase()));
        match target_slot {
            Slot::Blue => {
                self.project_repo
                    .update_blue_container(project.id, Some(container_id.clone()))
                    .await?;
            }
            Slot::Green => {
                self.project_repo
                    .update_green_container(project.id, Some(container_id.clone()))
                    .await?;
            }
        }

        // Perform health check (container running + HTTP response)
        let health_check_result = self
            .perform_health_check(trace_id, project, build, &container_id, target_port)
            .await;

        match health_check_result {
            Ok(_) => {
                info!("[{}] Health check passed, switching to {} slot", trace_id, target_slot);
                write_log!(format!("Health check passed, switching to {} slot", target_slot));

                let deployed_slot_str = target_slot.to_string();

                // Switch active slot
                self.logger.repo_call(trace_id, "DeploymentService", "ProjectRepo", "update_active_slot");
                self.project_repo
                    .update_active_slot(project.id, target_slot)
                    .await?;

                // Update build status to Success
                self.logger.repo_call(trace_id, "DeploymentService", "BuildRepo", "finish");
                self.build_repo
                    .finish(build.id, BuildStatus::Success)
                    .await?;

                self.logger.repo_call(trace_id, "DeploymentService", "BuildRepo", "update_deployed_slot");
                self.build_repo
                    .update_deployed_slot(build.id, Some(deployed_slot_str.clone()))
                    .await?;

                self.logger.event_emit(trace_id, "DeploymentService", "Deployment::Success");
                self.event_bus.emit(Event::Deployment {
                    project_id: project.id,
                    project_name: project.name.clone(),
                    build_id: build.id,
                    status: "Success".to_string(),
                    slot: target_slot,
                    url: format!("https://app.yourdomain.com/{}/", project.name),
                    timestamp: Event::now(),
                }).await;

                self.logger.event_emit(trace_id, "DeploymentService", "BuildStatus::Success");
                self.event_bus.emit(Event::BuildStatus {
                    build_id: build.id,
                    project_id: project.id,
                    status: BuildStatus::Success,
                    timestamp: Event::now(),
                }).await;

                // Stop old container (the previous active slot, opposite of target_slot)
                let old_slot = project.active_slot;
                let old_container_id = match old_slot {
                    Slot::Blue => project.blue_container_id.clone(),
                    Slot::Green => project.green_container_id.clone(),
                };

                if let Some(old_id) = old_container_id {
                    info!("[{}] Stopping old {} container: {}", trace_id, old_slot, old_id);
                    write_log!(format!("Stopping old {} container: {}", old_slot, old_id));

                    self.logger.external_call(trace_id, "DeploymentService", "Docker", "stop_container");
                    self.docker.stop_container(&old_id).await.ok();

                    self.logger.external_call(trace_id, "DeploymentService", "Docker", "remove_container");
                    self.docker.remove_container(&old_id).await.ok();

                    // Clear old container ID from database
                    self.logger.repo_call(trace_id, "DeploymentService", "ProjectRepo", &format!("update_{}_container", old_slot.to_string().to_lowercase()));
                    match old_slot {
                        Slot::Blue => {
                            self.project_repo
                                .update_blue_container(project.id, None)
                                .await?;
                        }
                        Slot::Green => {
                            self.project_repo
                                .update_green_container(project.id, None)
                                .await?;
                        }
                    }
                }

                write_log!("Deployment completed successfully");
                self.logger.service_exit(trace_id, "API", "DeploymentService", "deploy", timer.elapsed_ms());
                Ok(())
            }
            Err(e) => {
                warn!("[{}] Health check failed: {}", trace_id, e);
                write_log!(format!("Health check failed: {}", e));

                self.logger.event_emit(trace_id, "DeploymentService", "Deployment::Failed");
                self.event_bus.emit(Event::Deployment {
                    project_id: project.id,
                    project_name: project.name.clone(),
                    build_id: build.id,
                    status: "Failed".to_string(),
                    slot: target_slot,
                    url: format!("https://app.yourdomain.com/{}/", project.name),
                    timestamp: Event::now(),
                }).await;

                self.logger.event_emit(trace_id, "DeploymentService", "BuildStatus::Failed");
                self.event_bus.emit(Event::BuildStatus {
                    build_id: build.id,
                    project_id: project.id,
                    status: BuildStatus::Failed,
                    timestamp: Event::now(),
                }).await;

                self.logger.event_emit(trace_id, "DeploymentService", "Error");
                self.event_bus.emit(Event::Error {
                    project_id: Some(project.id),
                    build_id: Some(build.id),
                    message: format!("Health check failed: {}", e),
                    timestamp: Event::now(),
                }).await;

                write_log!(format!("Deployment failed: {}", e));
                self.logger.service_error(trace_id, "API", "DeploymentService", "deploy", &e);
                anyhow::bail!("Deployment failed: {}", e);
            }
        }
    }

    /// 컨테이너 헬스체크 수행 (컨테이너 실행 + HTTP 응답 확인)
    async fn perform_health_check(
        &self,
        trace_id: &str,
        project: &Project,
        build: &Build,
        container_id: &str,
        _target_port: u16,
    ) -> Result<()> {
        let max_attempts = 10;
        let retry_interval = Duration::from_secs(2);

        info!("[{}] Starting health check for container: {}", trace_id, container_id);

        // health_check_url이 비어있거나 "/" 또는 "disabled"면 HTTP 체크 스킵
        // 컨테이너 실행 여부만 확인
        let skip_http_check = project.health_check_url.is_empty()
            || project.health_check_url == "/"
            || project.health_check_url.to_lowercase() == "disabled"
            || project.health_check_url.to_lowercase() == "none";

        if skip_http_check {
            info!("[{}] HTTP health check disabled, checking container running status only", trace_id);
        }

        // 컨테이너 이름으로 헬스체크 URL 구성 (Docker 내부 네트워크에서 접근 가능)
        let container_name = format!("project-{}-{}", project.id,
            if project.active_slot == crate::db::models::Slot::Blue { "green" } else { "blue" });
        let runtime_port = project.runtime_port as u16;

        let health_url = if skip_http_check {
            "container-only".to_string()
        } else {
            let health_path = if project.health_check_url.starts_with('/') {
                project.health_check_url.clone()
            } else {
                format!("/{}", project.health_check_url)
            };
            format!("http://{}:{}{}", container_name, runtime_port, health_path)
        };

        info!("[{}] Health check URL: {}", trace_id, health_url);

        // HTTP client (skip_http_check이 false일 때만 사용)
        let http_client = if !skip_http_check {
            Some(Client::builder()
                .timeout(Duration::from_secs(5))
                .build()
                .context("Failed to create HTTP client")?)
        } else {
            None
        };

        for attempt in 1..=max_attempts {
            self.logger.event_emit(trace_id, "DeploymentService", &format!("HealthCheck::Attempt{}/{}", attempt, max_attempts));
            self.event_bus.emit(Event::HealthCheck {
                project_id: project.id,
                build_id: build.id,
                attempt,
                max_attempts,
                status: "Checking".to_string(),
                url: health_url.clone(),
                timestamp: Event::now(),
            }).await;

            // Step 1: Check if container is still running
            self.logger.external_call(trace_id, "DeploymentService", "Docker", "is_container_running");
            let is_running = self.docker.is_container_running(&container_id).await;

            if !is_running {
                warn!(
                    "[{}] Container {} stopped unexpectedly on attempt {}/{}",
                    trace_id, container_id, attempt, max_attempts
                );

                // Container died - get logs for debugging
                if let Ok(logs) = self.docker.get_container_logs(container_id, Some(50)).await {
                    let last_logs: String = logs.into_iter().take(20).collect::<Vec<_>>().join("\n");
                    warn!("[{}] Container logs before exit:\n{}", trace_id, last_logs);
                }

                self.event_bus.emit(Event::HealthCheck {
                    project_id: project.id,
                    build_id: build.id,
                    attempt,
                    max_attempts,
                    status: "ContainerStopped".to_string(),
                    url: health_url.clone(),
                    timestamp: Event::now(),
                }).await;

                anyhow::bail!("Container stopped unexpectedly during health check");
            }

            // HTTP 체크 스킵 시 컨테이너가 실행 중이면 바로 성공
            if skip_http_check {
                info!(
                    "[{}] Health check passed (container running) on attempt {}/{}",
                    trace_id, attempt, max_attempts
                );

                self.logger.event_emit(trace_id, "DeploymentService", "HealthCheck::Success");
                self.event_bus.emit(Event::HealthCheck {
                    project_id: project.id,
                    build_id: build.id,
                    attempt,
                    max_attempts,
                    status: "Success".to_string(),
                    url: health_url.clone(),
                    timestamp: Event::now(),
                }).await;

                return Ok(());
            }

            // Step 2: HTTP health check (only if not skipped)
            if let Some(ref client) = http_client {
                self.logger.external_call(trace_id, "DeploymentService", "HTTP", "health_check");

                match client.get(&health_url).send().await {
                    Ok(response) => {
                        let status = response.status();

                        // 2xx, 3xx 응답은 모두 성공으로 처리
                        if status.is_success() || status.is_redirection() {
                            info!(
                                "[{}] Health check passed on attempt {}/{} - HTTP {} from {}",
                                trace_id, attempt, max_attempts, status, health_url
                            );

                            self.logger.event_emit(trace_id, "DeploymentService", "HealthCheck::Success");
                            self.event_bus.emit(Event::HealthCheck {
                                project_id: project.id,
                                build_id: build.id,
                                attempt,
                                max_attempts,
                                status: "Success".to_string(),
                                url: health_url.clone(),
                                timestamp: Event::now(),
                            }).await;

                            return Ok(());
                        } else {
                            warn!(
                                "[{}] Health check attempt {}/{} - HTTP {} (not 2xx/3xx)",
                                trace_id, attempt, max_attempts, status
                            );
                        }
                    }
                    Err(e) => {
                        warn!(
                            "[{}] Health check attempt {}/{} - Connection error: {}",
                            trace_id, attempt, max_attempts, e
                        );
                    }
                }
            }

            if attempt < max_attempts {
                sleep(retry_interval).await;
            }
        }

        // All attempts failed - get final container logs
        if let Ok(logs) = self.docker.get_container_logs(container_id, Some(50)).await {
            let last_logs: String = logs.into_iter().take(30).collect::<Vec<_>>().join("\n");
            warn!("[{}] Final container logs:\n{}", trace_id, last_logs);
        }

        self.logger.event_emit(trace_id, "DeploymentService", "HealthCheck::Failed");
        self.event_bus.emit(Event::HealthCheck {
            project_id: project.id,
            build_id: build.id,
            attempt: max_attempts,
            max_attempts,
            status: "Failed".to_string(),
            url: health_url.clone(),
            timestamp: Event::now(),
        }).await;

        anyhow::bail!(
            "Health check failed after {} attempts. URL: {} - Application may have failed to start.",
            max_attempts, health_url
        )
    }

    /// 이전 빌드로 롤백
    pub async fn rollback(&self, trace_id: &str, project: &Project, target_build: &Build) -> Result<()> {
        let timer = Timer::start();
        self.logger.service_entry(trace_id, "API", "DeploymentService", "rollback", &target_build.id);

        info!(
            "[{}] Rolling back project {} to build #{}",
            trace_id, project.name, target_build.build_number
        );

        // 롤백 대상 빌드가 배포되었는지 확인
        let target_slot = target_build.get_deployed_slot()
            .context("Target build was never deployed")?;

        // output_path 확인
        let output_path = target_build.output_path.as_ref()
            .context("Target build has no output path")?;
        let output_path_buf = PathBuf::from(output_path);

        if !output_path_buf.exists() {
            anyhow::bail!("Target build output not found: {}", output_path);
        }

        info!(
            "[{}] Target build was deployed to {} slot, output path: {}",
            trace_id, target_slot, output_path
        );

        // 현재 활성 슬롯이 아닌 슬롯에 배포
        let deploy_slot = match project.active_slot {
            Slot::Blue => Slot::Green,
            Slot::Green => Slot::Blue,
        };

        let deploy_port = match deploy_slot {
            Slot::Blue => project.blue_port as u16,
            Slot::Green => project.green_port as u16,
        };

        info!("[{}] Deploying rollback to {} slot on port {}", trace_id, deploy_slot, deploy_port);

        // 기존 컨테이너 정리
        let old_container_id = match deploy_slot {
            Slot::Blue => project.blue_container_id.as_ref(),
            Slot::Green => project.green_container_id.as_ref(),
        };

        if let Some(old_id) = old_container_id {
            self.logger.external_call(trace_id, "DeploymentService", "Docker", "stop_container");
            self.docker.stop_container(old_id).await.ok();

            self.logger.external_call(trace_id, "DeploymentService", "Docker", "remove_container");
            self.docker.remove_container(old_id).await.ok();

            match deploy_slot {
                Slot::Blue => {
                    self.project_repo.update_blue_container(project.id, None).await?;
                }
                Slot::Green => {
                    self.project_repo.update_green_container(project.id, None).await?;
                }
            }
        }

        // 이전 빌드의 컨테이너 시작
        self.logger.external_call(trace_id, "DeploymentService", "Docker", "run_runtime_container");
        let container_id = self
            .docker
            .run_runtime_container(
                &project.runtime_image,
                &project.runtime_command,
                output_path_buf,
                deploy_port,
                project.runtime_port as u16,
                project.id,
                &deploy_slot.to_string().to_lowercase(),
                project.runtime_env_vars.as_deref(),
            )
            .await
            .context("Failed to start rollback container")?;

        info!("[{}] Rollback container started: {}", trace_id, container_id);

        // 컨테이너 ID 업데이트
        match deploy_slot {
            Slot::Blue => {
                self.project_repo
                    .update_blue_container(project.id, Some(container_id.clone()))
                    .await?;
            }
            Slot::Green => {
                self.project_repo
                    .update_green_container(project.id, Some(container_id.clone()))
                    .await?;
            }
        }

        // Health check (container running + HTTP response)
        self.perform_health_check(trace_id, project, target_build, &container_id, deploy_port).await?;

        info!("[{}] Health check passed, switching to {} slot", trace_id, deploy_slot);

        // 슬롯 전환
        self.project_repo
            .update_active_slot(project.id, deploy_slot)
            .await?;

        // 이전 활성 슬롯 정리
        let old_slot = project.active_slot;
        let old_active_container_id = match old_slot {
            Slot::Blue => project.blue_container_id.clone(),
            Slot::Green => project.green_container_id.clone(),
        };

        if let Some(old_id) = old_active_container_id {
            info!("[{}] Stopping old {} container: {}", trace_id, old_slot, old_id);
            self.docker.stop_container(&old_id).await.ok();
            self.docker.remove_container(&old_id).await.ok();

            match old_slot {
                Slot::Blue => {
                    self.project_repo.update_blue_container(project.id, None).await?;
                }
                Slot::Green => {
                    self.project_repo.update_green_container(project.id, None).await?;
                }
            }
        }

        self.logger.event_emit(trace_id, "DeploymentService", "Rollback::Success");
        self.event_bus.emit(Event::Deployment {
            project_id: project.id,
            project_name: project.name.clone(),
            build_id: target_build.id,
            status: "Rollback Success".to_string(),
            slot: deploy_slot,
            url: format!("https://app.yourdomain.com/{}/", project.name),
            timestamp: Event::now(),
        }).await;

        info!("[{}] Rollback completed successfully", trace_id);
        self.logger.service_exit(trace_id, "API", "DeploymentService", "rollback", timer.elapsed_ms());

        Ok(())
    }
}
