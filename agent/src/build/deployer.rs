use anyhow::{Context, Result};
use std::path::PathBuf;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::db::models::{Build, BuildStatus, Project, Slot};
use crate::docker::DockerClient;
use crate::events::Event;
use crate::state::AppState;

pub struct Deployer {
    pub state: AppState,
    pub docker: DockerClient,
}

impl Deployer {
    pub fn new(state: AppState, docker: DockerClient) -> Self {
        Self { state, docker }
    }

    pub async fn deploy(&self, project: &Project, build: &Build, output_path: PathBuf) -> Result<()> {
        info!(
            "Deploying build #{} for project {}",
            build.build_number, project.name
        );

        // Update status to Deploying
        self.state
            .db
            .update_build_status(build.id, BuildStatus::Deploying)
            .await?;

        self.state.emit_event(Event::BuildStatus {
            build_id: build.id,
            project_id: project.id,
            status: BuildStatus::Deploying,
            timestamp: Event::now(),
        });

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
            "Deploying to {} slot on port {}",
            target_slot, target_port
        );

        // Start runtime container
        let container_id = self
            .docker
            .run_runtime_container(
                &project.runtime_image,
                &project.runtime_command,
                output_path,
                target_port,
                &project.name,
                &target_slot.to_string().to_lowercase(),
            )
            .await
            .context("Failed to start runtime container")?;

        info!("Runtime container started: {}", container_id);

        // Update container ID in database
        match target_slot {
            Slot::Blue => {
                self.state
                    .db
                    .update_project_blue_container(project.id, Some(container_id.clone()))
                    .await?;
            }
            Slot::Green => {
                self.state
                    .db
                    .update_project_green_container(project.id, Some(container_id.clone()))
                    .await?;
            }
        }

        // Perform health check
        let health_check_result = self
            .perform_health_check(project, build, target_port)
            .await;

