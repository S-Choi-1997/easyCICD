use anyhow::{Context, Result};
use bollard::container::{
    Config, CreateContainerOptions, InspectContainerOptions, LogOutput, RemoveContainerOptions,
    RestartContainerOptions, StartContainerOptions, StopContainerOptions,
};
use bollard::exec::{CreateExecOptions, ResizeExecOptions, StartExecOptions, StartExecResults};
use bollard::image::CreateImageOptions;
use bollard::Docker;
use futures_util::StreamExt;
use serde_json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::time::timeout;
use tracing::{debug, info, warn};

/// Build container execution result
pub struct BuildResult {
    pub success: bool,
    pub exit_code: i64,
    pub logs: Vec<String>,
    pub container_id: String,
}

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
        // Check if image already exists locally
        let images = self.docker.list_images(None::<bollard::image::ListImagesOptions<String>>).await?;
        let image_with_tag = if image.contains(':') {
            image.to_string()
        } else {
            format!("{}:latest", image)
        };
        let image_exists = images.iter().any(|img| {
            img.repo_tags.iter().any(|tag| {
                tag == &image_with_tag
            })
        });

        if image_exists {
            info!("Image {} already exists locally", image);
            return Ok(());
        }

        info!("Pulling image: {} (this may take a while...)", image);

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
        let mut last_status = String::new();

        while let Some(result) = stream.next().await {
            match result {
                Ok(info) => {
                    if let Some(status) = &info.status {
                        // Log progress for important status changes
                        if status != &last_status {
                            if status.contains("Pulling") || status.contains("Downloading") || status.contains("Extracting") {
                                if let Some(id) = &info.id {
                                    info!("[{}] Pulling {}: {} {}", image, id, status,
                                        info.progress.as_deref().unwrap_or(""));
                                }
                            } else if status.contains("Pull complete") || status.contains("Download complete") {
                                if let Some(id) = &info.id {
                                    info!("[{}] Layer {} complete", image, id);
                                }
                            } else if status.contains("Digest") || status.contains("Status") {
                                info!("[{}] {}", image, status);
                            }
                            last_status = status.clone();
                        }
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

        info!("Image {} pulled successfully", image);
        Ok(())
    }

    /// Run build container (git clone happens inside container)
    /// Returns BuildResult with success/failure status and logs
    pub async fn run_build_container(
        &self,
        image: &str,
        command: &str,
        output_path: PathBuf,
        cache_path: PathBuf,
        cache_type: &str,
    ) -> Result<BuildResult> {
        self.ensure_image(image).await?;

        let container_name = format!("build-{}", uuid::Uuid::new_v4());

        // Convert container paths to host paths for DOOD
        let host_output = self.to_host_path(&output_path);
        let host_cache = self.to_host_path(&cache_path);

        info!("Build container mounts:");
        info!("  Output: {} (host: {})", output_path.display(), host_output.display());
        info!("  Cache: {} (host: {})", cache_path.display(), host_cache.display());

        let mut binds = vec![
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

        let config = Config {
            image: Some(image.to_string()),
            cmd: Some(vec!["/bin/sh".to_string(), "-c".to_string(), command.to_string()]),
            working_dir: Some("/".to_string()),  // Start at root, git clone creates /workspace
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

        // Collect logs with timeout (30 minutes max for long builds)
        let build_timeout = Duration::from_secs(30 * 60);
        let max_log_lines = 100_000;  // Prevent memory exhaustion

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
        let log_collection = async {
            while let Some(log_result) = log_stream.next().await {
                match log_result {
                    Ok(output) => {
                        let line = match output {
                            LogOutput::StdOut { message } => String::from_utf8_lossy(&message).to_string(),
                            LogOutput::StdErr { message } => String::from_utf8_lossy(&message).to_string(),
                            _ => continue,
                        };
                        // Print log immediately to stdout for real-time visibility
                        println!("[BUILD {}] {}", container_id, line.trim_end());
                        logs.push(line);

                        // Prevent memory exhaustion
                        if logs.len() >= max_log_lines {
                            warn!("Log limit reached ({} lines), truncating", max_log_lines);
                            logs.push(format!("... truncated after {} lines", max_log_lines));
                            break;
                        }
                    }
                    Err(e) => {
                        warn!("Error reading logs: {}", e);
                        let error_msg = format!("ERROR: Failed to read logs: {}", e);
                        println!("[BUILD-ERROR {}] {}", container_id, error_msg);
                        logs.push(error_msg);
                        break;
                    }
                }
            }
        };

        if timeout(build_timeout, log_collection).await.is_err() {
            warn!("Build timeout after {} minutes for container {}", build_timeout.as_secs() / 60, container_id);
            logs.push(format!("ERROR: Build timed out after {} minutes", build_timeout.as_secs() / 60));

            // Force stop the container
            let _ = self.docker.stop_container(
                &container_id,
                Some(StopContainerOptions { t: 5 }),
            ).await;
        }

        info!("Collected {} lines of logs from container {}", logs.len(), container_id);
        if logs.is_empty() {
            warn!("No logs collected from container {} - container may have exited immediately", container_id);
        }

        // Wait for container to finish (with timeout)
        let wait_timeout = Duration::from_secs(60);  // 1 minute to wait after logs
        let wait_result = timeout(
            wait_timeout,
            self.docker
                .wait_container(&container_id, None::<bollard::container::WaitContainerOptions<&str>>)
                .next()
        ).await;

        let exit_code = match wait_result {
            Ok(Some(Ok(result))) => {
                info!("Container {} exited with code: {}", container_id, result.status_code);
                result.status_code
            }
            Ok(Some(Err(e))) => {
                tracing::error!("Failed to wait for container {}: {}", container_id, e);
                // Try to inspect container to get exit code
                self.get_container_exit_code(&container_id).await
            }
            Ok(None) => {
                tracing::error!("Container {} wait returned None (container may have been killed)", container_id);
                self.get_container_exit_code(&container_id).await
            }
            Err(_) => {
                tracing::error!("Timeout waiting for container {} to finish", container_id);
                // Force stop the container
                let _ = self.docker.stop_container(
                    &container_id,
                    Some(StopContainerOptions { t: 5 }),
                ).await;
                -2  // Special code for timeout
            }
        };

        // Remove build container (cleanup)
        if let Err(e) = self
            .docker
            .remove_container(
                &container_id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await
        {
            warn!("Failed to remove build container {}: {}", container_id, e);
        }

        Ok(BuildResult {
            success: exit_code == 0,
            exit_code,
            logs,
            container_id,
        })
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
        env_vars: Option<&str>,
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

        // Build environment variables list
        let mut env = vec![format!("PORT={}", runtime_port)];

        // Parse and add user-defined environment variables (JSON format)
        if let Some(env_json) = env_vars {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(env_json) {
                for (key, value) in parsed {
                    let val_str = match value {
                        serde_json::Value::String(s) => s,
                        other => other.to_string().trim_matches('"').to_string(),
                    };
                    env.push(format!("{}={}", key, val_str));
                }
            }
        }

        let config = Config {
            image: Some(image.to_string()),
            cmd: Some(vec!["/bin/sh".to_string(), "-c".to_string(), command.to_string()]),
            working_dir: Some("/app".to_string()),
            env: Some(env),
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
    /// Check if image needs to be pulled (returns true if pull is needed)
    pub async fn needs_image_pull(&self, image: &str) -> bool {
        let images = match self.docker.list_images(None::<bollard::image::ListImagesOptions<String>>).await {
            Ok(imgs) => imgs,
            Err(_) => return true,
        };
        let image_with_tag = if image.contains(':') {
            image.to_string()
        } else {
            format!("{}:latest", image)
        };
        !images.iter().any(|img| {
            img.repo_tags.iter().any(|tag| tag == &image_with_tag)
        })
    }

    pub async fn run_standalone_container(
        &self,
        name: &str,
        image: &str,
        host_port: i32,
        container_port: i32,
        env_vars: Option<&str>,
        command: Option<&str>,
        persist_data: bool,
    ) -> Result<String> {
        self.ensure_image(image).await?;

        let container_name = format!("container-{}", name);

        // Stop and remove existing container with same name
        let _ = self.stop_container(&container_name).await;
        let _ = self.remove_container(&container_name).await;

        // Parse env vars from JSON if provided
        let mut env: Vec<String> = env_vars.and_then(|s| {
            serde_json::from_str::<serde_json::Value>(s)
                .ok()
                .and_then(|v| {
                    v.as_object().map(|obj| {
                        obj.iter()
                            .map(|(k, v)| format!("{}={}", k, v.as_str().unwrap_or(&v.to_string())))
                            .collect()
                    })
                })
        }).unwrap_or_default();

        // Add data directory env vars for known databases if persist_data is enabled
        if persist_data {
            let image_lower = image.to_lowercase();
            if image_lower.contains("postgres") || image_lower.contains("timescale") {
                // PostgreSQL/TimescaleDB: Set PGDATA to use /data directory
                env.push("PGDATA=/data".to_string());
                info!("Added PGDATA=/data for PostgreSQL/TimescaleDB");
            } else if image_lower.contains("mysql") || image_lower.contains("mariadb") {
                // MySQL/MariaDB: Set data directory
                env.push("MYSQL_DATADIR=/data".to_string());
                info!("Added MYSQL_DATADIR=/data for MySQL/MariaDB");
            } else if image_lower.contains("mongo") {
                // MongoDB: Set data directory
                env.push("MONGODB_DBPATH=/data".to_string());
                info!("Added MONGODB_DBPATH=/data for MongoDB");
            } else if image_lower.contains("redis") {
                // Redis: Set data directory and enable persistence
                env.push("REDIS_DATA_DIR=/data".to_string());
                info!("Added REDIS_DATA_DIR=/data for Redis");
            } else if image_lower.contains("elasticsearch") || image_lower.contains("opensearch") {
                // Elasticsearch/OpenSearch: Set data path
                env.push("path.data=/data".to_string());
                info!("Added path.data=/data for Elasticsearch/OpenSearch");
            } else if image_lower.contains("cassandra") || image_lower.contains("scylla") {
                // Cassandra/ScyllaDB: Set data directory
                env.push("CASSANDRA_DATA_DIR=/data".to_string());
                info!("Added CASSANDRA_DATA_DIR=/data for Cassandra/ScyllaDB");
            } else if image_lower.contains("couchdb") {
                // CouchDB: Set data directory
                env.push("COUCHDB_DATA_DIR=/data".to_string());
                info!("Added COUCHDB_DATA_DIR=/data for CouchDB");
            } else if image_lower.contains("influxdb") {
                // InfluxDB: Set data paths
                env.push("INFLUXDB_DATA_DIR=/data".to_string());
                env.push("INFLUXDB_META_DIR=/data/meta".to_string());
                env.push("INFLUXDB_WAL_DIR=/data/wal".to_string());
                info!("Added INFLUXDB_*_DIR=/data for InfluxDB");
            } else if image_lower.contains("neo4j") {
                // Neo4j: Data already in /data by default
                env.push("NEO4J_DATA=/data".to_string());
                info!("Added NEO4J_DATA=/data for Neo4j");
            } else if image_lower.contains("rabbitmq") {
                // RabbitMQ: Set data directory
                env.push("RABBITMQ_MNESIA_BASE=/data".to_string());
                info!("Added RABBITMQ_MNESIA_BASE=/data for RabbitMQ");
            }
        }

        let env_option = if env.is_empty() { None } else { Some(env) };

        // Parse command if provided
        let cmd: Option<Vec<String>> = command.map(|c| {
            vec!["/bin/sh".to_string(), "-c".to_string(), c.to_string()]
        });

        // Setup volume binds if persist_data is enabled
        let binds: Option<Vec<String>> = if persist_data {
            // Create data directory if it doesn't exist
            let data_dir = format!("/data/easycicd/containers/{}/data", name);
            std::fs::create_dir_all(&data_dir).ok();

            // Mount to /data in container
            info!("Mounting persistent data: {} -> /data", data_dir);
            Some(vec![format!("{}:/data", data_dir)])
        } else {
            None
        };

        let config = Config {
            image: Some(image.to_string()),
            cmd,
            env: env_option,
            host_config: Some(bollard::models::HostConfig {
                port_bindings: Some({
                    let mut port_bindings = HashMap::new();
                    // Map host_port:container_port
                    info!("Port mapping: {}:{}", host_port, container_port);
                    port_bindings.insert(
                        format!("{}/tcp", container_port),
                        Some(vec![bollard::models::PortBinding {
                            host_ip: Some("0.0.0.0".to_string()),
                            host_port: Some(host_port.to_string()),
                        }]),
                    );
                    port_bindings
                }),
                binds,
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

    /// Stop container (logs error but doesn't fail for cleanup scenarios)
    pub async fn stop_container(&self, container_id: &str) -> Result<()> {
        info!("Stopping container: {}", container_id);
        if let Err(e) = self.docker
            .stop_container(
                container_id,
                Some(StopContainerOptions {
                    t: 10, // 10 seconds timeout
                }),
            )
            .await
        {
            // Log the error but don't fail - container might already be stopped or removed
            warn!("Failed to stop container {}: {} (may already be stopped)", container_id, e);
        }
        Ok(())
    }

    /// Get container logs
    pub async fn get_container_logs(&self, container_id: &str, tail: Option<usize>) -> Result<Vec<String>> {
        use bollard::container::LogsOptions;

        let options = LogsOptions::<String> {
            stdout: true,
            stderr: true,
            tail: tail.unwrap_or(100).to_string(),
            timestamps: true,
            ..Default::default()
        };

        let mut logs = self.docker.logs(container_id, Some(options));
        let mut result = Vec::new();

        while let Some(log) = logs.next().await {
            match log {
                Ok(output) => {
                    let line = match output {
                        LogOutput::StdOut { message } | LogOutput::StdErr { message } => {
                            String::from_utf8_lossy(&message).to_string()
                        }
                        _ => continue,
                    };
                    result.push(line);
                }
                Err(e) => {
                    warn!("Error reading container logs: {}", e);
                    break;
                }
            }
        }

        Ok(result)
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

    /// Remove container (logs error but doesn't fail for cleanup scenarios)
    pub async fn remove_container(&self, container_id: &str) -> Result<()> {
        info!("Removing container: {}", container_id);
        if let Err(e) = self.docker
            .remove_container(
                container_id,
                Some(RemoveContainerOptions {
                    force: true,
                    ..Default::default()
                }),
            )
            .await
        {
            // Log the error but don't fail - container might already be removed
            warn!("Failed to remove container {}: {} (may already be removed)", container_id, e);
        }
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

    /// Get container exit code by inspecting container state
    async fn get_container_exit_code(&self, container_id: &str) -> i64 {
        if let Ok(inspect) = self.docker.inspect_container(container_id, None::<InspectContainerOptions>).await {
            if let Some(state) = inspect.state {
                tracing::error!("Container state: running={:?}, exit_code={:?}, error={:?}",
                    state.running, state.exit_code, state.error);
                if let Some(code) = state.exit_code {
                    return code;
                }
            }
        }
        -1  // Unknown exit code
    }

    /// 컨테이너 로그 스트리밍 (실시간 로그)
    /// tail: 초기 로드할 로그 줄 수 (None이면 기본 500줄, Some("all")이면 전체)
    pub async fn stream_container_logs(&self, container_id: &str, tail: Option<&str>) -> Result<impl futures_util::Stream<Item = Result<Vec<u8>, bollard::errors::Error>>> {
        use bollard::container::LogsOptions;

        let options = Some(LogsOptions::<String> {
            follow: true,
            stdout: true,
            stderr: true,
            tail: tail.unwrap_or("500").to_string(),
            ..Default::default()
        });

        let stream = self.docker.logs(container_id, options);

        Ok(stream.map(|result| {
            result.map(|output| match output {
                LogOutput::StdOut { message } => message.to_vec(),
                LogOutput::StdErr { message } => message.to_vec(),
                _ => Vec::new(),
            })
        }))
    }

    /// 컨테이너 내부에서 명령 실행 (exec 세션 생성)
    pub async fn create_exec_session(
        &self,
        container_id: &str,
        cmd: Vec<String>,
    ) -> Result<(String, StartExecResults)> {
        let exec_config = CreateExecOptions {
            attach_stdin: Some(true),
            attach_stdout: Some(true),
            attach_stderr: Some(true),
            tty: Some(true),
            cmd: Some(cmd),
            ..Default::default()
        };

        let exec_instance = self.docker
            .create_exec(container_id, exec_config)
            .await
            .context("Failed to create exec instance")?;

        let start_config = StartExecOptions {
            detach: false,
            tty: true,
            ..Default::default()
        };

        let output = self.docker
            .start_exec(&exec_instance.id, Some(start_config))
            .await
            .context("Failed to start exec")?;

        Ok((exec_instance.id, output))
    }

    /// exec PTY 크기 조정
    pub async fn resize_exec_tty(
        &self,
        exec_id: &str,
        height: u16,
        width: u16,
    ) -> Result<()> {
        self.docker
            .resize_exec(
                exec_id,
                ResizeExecOptions { height, width },
            )
            .await
            .context("Failed to resize exec TTY")?;

        Ok(())
    }
}
