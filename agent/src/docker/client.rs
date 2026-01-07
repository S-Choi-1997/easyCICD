use anyhow::{Context, Result};
use bollard::container::{
    Config, CreateContainerOptions, LogOutput, RemoveContainerOptions, StartContainerOptions,
    StopContainerOptions,
};
use bollard::image::CreateImageOptions;
use bollard::Docker;
use futures_util::StreamExt;
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{debug, info, warn};

#[derive(Clone)]
pub struct DockerClient {
    docker: Docker,
}

impl DockerClient {
    pub fn new() -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()
            .context("Failed to connect to Docker daemon")?;
        Ok(Self { docker })
    }

    /// Pull image if not exists
    pub async fn ensure_image(&self, image: &str) -> Result<()> {
        info!("Ensuring image: {}", image);

        let mut stream = self.docker.create_image(
            Some(CreateImageOptions {
                from_image: image,
                ..Default::default()
            }),
            None,
            None,
        );

        let mut has_error = false;
        let mut error_message = String::new();

        while let Some(result) = stream.next().await {
            match result {
                Ok(info) => {
                    if let Some(status) = info.status {
                        debug!("Image pull: {}", status);
                    }
                    if let Some(error) = info.error {
                        has_error = true;
                        error_message = error;
                        warn!("Image pull error: {}", error_message);
                    }
                }
                Err(e) => {
                    has_error = true;
                    error_message = e.to_string();
                    warn!("Image pull stream error: {}", e);
                }
            }
        }

        if has_error {
            anyhow::bail!("Failed to pull image {}: {}", image, error_message);
        }

        info!("Image {} ready", image);
        Ok(())
    }

    /// Run build container
    pub async fn run_build_container(
        &self,
        image: &str,
        command: &str,
        workspace_path: PathBuf,
        output_path: PathBuf,
        cache_path: PathBuf,
        cache_type: &str,
    ) -> Result<(String, Vec<String>)> {
        self.ensure_image(image).await?;

        let container_name = format!("build-{}", uuid::Uuid::new_v4());

        let mut binds = vec![
            format!("{}:/app:ro", workspace_path.display()),
            format!("{}:/output", output_path.display()),
        ];

        // Add cache mount if provided
        if cache_path.exists() {
            let cache_mount = match cache_type {
                "gradle" => "/root/.gradle",
                "maven" => "/root/.m2",
                "npm" => "/root/.npm",
                "pip" => "/root/.cache/pip",
                "cargo" => "/usr/local/cargo/registry",
                _ => "/cache",
            };
            binds.push(format!("{}:{}", cache_path.display(), cache_mount));
        }

        let config = Config {
            image: Some(image.to_string()),
            cmd: Some(vec!["/bin/sh".to_string(), "-c".to_string(), command.to_string()]),
            working_dir: Some("/app".to_string()),
            host_config: Some(bollard::models::HostConfig {
                binds: Some(binds),
                auto_remove: Some(false),
                ..Default::default()
            }),
            ..Default::default()
        };

        info!("Creating build container: {}", container_name);
        let container = self
            .docker
            .create_container(
                Some(CreateContainerOptions {
                    name: container_name.as_str(),
                    ..Default::default()
                }),
                config,
            )
            .await
            .context("Failed to create container")?;

        let container_id = container.id;

        info!("Starting build container: {}", container_id);
        self.docker
            .start_container(&container_id, None::<StartContainerOptions<&str>>)
            .await
            .context("Failed to start container")?;

        // Collect logs
        let mut log_stream = self.docker.logs(
            &container_id,
            Some(bollard::container::LogsOptions::<String> {
                follow: true,
                stdout: true,
                stderr: true,
                ..Default::default()
            }),
        );

        let mut logs = Vec::new();
        while let Some(log_result) = log_stream.next().await {
            match log_result {
                Ok(output) => {
                    let line = match output {
                        LogOutput::StdOut { message } => String::from_utf8_lossy(&message).to_string(),
                        LogOutput::StdErr { message } => String::from_utf8_lossy(&message).to_string(),
                        _ => continue,
                    };
                    logs.push(line);
                }
                Err(e) => {
                    warn!("Error reading logs: {}", e);
                    break;
                }
            }
        }

        // Wait for container to finish
        let wait_result = self
            .docker
            .wait_container(&container_id, None::<bollard::container::WaitContainerOptions<&str>>)
            .next()
            .await;

        let exit_code = match wait_result {
            Some(Ok(result)) => result.status_code,
            _ => -1,
        };

        // Remove container
        let _ = self
            .docker
            .remove_container(
                &container_id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await;

        if exit_code != 0 {
            anyhow::bail!("Build failed with exit code: {}", exit_code);
        }

        Ok((container_id, logs))
    }

    /// Run runtime container (Blue/Green)
    pub async fn run_runtime_container(
        &self,
        image: &str,
        command: &str,
        output_path: PathBuf,
        port: u16,
        project_name: &str,
        slot: &str,
    ) -> Result<String> {
        self.ensure_image(image).await?;

        let container_name = format!("{}-{}", project_name, slot);

        // Stop and remove existing container with same name
        let _ = self.stop_container(&container_name).await;
        let _ = self.remove_container(&container_name).await;

        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            "8080/tcp".to_string(),
            Some(vec![bollard::models::PortBinding {
                host_ip: Some("0.0.0.0".to_string()),
                host_port: Some(port.to_string()),
            }]),
        );

        let config = Config {
            image: Some(image.to_string()),
            cmd: Some(vec!["/bin/sh".to_string(), "-c".to_string(), command.to_string()]),
            working_dir: Some("/app".to_string()),
            host_config: Some(bollard::models::HostConfig {
                binds: Some(vec![format!("{}:/app:ro", output_path.display())]),
                port_bindings: Some(port_bindings),
                network_mode: Some("bridge".to_string()),
                restart_policy: Some(bollard::models::RestartPolicy {
                    name: Some(bollard::models::RestartPolicyNameEnum::UNLESS_STOPPED),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            exposed_ports: Some({
                let mut map = HashMap::new();
                map.insert("8080/tcp".to_string(), HashMap::new());
                map
            }),
            ..Default::default()
        };

        info!("Creating runtime container: {}", container_name);
        let container = self
            .docker
            .create_container(
                Some(CreateContainerOptions {
                    name: container_name.as_str(),
                    ..Default::default()
                }),
                config,
            )
            .await
            .context("Failed to create runtime container")?;

        let container_id = container.id.clone();

        info!("Starting runtime container: {}", container_id);
        self.docker
            .start_container(&container_id, None::<StartContainerOptions<&str>>)
            .await
            .context("Failed to start runtime container")?;

        Ok(container_id)
    }

    /// Stop container
    pub async fn stop_container(&self, container_id: &str) -> Result<()> {
        info!("Stopping container: {}", container_id);
        self.docker
            .stop_container(
                container_id,
                Some(StopContainerOptions {
                    t: 10, // 10 seconds timeout
                }),
            )
            .await
            .ok();
        Ok(())
    }

    /// Remove container
    pub async fn remove_container(&self, container_id: &str) -> Result<()> {
        info!("Removing container: {}", container_id);
        self.docker
            .remove_container(
                container_id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await
            .ok();
        Ok(())
    }

    /// Check if container is running
    pub async fn is_container_running(&self, container_id: &str) -> bool {
        match self.docker.inspect_container(container_id, None::<bollard::container::InspectContainerOptions>).await {
            Ok(info) => {
                if let Some(state) = info.state {
                    state.running.unwrap_or(false)
                } else {
                    false
                }
            }
            Err(_) => false,
        }
    }
}
