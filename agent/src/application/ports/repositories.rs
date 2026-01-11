use async_trait::async_trait;
use anyhow::Result;
use crate::db::models::{Project, Build, CreateProject, CreateBuild, Slot, BuildStatus, Container, CreateContainer, ContainerStatus};

/// Repository trait for Project operations
#[async_trait]
pub trait ProjectRepository: Send + Sync {
    /// Create a new project
    async fn create(&self, project: CreateProject) -> Result<Project>;

    /// Get a project by ID
    async fn get(&self, id: i64) -> Result<Option<Project>>;

    /// Get a project by name
    async fn get_by_name(&self, name: &str) -> Result<Option<Project>>;

    /// List all projects
    async fn list(&self) -> Result<Vec<Project>>;

    /// Update the active slot for a project
    async fn update_active_slot(&self, id: i64, slot: Slot) -> Result<()>;

    /// Update the blue container ID
    async fn update_blue_container(&self, id: i64, container_id: Option<String>) -> Result<()>;

    /// Update the green container ID
    async fn update_green_container(&self, id: i64, container_id: Option<String>) -> Result<()>;

    /// Delete a project
    async fn delete(&self, id: i64) -> Result<()>;
}

/// Repository trait for Build operations
#[async_trait]
pub trait BuildRepository: Send + Sync {
    /// Create a new build
    async fn create(&self, build: CreateBuild) -> Result<Build>;

    /// Get a build by ID
    async fn get(&self, id: i64) -> Result<Option<Build>>;

    /// List all builds
    async fn list(&self, limit: i64) -> Result<Vec<Build>>;

    /// List builds for a specific project
    async fn list_by_project(&self, project_id: i64, limit: i64) -> Result<Vec<Build>>;

    /// List recent builds (all projects)
    async fn list_recent(&self, limit: i64) -> Result<Vec<Build>>;

    /// Update build status
    async fn update_status(&self, id: i64, status: BuildStatus) -> Result<()>;

    /// Finish a build (update status and finished_at)
    async fn finish(&self, id: i64, status: BuildStatus) -> Result<()>;

    /// Update deployed slot
    async fn update_deployed_slot(&self, id: i64, slot: Option<String>) -> Result<()>;

    /// Update deploy log path
    async fn update_deploy_log_path(&self, id: i64, path: String) -> Result<()>;
}

/// Repository trait for Settings operations
#[async_trait]
pub trait SettingsRepository: Send + Sync {
    /// Get a setting value by key
    async fn get(&self, key: &str) -> Result<Option<String>>;

    /// Set a setting value
    async fn set(&self, key: &str, value: &str) -> Result<()>;

    /// Delete a setting
    async fn delete(&self, key: &str) -> Result<()>;
}

/// Repository trait for Container operations
#[async_trait]
pub trait ContainerRepository: Send + Sync {
    /// Create a new container
    async fn create(&self, container: CreateContainer) -> Result<Container>;

    /// Get a container by ID
    async fn get(&self, id: i64) -> Result<Option<Container>>;

    /// Get a container by name
    async fn get_by_name(&self, name: &str) -> Result<Option<Container>>;

    /// List all containers
    async fn list(&self) -> Result<Vec<Container>>;

    /// Update container status
    async fn update_status(&self, id: i64, status: ContainerStatus) -> Result<()>;

    /// Update container ID (Docker container ID)
    async fn update_container_id(&self, id: i64, container_id: Option<String>) -> Result<()>;

    /// Delete a container
    async fn delete(&self, id: i64) -> Result<()>;

    /// Allocate a port in the container range (15000-19999)
    async fn allocate_port(&self) -> Result<i32>;

    /// Release a port
    async fn release_port(&self, port: i32) -> Result<()>;
}
