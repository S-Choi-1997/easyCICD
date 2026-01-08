use anyhow::{Context, Result};
use std::path::PathBuf;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

use crate::db::models::{Build, BuildStatus, Project};
use crate::docker::DockerClient;
use crate::events::Event;
use crate::state::AppState;

pub struct BuildExecutor {
    pub state: AppState,
    pub docker: DockerClient,
}

impl BuildExecutor {
    pub fn new(state: AppState, docker: DockerClient) -> Self {
        Self { state, docker }
    }

    pub async fn execute_build(&self, project: &Project, build: &Build) -> Result<PathBuf> {
        info!(
            "Executing build #{} for project {}",
            build.build_number, project.name
        );

        // Update status to Building
        self.state
            .db
            .update_build_status(build.id, BuildStatus::Building)
            .await?;

        self.state.emit_event(Event::BuildStatus {
            build_id: build.id,
            project_id: project.id,
            status: BuildStatus::Building,
            timestamp: Event::now(),
        });

        // Setup paths
        let workspace_path = PathBuf::from("/data/workspace").join(&project.name);
        let output_path = PathBuf::from("/data/output").join(format!("build{}", build.id));
        let cache_path = PathBuf::from("/data/cache").join(&project.cache_type);
        let log_path = PathBuf::from(&build.log_path);

        // Create directories
        fs::create_dir_all(&workspace_path)
            .await
            .context("Failed to create workspace directory")?;
        fs::create_dir_all(&output_path)
            .await
            .context("Failed to create output directory")?;
        fs::create_dir_all(&cache_path)
            .await
            .context("Failed to create cache directory")?;
        fs::create_dir_all(log_path.parent().unwrap())
            .await
            .context("Failed to create log directory")?;

        // Clone or pull repository
        self.prepare_source(&workspace_path, &project.repo, &project.branch)
            .await?;

        // Verify workspace has expected files
        info!("Verifying workspace files at: {}", workspace_path.display());
        self.verify_workspace(&workspace_path).await?;

        // Open log file
        let mut log_file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .await
            .context("Failed to open log file")?;

        // Create nginx config if runtime is nginx
        if project.runtime_image.contains("nginx") {
            self.create_nginx_config(&output_path).await?;
        }

        // Run build container
        info!("Running build container for build #{}", build.build_number);
        let result = self
            .docker
            .run_build_container(
                &project.build_image,
                &project.build_command,
                workspace_path,
                output_path.clone(),
                cache_path,
                &project.cache_type,
                project.working_directory.as_deref().unwrap_or(""),
            )
            .await;

        match result {
            Ok((container_id, logs)) => {
                info!("Build #{} completed successfully", build.build_number);

                // Write logs to file and emit events
                for (idx, line) in logs.iter().enumerate() {
                    log_file
                        .write_all(line.as_bytes())
                        .await
                        .context("Failed to write log")?;
                    log_file.write_all(b"\n").await.ok();

                    // Emit log event
                    self.state.emit_event(Event::Log {
                        build_id: build.id,
                        line: line.clone(),
                        line_number: idx,
                        timestamp: Event::now(),
                    });
                }

                log_file.flush().await.ok();

                Ok(output_path)
            }
            Err(e) => {
                warn!("Build #{} failed: {}", build.build_number, e);

                // Write error to log
                let error_msg = format!("Build failed: {}", e);
                log_file.write_all(error_msg.as_bytes()).await.ok();
                log_file.flush().await.ok();

                // Update status to Failed
                self.state
                    .db
                    .update_build_status(build.id, BuildStatus::Failed)
                    .await?;

                self.state.emit_event(Event::BuildStatus {
                    build_id: build.id,
                    project_id: project.id,
                    status: BuildStatus::Failed,
                    timestamp: Event::now(),
                });

                self.state.emit_event(Event::Error {
                    project_id: Some(project.id),
                    build_id: Some(build.id),
                    message: error_msg.clone(),
                    timestamp: Event::now(),
                });

                anyhow::bail!("Build failed: {}", e);
            }
        }
    }

