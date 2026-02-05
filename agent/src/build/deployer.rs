use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;
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

        // Build is already successful at this point, now starting deployment
        // Emit deployment status event
        self.state.emit_event(Event::Deployment {
            project_id: project.id,
            project_name: project.name.clone(),
            build_id: build.id,
            status: "deploying".to_string(),
            slot: project.get_inactive_slot(),
            url: format!("http://{}:{}", "localhost", project.get_inactive_port()),
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
                project.runtime_env_vars.as_deref(),
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

        // 빌드 성공 시 바로 배포 성공 처리 (헬스체크 없이)
        info!("Build succeeded, switching to {} slot", target_slot);
        write_log!(format!("Build succeeded, switching to {} slot", target_slot));

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
        let old_slot = project.active_slot;
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
}
