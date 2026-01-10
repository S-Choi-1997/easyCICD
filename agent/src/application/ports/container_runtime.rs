use async_trait::async_trait;
use anyhow::Result;
use std::path::PathBuf;

/// Configuration for running a build container
#[derive(Debug, Clone)]
pub struct BuildContainerConfig {
    pub image: String,
    pub command: String,
    pub workspace_path: PathBuf,
    pub output_path: PathBuf,
    pub cache_path: PathBuf,
    pub cache_type: String,
    pub working_directory: Option<String>,
}

/// Output from a build container execution
#[derive(Debug)]
pub struct BuildOutput {
    pub container_id: String,
    pub logs: Vec<String>,
    pub exit_code: i64,
}

/// Configuration for running a runtime container (blue/green deployment)
#[derive(Debug, Clone)]
pub struct RuntimeContainerConfig {
    pub image: String,
    pub command: String,
    pub output_path: PathBuf,
    pub host_port: u16,
    pub container_port: u16,
    pub project_id: i64,
    pub slot: String,
}

/// Trait for container runtime operations (Docker, Podman, etc.)
#[async_trait]
pub trait ContainerRuntime: Send + Sync + Clone {
    /// Run a build container and return the output
    async fn run_build(&self, config: BuildContainerConfig) -> Result<BuildOutput>;

    /// Run a runtime container for blue/green deployment
    /// Returns the container ID
    async fn run_runtime(&self, config: RuntimeContainerConfig) -> Result<String>;

    /// Check if a container is running
    async fn is_running(&self, container_id: &str) -> bool;

    /// Stop a container
    async fn stop(&self, container_id: &str) -> Result<()>;

    /// Remove a container
    async fn remove(&self, container_id: &str) -> Result<()>;

    /// Get the gateway IP for container communication
    fn gateway_ip(&self) -> &str;
}
