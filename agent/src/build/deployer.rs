use anyhow::{Context, Result};
use std::path::PathBuf;
use std::time::Duration;
use tokio::fs;
use tokio::io::AsyncWriteExt;
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
        write_log!(format!("Deploying to {} slot on port {}", target_slot, target_port));

        // Clean up target slot's old container if it exists
        let old_container_id = match target_slot {
            Slot::Blue => project.blue_container_id.as_ref(),
            Slot::Green => project.green_container_id.as_ref(),
        };

        if let Some(old_id) = old_container_id {
            // Check if container actually exists
            if self.docker.is_container_running(old_id).await {
                info!("Stopping old {} container: {}", target_slot, old_id);
                write_log!(format!("Stopping old {} container: {}", target_slot, old_id));
                self.docker.stop_container(old_id).await.ok();
            } else {
                info!("Old {} container {} not found, skipping cleanup", target_slot, old_id);
                write_log!(format!("Old {} container {} not found, skipping cleanup", target_slot, old_id));
            }
            // Always try to remove regardless of running state
            self.docker.remove_container(old_id).await.ok();

            // Clear from database
            match target_slot {
                Slot::Blue => {
                    self.state.db.update_project_blue_container(project.id, None).await?;
                }
                Slot::Green => {
                    self.state.db.update_project_green_container(project.id, None).await?;
                }
            }
        }

        // Start runtime container
        write_log!(format!("Starting runtime container with image: {}", project.runtime_image));
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
            )
            .await
            .context("Failed to start runtime container")?;

        info!("Runtime container started: {}", container_id);
        write_log!(format!("Runtime container started: {}", container_id));

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
            .perform_health_check(project, build, &container_id)
            .await;

        match health_check_result {
            Ok(_) => {
                info!("Health check passed, switching to {} slot", target_slot);
                write_log!(format!("Health check passed, switching to {} slot", target_slot));

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
                    write_log!(format!("Stopping old {} container: {}", old_slot, old_id));
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

                write_log!("Deployment completed successfully");
                Ok(())
            }
            Err(e) => {
                warn!("Health check failed: {}", e);
                write_log!(format!("Health check failed: {}", e));

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

                write_log!(format!("Deployment failed: {}", e));
                anyhow::bail!("Deployment failed: {}", e);
            }
        }
    }

    async fn perform_health_check(&self, project: &Project, build: &Build, container_id: &str) -> Result<()> {
        let max_attempts = 10;
        let retry_interval = Duration::from_secs(2);

        info!("Starting container health check for container: {}", container_id);

        for attempt in 1..=max_attempts {
            self.state.emit_event(Event::HealthCheck {
                project_id: project.id,
                build_id: build.id,
                attempt,
                max_attempts,
                status: "Checking".to_string(),
                url: format!("container://{}", container_id),
                timestamp: Event::now(),
            });

            // Check if container is running
            let is_running = self.docker.is_container_running(&container_id).await;

            if is_running {
                info!("Container health check passed on attempt {}/{} - Container is running", attempt, max_attempts);

                self.state.emit_event(Event::HealthCheck {
                    project_id: project.id,
                    build_id: build.id,
                    attempt,
                    max_attempts,
                    status: "Success".to_string(),
                    url: format!("container://{}", container_id),
                    timestamp: Event::now(),
                });

                return Ok(());
            } else {
                warn!(
                    "Container health check failed on attempt {}/{} - Container is not running",
                    attempt,
                    max_attempts
                );
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
            url: format!("container://{}", container_id),
            timestamp: Event::now(),
        });

        anyhow::bail!("Health check timed out after {} attempts", max_attempts)
    }
}