        match health_check_result {
            Ok(_) => {
                info!("Health check passed, switching to {} slot", target_slot);

                let deployed_slot_str = target_slot.to_string();

                // Switch active slot
                self.state
                    .db
                    .update_project_active_slot(project.id, target_slot)
                    .await?;

                // Update build status to Success
                self.state
                    .db
                    .finish_build(build.id, BuildStatus::Success)
                    .await?;

                self.state
                    .db
                    .update_build_deployed_slot(build.id, Some(deployed_slot_str.clone()))
                    .await?;

                self.state.emit_event(Event::Deployment {
                    project_id: project.id,
                    project_name: project.name.clone(),
                    build_id: build.id,
                    status: "Success".to_string(),
                    slot: target_slot,
                    url: format!("https://app.yourdomain.com/{}/", project.name),
                    timestamp: Event::now(),
                });

                self.state.emit_event(Event::BuildStatus {
                    build_id: build.id,
                    project_id: project.id,
                    status: BuildStatus::Success,
                    timestamp: Event::now(),
                });

                // Stop old container (the previous active slot, opposite of target_slot)
                let old_slot = project.active_slot;  // This is the OLD slot before switching
                let old_container_id = match old_slot {
                    Slot::Blue => project.blue_container_id.clone(),
                    Slot::Green => project.green_container_id.clone(),
                };

                if let Some(old_id) = old_container_id {
                    info!("Stopping old {} container: {}", old_slot, old_id);
                    self.docker.stop_container(&old_id).await.ok();
                    self.docker.remove_container(&old_id).await.ok();

                    // Clear old container ID from database
                    match old_slot {
                        Slot::Blue => {
                            self.state
                                .db
                                .update_project_blue_container(project.id, None)
                                .await?;
                        }
                        Slot::Green => {
                            self.state
                                .db
                                .update_project_green_container(project.id, None)
                                .await?;
                        }
                    }
                }

                Ok(())
            }
            Err(e) => {
                warn!("Health check failed: {}", e);

                // TODO: TEMPORARILY DISABLED FOR DEBUGGING
                // // Rollback: stop and remove new container
                // self.docker.stop_container(&container_id).await.ok();
                // self.docker.remove_container(&container_id).await.ok();

                // // Clear container ID from database
                // match target_slot {
                //     Slot::Blue => {
                //         self.state
                //             .db
                //             .update_project_blue_container(project.id, None)
                //             .await?;
                //     }
                //     Slot::Green => {
                //         self.state
                //             .db
                //             .update_project_green_container(project.id, None)
                //             .await?;
                //     }
                // }

                // // Update build status to Failed
                // self.state
                //     .db
                //     .finish_build(build.id, BuildStatus::Failed)
                //     .await?;

                self.state.emit_event(Event::Deployment {
                    project_id: project.id,
                    project_name: project.name.clone(),
                    build_id: build.id,
                    status: "Failed".to_string(),
                    slot: target_slot,
                    url: format!("https://app.yourdomain.com/{}/", project.name),
                    timestamp: Event::now(),
                });

                self.state.emit_event(Event::BuildStatus {
                    build_id: build.id,
                    project_id: project.id,
                    status: BuildStatus::Failed,
                    timestamp: Event::now(),
                });

                self.state.emit_event(Event::Error {
                    project_id: Some(project.id),
                    build_id: Some(build.id),
                    message: format!("Health check failed: {}", e),
                    timestamp: Event::now(),
                });

                anyhow::bail!("Deployment failed: {}", e);
            }
        }
    }

    async fn perform_health_check(&self, project: &Project, build: &Build, port: u16) -> Result<()> {
        let max_attempts = 10;
        let retry_interval = Duration::from_secs(5);
        let gateway_ip = self.docker.gateway_ip();
        let health_check_url = format!("http://{}:{}{}", gateway_ip, port, project.health_check_url);

        info!("Starting health check: {}", health_check_url);

        for attempt in 1..=max_attempts {
            self.state.emit_event(Event::HealthCheck {
                project_id: project.id,
                build_id: build.id,
                attempt,
                max_attempts,
                status: "Checking".to_string(),
                url: health_check_url.clone(),
                timestamp: Event::now(),
            });

            match reqwest::get(&health_check_url).await {
                Ok(response) => {
                    let status_code = response.status();
                    if status_code.is_success() {
                        info!("Health check passed on attempt {}/{} - Status: {}", attempt, max_attempts, status_code);

                        self.state.emit_event(Event::HealthCheck {
                            project_id: project.id,
                            build_id: build.id,
                            attempt,
                            max_attempts,
                            status: "Success".to_string(),
                            url: health_check_url,
                            timestamp: Event::now(),
                        });

                        return Ok(());
                    } else {
                        let body = response.text().await.unwrap_or_else(|_| "<failed to read body>".to_string());
                        warn!(
                            "Health check returned status {} on attempt {}/{} - URL: {} - Body preview: {}",
                            status_code,
                            attempt,
                            max_attempts,
                            health_check_url,
                            &body.chars().take(200).collect::<String>()
                        );
                    }
                }
                Err(e) => {
                    warn!(
                        "Health check failed on attempt {}/{} - URL: {} - Error: {}",
                        attempt, max_attempts, health_check_url, e
                    );

                    // Check if container is still running
                    if let Some(container_id) = match port {
                        p if p == project.blue_port as u16 => project.blue_container_id.as_ref(),
                        p if p == project.green_port as u16 => project.green_container_id.as_ref(),
                        _ => None,
                    } {
                        if !self.docker.is_container_running(container_id).await {
                            warn!("Container {} is not running!", container_id);
                        }
                    }
                }
            }

            if attempt < max_attempts {
                sleep(retry_interval).await;
            }
        }

        self.state.emit_event(Event::HealthCheck {
            project_id: project.id,
            build_id: build.id,
            attempt: max_attempts,
            max_attempts,
            status: "Failed".to_string(),
            url: health_check_url,
            timestamp: Event::now(),
        });

        anyhow::bail!("Health check timed out after {} attempts", max_attempts)
    }
}