    async fn prepare_source(
        &self,
        workspace_path: &PathBuf,
        repo: &str,
        branch: &str,
    ) -> Result<()> {
        info!("Preparing source code at {:?}", workspace_path);

        if workspace_path.join(".git").exists() {
            info!("Repository exists, pulling latest changes");

            let output = tokio::process::Command::new("git")
                .current_dir(workspace_path)
                .args(&["pull", "origin", branch])
                .output()
                .await
                .context("Failed to pull git repository")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Git pull failed: {}", stderr);
                anyhow::bail!("Git pull failed: {}", stderr);
            }
        } else {
            info!("Cloning repository: {}", repo);

            let git_url = if repo.starts_with("http") {
                repo.to_string()
            } else {
                format!("https://github.com/{}.git", repo)
            };

            let output = tokio::process::Command::new("git")
                .args(&["clone", "-b", branch, &git_url, workspace_path.to_str().unwrap()])
                .output()
                .await
                .context("Failed to clone git repository")?;

            if !output.status.success() {
                let stderr = String::from_utf8_lossy(&output.stderr);
                warn!("Git clone failed: {}", stderr);
                anyhow::bail!("Git clone failed: {}", stderr);
            }
        }

        // Get current commit hash
        let output = tokio::process::Command::new("git")
            .current_dir(workspace_path)
            .args(&["rev-parse", "HEAD"])
            .output()
            .await
            .context("Failed to get commit hash")?;

        let commit_hash = String::from_utf8_lossy(&output.stdout).trim().to_string();
        info!("Current commit: {}", commit_hash);

        Ok(())
    }

    async fn verify_workspace(&self, workspace_path: &PathBuf) -> Result<()> {
        // Check if workspace directory exists
        if !workspace_path.exists() {
            anyhow::bail!("Workspace directory does not exist: {}", workspace_path.display());
        }

        // List directory contents
        let mut entries = fs::read_dir(workspace_path).await
            .context("Failed to read workspace directory")?;

        let mut file_count = 0;
        let mut files = Vec::new();

        while let Some(entry) = entries.next_entry().await? {
            file_count += 1;
            if file_count <= 10 {
                files.push(entry.file_name().to_string_lossy().to_string());
            }
        }

        if file_count == 0 {
            anyhow::bail!("Workspace directory is empty: {}", workspace_path.display());
        }

        info!("Workspace verification passed - {} files/dirs found", file_count);
        info!("First files: {:?}", files);

        // Check for common build files
        let common_files = ["package.json", "pom.xml", "build.gradle", "Cargo.toml", "Makefile", "Dockerfile"];
        let mut found_files = Vec::new();

        for file in &common_files {
            if workspace_path.join(file).exists() {
                found_files.push(*file);
            }
        }

        if !found_files.is_empty() {
            info!("Detected build files: {:?}", found_files);
        }

        Ok(())
    }

    async fn create_nginx_config(&self, output_path: &PathBuf) -> Result<()> {
        let nginx_conf = r#"daemon off;
error_log /dev/stdout info;
pid /tmp/nginx.pid;

events {
    worker_connections 1024;
}

http {
    include /etc/nginx/mime.types;
    default_type application/octet-stream;

    access_log /dev/stdout;
    client_body_temp_path /tmp/client_body;
    proxy_temp_path /tmp/proxy;
    fastcgi_temp_path /tmp/fastcgi;

    server {
        listen 8080;
        server_name _;
        root /app;
        index index.html;

        location / {
            try_files $uri $uri/ /index.html;
        }

        # Cache static assets
        location ~* \.(jpg|jpeg|png|gif|ico|css|js|svg|woff|woff2|ttf|eot)$ {
            expires 1y;
            add_header Cache-Control "public, immutable";
        }

        # Health check endpoint
        location = /health {
            access_log off;
            return 200 "OK";
            add_header Content-Type text/plain;
        }
    }
}
"#;

        let conf_path = output_path.join("nginx.conf");
        fs::write(&conf_path, nginx_conf)
            .await
            .context("Failed to write nginx config")?;

        info!("Created nginx config at: {}", conf_path.display());
        Ok(())
    }
}
