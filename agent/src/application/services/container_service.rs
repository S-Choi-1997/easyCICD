use std::sync::Arc;
use anyhow::{Result, Context};
use tracing::info;

use crate::application::ports::repositories::ContainerRepository;
use crate::db::models::{Container, CreateContainer, ContainerStatus};
use crate::docker::DockerClient;
use crate::infrastructure::logging::{BoundaryLogger, Timer};

pub struct ContainerService<CR>
where
    CR: ContainerRepository,
{
    container_repo: Arc<CR>,
    docker: DockerClient,
    logger: Arc<BoundaryLogger>,
}

impl<CR> ContainerService<CR>
where
    CR: ContainerRepository,
{
    pub fn new(
        container_repo: Arc<CR>,
        docker: DockerClient,
        logger: Arc<BoundaryLogger>,
    ) -> Self {
        Self {
            container_repo,
            docker,
            logger,
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

        // If already running, return
        if container.status == ContainerStatus::Running && container.container_id.is_some() {
            info!("[{}] Container {} is already running", trace_id, container.name);
            return Ok(container);
        }

        // Start Docker container
        self.logger.external_call(trace_id, "ContainerService", "Docker", "run_container");
        let docker_timer = Timer::start();

        let docker_container_id = self.docker.run_standalone_container(
            &container.name,
            &container.image,
            container.port,
            container.env_vars.as_deref(),
            container.command.as_deref(),
        ).await?;

        self.logger.external_done(trace_id, "ContainerService", "Docker", "run_container", docker_timer.elapsed_ms());

        // Update DB
        self.container_repo.update_container_id(id, Some(docker_container_id)).await?;
        self.container_repo.update_status(id, ContainerStatus::Running).await?;

        // Return updated container
        let updated = self.container_repo.get(id).await?
            .context("Container not found after update")?;

        self.logger.service_exit(trace_id, "API", "ContainerService", "start_container", timer.elapsed_ms());
        Ok(updated)
    }

    /// Stop a container
    pub async fn stop_container(&self, trace_id: &str, id: i64) -> Result<Container> {
        let timer = Timer::start();
        self.logger.service_entry(trace_id, "API", "ContainerService", "stop_container", &id);

        let container = self.container_repo.get(id).await?
            .context(format!("Container not found: {}", id))?;

        // If already stopped, return
        if container.status == ContainerStatus::Stopped {
            info!("[{}] Container {} is already stopped", trace_id, container.name);
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

    /// List all containers
    pub async fn list_containers(&self, trace_id: &str) -> Result<Vec<Container>> {
        let timer = Timer::start();
        self.logger.service_entry(trace_id, "API", "ContainerService", "list_containers", &"");

        let containers = self.container_repo.list().await?;

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
}
