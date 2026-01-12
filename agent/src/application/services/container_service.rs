use std::sync::Arc;
use anyhow::{Result, Context};
use tracing::info;

use crate::application::ports::repositories::ContainerRepository;
use crate::db::models::{Container, CreateContainer, ContainerStatus};
use crate::docker::DockerClient;
use crate::infrastructure::logging::{BoundaryLogger, Timer};
use crate::application::events::event_bus::EventBus;
use crate::events::Event;

pub struct ContainerService<CR, EB>
where
    CR: ContainerRepository,
    EB: EventBus,
{
    container_repo: Arc<CR>,
    docker: DockerClient,
    logger: Arc<BoundaryLogger>,
    event_bus: Arc<EB>,
}

impl<CR, EB> ContainerService<CR, EB>
where
    CR: ContainerRepository,
    EB: EventBus,
{
    pub fn new(
        container_repo: Arc<CR>,
        docker: DockerClient,
        logger: Arc<BoundaryLogger>,
        event_bus: Arc<EB>,
    ) -> Self {
        Self {
            container_repo,
            docker,
            logger,
            event_bus,
        }
    }

    /// Create a new container (DB only, not started)
    pub async fn create_container(&self, trace_id: &str, req: CreateContainer) -> Result<Container> {
        let timer = Timer::start();
        self.logger.service_entry(trace_id, "API", "ContainerService", "create_container", &req.name);

        let container = self.container_repo.create(req).await?;

        self.logger.service_exit(trace_id, "API", "ContainerService", "create_container", timer.elapsed_ms());
        Ok(container)
    }

    /// Start a container
    pub async fn start_container(&self, trace_id: &str, id: i64) -> Result<Container> {
        let timer = Timer::start();
        self.logger.service_entry(trace_id, "API", "ContainerService", "start_container", &id);

        // Get container from DB
        let container = self.container_repo.get(id).await?
            .context(format!("Container not found: {}", id))?;

        // If already running, emit event and return
        if container.status == ContainerStatus::Running && container.container_id.is_some() {
            info!("[{}] Container {} is already running", trace_id, container.name);

            // Emit status event even if already running (for UI sync)
            let event = Event::standalone_container_status(
                id,
                container.name.clone(),
                container.container_id.clone(),
                "running".to_string(),
            );
            self.event_bus.emit(event).await;

            return Ok(container);
        }

        // Start Docker container
        self.logger.external_call(trace_id, "ContainerService", "Docker", "run_container");
        let docker_timer = Timer::start();

        let container_port = container.container_port.unwrap_or(container.port);
        let persist_data = container.persist_data != 0;

        let docker_container_id = self.docker.run_standalone_container(
            &container.name,
            &container.image,
            container.port,
            container_port,
            container.env_vars.as_deref(),
            container.command.as_deref(),
            persist_data,
        ).await?;

        self.logger.external_done(trace_id, "ContainerService", "Docker", "run_container", docker_timer.elapsed_ms());

        // Update DB
        self.container_repo.update_container_id(id, Some(docker_container_id.clone())).await?;
        self.container_repo.update_status(id, ContainerStatus::Running).await?;

        // Return updated container
        let updated = self.container_repo.get(id).await?
            .context("Container not found after update")?;

        // Emit status event
        let event = Event::standalone_container_status(
            id,
            container.name.clone(),
            Some(docker_container_id),
            "running".to_string(),
        );
        self.event_bus.emit(event).await;

        self.logger.service_exit(trace_id, "API", "ContainerService", "start_container", timer.elapsed_ms());
        Ok(updated)
    }

    /// Stop a container
    pub async fn stop_container(&self, trace_id: &str, id: i64) -> Result<Container> {
        let timer = Timer::start();
        self.logger.service_entry(trace_id, "API", "ContainerService", "stop_container", &id);

        let container = self.container_repo.get(id).await?
            .context(format!("Container not found: {}", id))?;

        // If already stopped, emit event and return
        if container.status == ContainerStatus::Stopped {
            info!("[{}] Container {} is already stopped", trace_id, container.name);

            // Emit status event even if already stopped (for UI sync)
            let event = Event::standalone_container_status(
                id,
                container.name.clone(),
                None,
                "stopped".to_string(),
            );
            self.event_bus.emit(event).await;

            return Ok(container);
        }

        // Stop Docker container
        if let Some(docker_id) = &container.container_id {
            self.logger.external_call(trace_id, "ContainerService", "Docker", "stop_container");
            let docker_timer = Timer::start();

            self.docker.stop_container(docker_id).await.ok();
            self.docker.remove_container(docker_id).await.ok();

            self.logger.external_done(trace_id, "ContainerService", "Docker", "stop_container", docker_timer.elapsed_ms());
        }

        // Update DB
        self.container_repo.update_container_id(id, None).await?;
        self.container_repo.update_status(id, ContainerStatus::Stopped).await?;

        let updated = self.container_repo.get(id).await?
            .context("Container not found after update")?;

        // Emit status event
        let event = Event::standalone_container_status(
            id,
            container.name.clone(),
            None,
            "stopped".to_string(),
        );
        self.event_bus.emit(event).await;

        self.logger.service_exit(trace_id, "API", "ContainerService", "stop_container", timer.elapsed_ms());
        Ok(updated)
    }

    /// Delete a container (stop first if running)
    pub async fn delete_container(&self, trace_id: &str, id: i64) -> Result<()> {
        let timer = Timer::start();
        self.logger.service_entry(trace_id, "API", "ContainerService", "delete_container", &id);

        let container = self.container_repo.get(id).await?;

        if let Some(c) = container {
            // Stop if running
            if c.status == ContainerStatus::Running {
                if let Some(docker_id) = &c.container_id {
                    self.docker.stop_container(docker_id).await.ok();
                    self.docker.remove_container(docker_id).await.ok();
                }
            }

            // Delete from DB (also releases port)
            self.container_repo.delete(id).await?;
        }

        self.logger.service_exit(trace_id, "API", "ContainerService", "delete_container", timer.elapsed_ms());
        Ok(())
    }

    /// List all containers (with real-time Docker status sync)
    pub async fn list_containers(&self, trace_id: &str) -> Result<Vec<Container>> {
        let timer = Timer::start();
        self.logger.service_entry(trace_id, "API", "ContainerService", "list_containers", &"");

        let mut containers = self.container_repo.list().await?;

        // Sync each container's status with actual Docker state
        for container in &mut containers {
            if let Some(ref docker_id) = container.container_id {
                // Check if container is actually running in Docker
                let is_running = self.docker.is_container_running(docker_id).await;

                let actual_status = if is_running {
                    ContainerStatus::Running
                } else {
                    ContainerStatus::Stopped
                };

                // Update DB if status differs
                if container.status != actual_status {
                    info!("[{}] Syncing container {} status: {:?} -> {:?}",
                          trace_id, container.name, container.status, actual_status);

                    self.container_repo.update_status(container.id, actual_status).await?;
                    container.status = actual_status;

                    // Emit status event for WebSocket clients
                    let status_str = if is_running { "running" } else { "stopped" };
                    let event = Event::standalone_container_status(
                        container.id,
                        container.name.clone(),
                        Some(docker_id.clone()),
                        status_str.to_string(),
                    );
                    self.event_bus.emit(event).await;
                }
            }
        }

        self.logger.service_exit(trace_id, "API", "ContainerService", "list_containers", timer.elapsed_ms());
        Ok(containers)
    }

    /// Get a container by ID
    pub async fn get_container(&self, trace_id: &str, id: i64) -> Result<Option<Container>> {
        let timer = Timer::start();
        self.logger.service_entry(trace_id, "API", "ContainerService", "get_container", &id);

        let container = self.container_repo.get(id).await?;

        self.logger.service_exit(trace_id, "API", "ContainerService", "get_container", timer.elapsed_ms());
        Ok(container)
    }

    /// Get container logs
    pub async fn get_logs(&self, trace_id: &str, id: i64, tail: Option<usize>) -> Result<Vec<String>> {
        let timer = Timer::start();
        self.logger.service_entry(trace_id, "API", "ContainerService", "get_logs", &id);

        let container = self.container_repo.get(id).await?
            .context(format!("Container not found: {}", id))?;

        let logs = if let Some(docker_id) = &container.container_id {
            self.logger.external_call(trace_id, "ContainerService", "Docker", "get_logs");
            let docker_timer = Timer::start();

            let result = self.docker.get_container_logs(docker_id, tail).await
                .unwrap_or_else(|e| vec![format!("Failed to get logs: {}", e)]);

            self.logger.external_done(trace_id, "ContainerService", "Docker", "get_logs", docker_timer.elapsed_ms());
            result
        } else {
            vec!["Container is not running".to_string()]
        };

        self.logger.service_exit(trace_id, "API", "ContainerService", "get_logs", timer.elapsed_ms());
        Ok(logs)
    }
}
