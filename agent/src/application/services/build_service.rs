use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

use crate::application::ports::repositories::{BuildRepository, ProjectRepository};
use crate::application::events::{EventBus, Event};
use crate::db::models::{BuildStatus, Project, Build};
use crate::docker::DockerClient;
use crate::infrastructure::logging::{BoundaryLogger, Timer};

/// BuildService - 빌드 실행을 담당하는 서비스
///
/// 책임:
/// - Git 소스 준비 (clone/pull)
/// - 빌드 컨테이너 실행
/// - 빌드 로그 수집 및 저장
/// - 빌드 상태 업데이트
/// - 이벤트 발행
pub struct BuildService<BR, PR, EB>
where
    BR: BuildRepository,
    PR: ProjectRepository,
    EB: EventBus,
{
    build_repo: Arc<BR>,
    project_repo: Arc<PR>,
    event_bus: EB,
    docker: DockerClient,
    logger: Arc<BoundaryLogger>,
}

impl<BR, PR, EB> BuildService<BR, PR, EB>
where
    BR: BuildRepository,
    PR: ProjectRepository,
    EB: EventBus,
{
    pub fn new(
        build_repo: Arc<BR>,
        project_repo: Arc<PR>,
        event_bus: EB,
        docker: DockerClient,
        logger: Arc<BoundaryLogger>,
    ) -> Self {
        Self {
            build_repo,
            project_repo,
            event_bus,
            docker,
            logger,
        }
    }

    /// 빌드 실행
    pub async fn execute_build(&self, trace_id: &str, build_id: i64) -> Result<PathBuf> {
        let timer = Timer::start();
        self.logger.service_entry(trace_id, "API", "BuildService", "execute_build", &build_id);

        // Get build
        self.logger.repo_call(trace_id, "BuildService", "BuildRepo", "get");
        let repo_timer = Timer::start();
        let build = self.build_repo.get(build_id).await?
            .context("Build not found")?;
        self.logger.repo_done(trace_id, "BuildService", "BuildRepo", "get", repo_timer.elapsed_ms());

        // Get project
        self.logger.repo_call(trace_id, "BuildService", "ProjectRepo", "get");
        let repo_timer = Timer::start();
        let project = self.project_repo.get(build.project_id).await?
            .context("Project not found")?;
        self.logger.repo_done(trace_id, "BuildService", "ProjectRepo", "get", repo_timer.elapsed_ms());

        info!(
            "[{}] Executing build #{} for project {}",
            trace_id, build.build_number, project.name
        );

        // Update status to Building
        self.logger.repo_call(trace_id, "BuildService", "BuildRepo", "update_status");
        self.build_repo.update_status(build_id, BuildStatus::Building).await?;

        // Emit event
        self.logger.event_emit(trace_id, "BuildService", "BuildStatus::Building");
        self.event_bus.emit(Event::BuildStatus {
            build_id: build.id,
            project_id: project.id,
            status: BuildStatus::Building,
            timestamp: Event::now(),
        }).await;

        // Setup paths (using project ID instead of name)
        let workspace_path = PathBuf::from("/data/workspace").join(project.id.to_string());
        let output_path = PathBuf::from("/data/output").join(format!("build{}", build.id));
        let cache_path = PathBuf::from("/data/cache").join(&project.cache_type);
        let log_path = PathBuf::from(&build.log_path);

        // Create directories
        fs::create_dir_all(&workspace_path).await.context("Failed to create workspace directory")?;
        fs::create_dir_all(&output_path).await.context("Failed to create output directory")?;
        fs::create_dir_all(&cache_path).await.context("Failed to create cache directory")?;
        fs::create_dir_all(log_path.parent().unwrap()).await.context("Failed to create log directory")?;

        // Prepare source
        self.prepare_source(trace_id, &workspace_path, &project.repo, &project.branch).await?;

        // Verify workspace
        self.verify_workspace(&workspace_path).await?;

        // Open log file
        let mut log_file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .await
            .context("Failed to open log file")?;

        // Create nginx config if needed
        if project.runtime_image.contains("nginx") {
            self.create_nginx_config(&output_path).await?;
        }

        // Run build container
        self.logger.external_call(trace_id, "BuildService", "Docker", "run_build_container");
        let docker_timer = Timer::start();

        let result = self.docker.run_build_container(
            &project.build_image,
            &project.build_command,
            workspace_path,
            output_path.clone(),
            cache_path,
            &project.cache_type,
            project.working_directory.as_deref().unwrap_or(""),
        ).await;

        match result {
            Ok((container_id, logs)) => {
                self.logger.external_done(trace_id, "BuildService", "Docker", "run_build_container", docker_timer.elapsed_ms());
                info!("[{}] Build #{} completed successfully", trace_id, build.build_number);

                // Write logs and emit events
                for (idx, line) in logs.iter().enumerate() {
                    log_file.write_all(line.as_bytes()).await.context("Failed to write log")?;
                    log_file.write_all(b"\n").await.ok();

                    self.event_bus.emit(Event::Log {
                        build_id: build.id,
                        line: line.clone(),
                        line_number: idx,
                        timestamp: Event::now(),
                    }).await;
                }

                log_file.flush().await.ok();

                self.logger.service_exit(trace_id, "API", "BuildService", "execute_build", timer.elapsed_ms());
                Ok(output_path)
            }
            Err(e) => {
                self.logger.external_error(trace_id, "BuildService", "Docker", "run_build_container", &e);
                warn!("[{}] Build #{} failed: {}", trace_id, build.build_number, e);

                // Write error to log
                let error_msg = format!("Build failed: {}", e);
                log_file.write_all(error_msg.as_bytes()).await.ok();
                log_file.flush().await.ok();

                // Update status to Failed
                self.logger.repo_call(trace_id, "BuildService", "BuildRepo", "update_status");
                self.build_repo.update_status(build.id, BuildStatus::Failed).await?;

                // Emit events
                self.logger.event_emit(trace_id, "BuildService", "BuildStatus::Failed");
                self.event_bus.emit(Event::BuildStatus {
                    build_id: build.id,
                    project_id: project.id,
                    status: BuildStatus::Failed,
                    timestamp: Event::now(),
                }).await;

                self.event_bus.emit(Event::Error {
                    project_id: Some(project.id),
                    build_id: Some(build.id),
                    message: error_msg.clone(),
                    timestamp: Event::now(),
                }).await;

                self.logger.service_error(trace_id, "API", "BuildService", "execute_build", &e);
                anyhow::bail!("Build failed: {}", e);
            }
        }
    }

    /// Git 소스 준비 (clone 또는 pull)
    async fn prepare_source(&self, trace_id: &str, workspace_path: &PathBuf, repo: &str, branch: &str) -> Result<()> {
        self.logger.external_call(trace_id, "BuildService", "Git", "prepare_source");
        let timer = Timer::start();

        let git_dir = workspace_path.join(".git");
        if workspace_path.exists() && git_dir.exists() {
            info!("[{}] Pulling latest changes for {}", trace_id, repo);
            let status = tokio::process::Command::new("git")
                .arg("-C")
                .arg(workspace_path)
                .arg("pull")
                .arg("origin")
                .arg(branch)
                .status()
                .await
                .context("Failed to pull repository")?;

            if !status.success() {
                anyhow::bail!("Git pull failed");
            }
        } else {
            info!("[{}] Cloning repository: {}", trace_id, repo);
            let status = tokio::process::Command::new("git")
                .arg("clone")
                .arg("--depth")
                .arg("1")
                .arg("--branch")
                .arg(branch)
                .arg(repo)
                .arg(workspace_path)
                .status()
                .await
                .context("Failed to clone repository")?;

            if !status.success() {
                anyhow::bail!("Git clone failed");
            }
        }

        self.logger.external_done(trace_id, "BuildService", "Git", "prepare_source", timer.elapsed_ms());
        Ok(())
    }

    /// Workspace 검증
    async fn verify_workspace(&self, workspace_path: &PathBuf) -> Result<()> {
        let mut entries = fs::read_dir(workspace_path).await?;
        let mut has_files = false;

        while let Some(entry) = entries.next_entry().await? {
            if entry.file_name() != ".git" {
                has_files = true;
                break;
            }
        }

        if !has_files {
            anyhow::bail!("Workspace is empty (no files except .git)");
        }

        Ok(())
    }

    /// Nginx 설정 파일 생성
    async fn create_nginx_config(&self, output_path: &PathBuf) -> Result<()> {
        let nginx_conf = r#"
daemon off;
worker_processes 1;
error_log /dev/stdout info;

events {
    worker_connections 1024;
}

http {
    include /etc/nginx/mime.types;
    default_type application/octet-stream;
    access_log /dev/stdout;

    server {
        listen 8080;
        root /app;
        index index.html;

        location / {
            try_files $uri $uri/ /index.html;
        }
    }
}
"#;

        let config_path = output_path.join("nginx.conf");
        fs::write(&config_path, nginx_conf).await.context("Failed to write nginx.conf")?;

        info!("Created nginx.conf at {}", config_path.display());
        Ok(())
    }
}
