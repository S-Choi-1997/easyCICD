use anyhow::{Context, Result};
use bollard::container::{
    Config, CreateContainerOptions, InspectContainerOptions, LogOutput, RemoveContainerOptions,
    RestartContainerOptions, StartContainerOptions, StopContainerOptions,
};
use bollard::image::CreateImageOptions;
use bollard::Docker;
use futures_util::StreamExt;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tracing::{debug, info, warn};

#[derive(Clone)]
pub struct DockerClient {
    docker: Docker,
    host_data_path: Option<String>,
    gateway_ip: String,
}

impl DockerClient {
    pub fn new() -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()
            .context("Failed to connect to Docker daemon")?;
        Ok(Self {
            docker,
            host_data_path: None,
            gateway_ip: "172.17.0.1".to_string(),
        })
    }

    pub async fn new_with_host_path_detection() -> Result<Self> {
        let docker = Docker::connect_with_local_defaults()
            .context("Failed to connect to Docker daemon")?;

        let mut client = Self {
            docker: docker.clone(),
            host_data_path: None,
            gateway_ip: "172.17.0.1".to_string(),
        };

        // Detect host path and gateway IP by inspecting our own container
        if let Ok(hostname) = std::fs::read_to_string("/etc/hostname") {
            let container_id = hostname.trim();
            info!("Detecting host path for container: {}", container_id);

            if let Ok(inspect) = docker.inspect_container(container_id, None::<InspectContainerOptions>).await {
                // Detect host data path
                if let Some(mounts) = inspect.mounts {
                    for mount in mounts {
                        if mount.destination == Some("/data".to_string()) {
                            if let Some(source) = mount.source {
                                info!("Detected host data path: {}", source);
                                client.host_data_path = Some(source);
                                break;
                            }
                        }
                    }
                }

                // Detect gateway IP from network settings
                if let Some(network_settings) = inspect.network_settings {
                    if let Some(networks) = network_settings.networks {
                        if let Some((_name, network)) = networks.iter().next() {
                            if let Some(gateway) = &network.gateway {
                                if !gateway.is_empty() {
                                    info!("Detected gateway IP: {}", gateway);
                                    client.gateway_ip = gateway.clone();
                                }
                            }
                        }
                    }
                }
            }
        }

        if client.host_data_path.is_none() {
            warn!("Could not detect host data path - DOOD mounts may fail!");
        }

        Ok(client)
    }

    /// Convert container path to host path for DOOD
    fn to_host_path(&self, container_path: &Path) -> PathBuf {
        if let Some(host_data) = &self.host_data_path {
            // If path starts with /data, replace it with host path
            if let Ok(rel_path) = container_path.strip_prefix("/data") {
                let host_path = PathBuf::from(host_data).join(rel_path);
                info!("Path conversion: {} -> {}", container_path.display(), host_path.display());
                return host_path;
            }
        }

        // Fallback: use original path (may fail in DOOD)
        warn!("No host path conversion for: {}", container_path.display());
        container_path.to_path_buf()
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
        working_directory: &str,
    ) -> Result<(String, Vec<String>)> {
        self.ensure_image(image).await?;

        let container_name = format!("build-{}", uuid::Uuid::new_v4());

        // Convert container paths to host paths for DOOD
        let host_workspace = self.to_host_path(&workspace_path);
        let host_output = self.to_host_path(&output_path);
        let host_cache = self.to_host_path(&cache_path);

        info!("Build container mounts:");
        info!("  Workspace: {} (host: {})", workspace_path.display(), host_workspace.display());
        info!("  Output: {} (host: {})", output_path.display(), host_output.display());
        info!("  Cache: {} (host: {})", cache_path.display(), host_cache.display());

        let mut binds = vec![
            format!("{}:/app", host_workspace.display()),
            format!("{}:/output", host_output.display()),
            // Mount Docker socket for Docker-outside-of-Docker (DOOD)
            "/var/run/docker.sock:/var/run/docker.sock".to_string(),
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
            binds.push(format!("{}:{}", host_cache.display(), cache_mount));
        }

        // Determine working directory
        let work_dir = if working_directory.is_empty() {
            "/app".to_string()
        } else {
            format!("/app/{}", working_directory.trim_start_matches('/'))
        };

        let config = Config {
            image: Some(image.to_string()),
            cmd: Some(vec!["/bin/sh".to_string(), "-c".to_string(), command.to_string()]),
            working_dir: Some(work_dir),
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
                    logs.push(format!("ERROR: Failed to read logs: {}", e));
                    break;
                }
            }
        }

        info!("Collected {} lines of logs from container {}", logs.len(), container_id);
        if logs.is_empty() {
            warn!("No logs collected from container {} - container may have exited immediately", container_id);
        }

        // Wait for container to finish
        let wait_result = self
            .docker
            .wait_container(&container_id, None::<bollard::container::WaitContainerOptions<&str>>)
            .next()
            .await;

        let exit_code = match wait_result {
            Some(Ok(result)) => {
                info!("Container {} exited with code: {}", container_id, result.status_code);
                result.status_code
            }
            Some(Err(e)) => {
                tracing::error!("Failed to wait for container {}: {}", container_id, e);
                // Try to inspect container to get more info
                if let Ok(inspect) = self.docker.inspect_container(&container_id, None::<bollard::container::InspectContainerOptions>).await {
                    if let Some(state) = inspect.state {
                        tracing::error!("Container state: running={:?}, exit_code={:?}, error={:?}",
                            state.running, state.exit_code, state.error);
                        if let Some(exit_code) = state.exit_code {
                            return Ok((container_id, logs));
                        }
                    }
                }
                -1
            }
            None => {
                tracing::error!("Container {} wait returned None (container may have been killed)", container_id);
                // Try to inspect container to get more info
                if let Ok(inspect) = self.docker.inspect_container(&container_id, None::<bollard::container::InspectContainerOptions>).await {
                    if let Some(state) = inspect.state {
                        tracing::error!("Container state: running={:?}, exit_code={:?}, error={:?}",
                            state.running, state.exit_code, state.error);
                    }
                }
                -1
            }
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
        runtime_port: u16,
        project_id: i64,
        slot: &str,
    ) -> Result<String> {
        self.ensure_image(image).await?;

        let container_name = format!("project-{}-{}", project_id, slot);

        // Stop and remove existing container with same name
        let _ = self.stop_container(&container_name).await;
        let _ = self.remove_container(&container_name).await;

        // Convert container path to host path for DOOD
        let host_output = self.to_host_path(&output_path);
        info!("Runtime container mount: {} (host: {})", output_path.display(), host_output.display());

        let container_port_str = format!("{}/tcp", runtime_port);

        let mut port_bindings = HashMap::new();
        port_bindings.insert(
            container_port_str.clone(),
            Some(vec![bollard::models::PortBinding {
                host_ip: Some("0.0.0.0".to_string()),
                host_port: Some(port.to_string()),
            }]),
        );

        let config = Config {
            image: Some(image.to_string()),
            cmd: Some(vec!["/bin/sh".to_string(), "-c".to_string(), command.to_string()]),
            working_dir: Some("/app".to_string()),
            env: Some(vec![format!("PORT={}", runtime_port)]),  // Apps can read PORT env var
            host_config: Some(bollard::models::HostConfig {
                binds: Some(vec![format!("{}:/app:ro", host_output.display())]),
                port_bindings: Some(port_bindings),
                restart_policy: Some(bollard::models::RestartPolicy {
                    name: Some(bollard::models::RestartPolicyNameEnum::UNLESS_STOPPED),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            exposed_ports: Some({
                let mut map = HashMap::new();
                map.insert(container_port_str.clone(), HashMap::new());
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

        // Connect to easycicd network
        info!("Connecting runtime container to easycicd network");
        self.docker
            .connect_network(
                "easycicd_easycicd",
                bollard::network::ConnectNetworkOptions {
                    container: container_id.as_str(),
                    ..Default::default()
                },
            )
            .await
            .context("Failed to connect container to network")?;

        info!("Starting runtime container: {}", container_id);
        if let Err(e) = self.docker
            .start_container(&container_id, None::<StartContainerOptions<&str>>)
            .await
        {
            return Err(anyhow::anyhow!(
                "Failed to start runtime container '{}' on port {}: {}. \
                Check if port is already in use or if there are configuration issues.",
                container_name, port, e
            ));
        }

        Ok(container_id)
    }

    /// Run standalone container (DB, Redis, etc.)
    pub async fn run_standalone_container(
        &self,
        name: &str,
        image: &str,
        host_port: i32,
        env_vars: Option<&str>,
        command: Option<&str>,
    ) -> Result<String> {
        self.ensure_image(image).await?;

        let container_name = format!("container-{}", name);

        // Stop and remove existing container with same name
        let _ = self.stop_container(&container_name).await;
        let _ = self.remove_container(&container_name).await;

        // Parse env vars from JSON if provided
        let env: Option<Vec<String>> = env_vars.and_then(|s| {
            serde_json::from_str::<serde_json::Value>(s)
                .ok()
                .and_then(|v| {
                    v.as_object().map(|obj| {
                        obj.iter()
                            .map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or(&v.to_string())))
                            .collect()
                    })
                })
        });

        // Parse command if provided
        let cmd: Option<Vec<String>> = command.map(|c| {
            vec!["/bin/sh".to_string(), "-c".to_string(), c.to_string()]
        });

        let config = Config {
            image: Some(image.to_string()),
            cmd,
            env,
            host_config: Some(bollard::models::HostConfig {
                port_bindings: Some({
                    let mut port_bindings = HashMap::new();
                    // Map container's default port to host port
                    // Most images expose their service port, we'll just bind to host
                    port_bindings.insert(
                        format!("{}/tcp", host_port),
                        Some(vec![bollard::models::PortBinding {
                            host_ip: Some("0.0.0.0".to_string()),
                            host_port: Some(host_port.to_string()),
                        }]),
                    );
                    port_bindings
                }),
                publish_all_ports: Some(true),
                restart_policy: Some(bollard::models::RestartPolicy {
                    name: Some(bollard::models::RestartPolicyNameEnum::UNLESS_STOPPED),
                    ..Default::default()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };

        info!("Creating standalone container: {}", container_name);
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
            .context("Failed to create standalone container")?;

        let container_id = container.id.clone();

        // Connect to easycicd network
        info!("Connecting standalone container to easycicd network");
        self.docker
            .connect_network(
                "easycicd_easycicd",
                bollard::network::ConnectNetworkOptions {
                    container: container_id.as_str(),
                    ..Default::default()
                },
            )
            .await
            .context("Failed to connect container to network")?;

        info!("Starting standalone container: {}", container_id);
        if let Err(e) = self.docker
            .start_container(&container_id, None::<StartContainerOptions<&str>>)
            .await
        {
            return Err(anyhow::anyhow!(
                "Failed to start standalone container '{}' on port {}: {}.",
                container_name, host_port, e
            ));
        }

        Ok(container_id)
    }

    /// Get gateway IP for health checks
    pub fn gateway_ip(&self) -> &str {
        &self.gateway_ip
    }

    /// Get access to underlying Docker API for advanced operations
    pub fn docker_api(&self) -> &Docker {
        &self.docker
    }

    /// Start container
    pub async fn start_container(&self, container_id: &str) -> Result<()> {
        info!("Starting container: {}", container_id);
        self.docker
            .start_container(container_id, None::<StartContainerOptions<String>>)
            .await
            .context("Failed to start container")?;
        Ok(())
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

    /// Restart container
    pub async fn restart_container(&self, container_id: &str) -> Result<()> {
        info!("Restarting container: {}", container_id);
        self.docker
            .restart_container(
                container_id,
                Some(RestartContainerOptions {
                    t: 10, // 10 seconds timeout
                }),
            )
            .await
            .context("Failed to restart container")?;
        Ok(())
    }

    /// Remove container
    #[allow(deprecated)]
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
