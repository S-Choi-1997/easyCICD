use anyhow::{Context, Result};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::process::Command;
use tracing::{info, warn};

use crate::application::ports::repositories::{BuildRepository, ProjectRepository};
use crate::application::events::{EventBus, Event};
use crate::db::models::{CreateBuild, CreateProject, Project, Build, Slot};
use crate::docker::DockerClient;
use crate::infrastructure::logging::{BoundaryLogger, Timer};

/// ProjectService - 프로젝트 생명주기 관리를 담당하는 서비스
///
/// 책임:
/// - 프로젝트 CRUD 작업
/// - 컨테이너 시작/중지/재시작
/// - 빌드 트리거 (Git 정보 수집)
/// - 프로젝트 삭제 시 리소스 정리
/// - 이벤트 발행
pub struct ProjectService<PR, BR, EB>
where
    PR: ProjectRepository,
    BR: BuildRepository,
    EB: EventBus,
{
    project_repo: Arc<PR>,
    build_repo: Arc<BR>,
    event_bus: EB,
    docker: DockerClient,
    logger: Arc<BoundaryLogger>,
}

impl<PR, BR, EB> ProjectService<PR, BR, EB>
where
    PR: ProjectRepository,
    BR: BuildRepository,
    EB: EventBus,
{
    pub fn new(
        project_repo: Arc<PR>,
        build_repo: Arc<BR>,
        event_bus: EB,
        docker: DockerClient,
        logger: Arc<BoundaryLogger>,
    ) -> Self {
        Self {
            project_repo,
            build_repo,
            event_bus,
            docker,
            logger,
        }
    }

    /// 프로젝트 목록 조회
    pub async fn list_projects(&self, trace_id: &str) -> Result<Vec<Project>> {
        let timer = Timer::start();
        self.logger.service_entry(trace_id, "API", "ProjectService", "list_projects", &());

        self.logger.repo_call(trace_id, "ProjectService", "ProjectRepo", "list");
        let repo_timer = Timer::start();
        let projects = self.project_repo.list().await?;
        self.logger.repo_done(trace_id, "ProjectService", "ProjectRepo", "list", repo_timer.elapsed_ms());

        self.logger.service_exit(trace_id, "API", "ProjectService", "list_projects", timer.elapsed_ms());
        Ok(projects)
    }

    /// 프로젝트 단건 조회
    pub async fn get_project(&self, trace_id: &str, id: i64) -> Result<Option<Project>> {
        let timer = Timer::start();
        self.logger.service_entry(trace_id, "API", "ProjectService", "get_project", &id);

        self.logger.repo_call(trace_id, "ProjectService", "ProjectRepo", "get");
        let repo_timer = Timer::start();
        let project = self.project_repo.get(id).await?;
        self.logger.repo_done(trace_id, "ProjectService", "ProjectRepo", "get", repo_timer.elapsed_ms());

        self.logger.service_exit(trace_id, "API", "ProjectService", "get_project", timer.elapsed_ms());
        Ok(project)
    }

    /// 프로젝트 생성
    pub async fn create_project(&self, trace_id: &str, create_project: CreateProject) -> Result<Project> {
        let timer = Timer::start();
        self.logger.service_entry(trace_id, "API", "ProjectService", "create_project", &create_project.name);

        self.logger.repo_call(trace_id, "ProjectService", "ProjectRepo", "create");
        let repo_timer = Timer::start();
        let project = self.project_repo.create(create_project).await?;
        self.logger.repo_done(trace_id, "ProjectService", "ProjectRepo", "create", repo_timer.elapsed_ms());

        self.logger.service_exit(trace_id, "API", "ProjectService", "create_project", timer.elapsed_ms());
        Ok(project)
    }

    /// 빌드 트리거 (Git 정보 수집 및 빌드 생성)
    pub async fn trigger_build(&self, trace_id: &str, project_id: i64) -> Result<Build> {
        let timer = Timer::start();
        self.logger.service_entry(trace_id, "API", "ProjectService", "trigger_build", &project_id);

        // Get project
        self.logger.repo_call(trace_id, "ProjectService", "ProjectRepo", "get");
        let repo_timer = Timer::start();
        let project = self.project_repo.get(project_id).await?
            .context("Project not found")?;
        self.logger.repo_done(trace_id, "ProjectService", "ProjectRepo", "get", repo_timer.elapsed_ms());

        // Get current commit hash from workspace
        let workspace_path = PathBuf::from("/data/workspace")
            .join(&project.name);

        self.logger.external_call(trace_id, "ProjectService", "Git", "rev-parse");
        let git_timer = Timer::start();

        let commit_hash = if workspace_path.exists() {
            match Command::new("git")
                .args(&["-C", workspace_path.to_str().unwrap(), "rev-parse", "HEAD"])
                .output()
                .await
            {
                Ok(output) if output.status.success() => {
                    String::from_utf8_lossy(&output.stdout).trim().to_string()
                }
                _ => "HEAD".to_string(),
            }
        } else {
            "HEAD".to_string()
        };

        self.logger.external_done(trace_id, "ProjectService", "Git", "rev-parse", git_timer.elapsed_ms());

        // Get commit message and author
        let (commit_message, author) = if workspace_path.exists() && commit_hash != "HEAD" {
            self.logger.external_call(trace_id, "ProjectService", "Git", "log");
            let git_timer = Timer::start();

            let message = Command::new("git")
                .args(&[
                    "-C",
                    workspace_path.to_str().unwrap(),
                    "log",
                    "-1",
                    "--format=%s",
                ])
                .output()
                .await
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                    } else {
                        None
                    }
                });

            let author = Command::new("git")
                .args(&[
                    "-C",
                    workspace_path.to_str().unwrap(),
                    "log",
                    "-1",
                    "--format=%an <%ae>",
                ])
                .output()
                .await
                .ok()
                .and_then(|o| {
                    if o.status.success() {
                        Some(String::from_utf8_lossy(&o.stdout).trim().to_string())
                    } else {
                        None
                    }
                });

            self.logger.external_done(trace_id, "ProjectService", "Git", "log", git_timer.elapsed_ms());

            (message, author)
        } else {
            (None, None)
        };

        // Create build
        let create_build = CreateBuild {
            project_id: project.id,
            commit_hash,
            commit_message,
            author,
        };

        self.logger.repo_call(trace_id, "ProjectService", "BuildRepo", "create");
        let repo_timer = Timer::start();
        let build = self.build_repo.create(create_build).await?;
        self.logger.repo_done(trace_id, "ProjectService", "BuildRepo", "create", repo_timer.elapsed_ms());

        info!(
            "[{}] Build #{} created for project {}",
            trace_id, build.build_number, project.name
        );

        self.logger.service_exit(trace_id, "API", "ProjectService", "trigger_build", timer.elapsed_ms());
        Ok(build)
    }

    /// 프로젝트 삭제 (컨테이너, 파일, DB 정리)
    pub async fn delete_project(&self, trace_id: &str, id: i64) -> Result<()> {
        let timer = Timer::start();
        self.logger.service_entry(trace_id, "API", "ProjectService", "delete_project", &id);

        // Get project first
        self.logger.repo_call(trace_id, "ProjectService", "ProjectRepo", "get");
        let repo_timer = Timer::start();
        let project = self.project_repo.get(id).await?
            .context("Project not found")?;
        self.logger.repo_done(trace_id, "ProjectService", "ProjectRepo", "get", repo_timer.elapsed_ms());

        // Stop and remove containers
        for slot in [Slot::Blue, Slot::Green] {
            let container_id = match slot {
                Slot::Blue => &project.blue_container_id,
                Slot::Green => &project.green_container_id,
            };

            // Try to stop by container ID first
            if let Some(cid) = container_id {
                info!("[{}] Stopping container {} for project {}", trace_id, cid, project.name);

                self.logger.external_call(trace_id, "ProjectService", "Docker", "stop_container");
                self.docker.stop_container(cid).await.ok();
            }

            // Also try to stop by container name
            let container_name = match slot {
                Slot::Blue => format!("project-{}-blue", project.id),
                Slot::Green => format!("project-{}-green", project.id),
            };

            info!("[{}] Attempting to stop container by name: {}", trace_id, container_name);
            self.logger.external_call(trace_id, "ProjectService", "Docker", "stop_container");
            self.docker.stop_container(&container_name).await.ok();
        }

        // Remove directories
        let workspace_path = PathBuf::from("/data/workspace").join(&project.name);
        let output_base = PathBuf::from("/data/output");
        let cache_path = PathBuf::from("/data/cache").join(&project.cache_type);
        let logs_path = PathBuf::from("/data/easycicd/logs").join(&project.name);

        for path in [workspace_path, cache_path, logs_path] {
            if path.exists() {
                info!("[{}] Removing directory: {:?}", trace_id, path);
                if let Err(e) = fs::remove_dir_all(&path).await {
                    warn!("[{}] Failed to remove directory {:?}: {}", trace_id, path, e);
                }
            }
        }

        // Remove only build output directories for this specific project
        if let Ok(builds) = self.build_repo.list_by_project(id, 10000).await {
            for build in builds {
                let build_output_path = output_base.join(format!("build{}", build.id));
                if build_output_path.exists() {
                    info!("[{}] Removing build output: {:?}", trace_id, build_output_path);
                    if let Err(e) = fs::remove_dir_all(&build_output_path).await {
                        warn!("[{}] Failed to remove build output {:?}: {}", trace_id, build_output_path, e);
                    }
                }
            }
        }

        // Delete from database
        self.logger.repo_call(trace_id, "ProjectService", "ProjectRepo", "delete");
        let repo_timer = Timer::start();
        self.project_repo.delete(id).await?;
        self.logger.repo_done(trace_id, "ProjectService", "ProjectRepo", "delete", repo_timer.elapsed_ms());

        info!("[{}] Project {} deleted successfully", trace_id, project.name);

        self.logger.service_exit(trace_id, "API", "ProjectService", "delete_project", timer.elapsed_ms());
        Ok(())
    }

    /// 컨테이너 시작
    pub async fn start_containers(&self, trace_id: &str, project_id: i64) -> Result<Vec<ContainerOperationResult>> {
        let timer = Timer::start();
        self.logger.service_entry(trace_id, "API", "ProjectService", "start_containers", &project_id);

        // Get project
        self.logger.repo_call(trace_id, "ProjectService", "ProjectRepo", "get");
        let repo_timer = Timer::start();
        let project = self.project_repo.get(project_id).await?
            .context("Project not found")?;
        self.logger.repo_done(trace_id, "ProjectService", "ProjectRepo", "get", repo_timer.elapsed_ms());

        let mut results = Vec::new();

        // Start both containers
        for slot in [Slot::Blue, Slot::Green] {
            let container_name = match slot {
                Slot::Blue => format!("project-{}-blue", project.id),
                Slot::Green => format!("project-{}-green", project.id),
            };

            self.logger.external_call(trace_id, "ProjectService", "Docker", "start_container");
            let docker_timer = Timer::start();

            match self.docker.start_container(&container_name).await {
                Ok(_) => {
                    self.logger.external_done(trace_id, "ProjectService", "Docker", "start_container", docker_timer.elapsed_ms());
                    info!("[{}] Started container: {}", trace_id, container_name);
                    results.push(ContainerOperationResult {
                        slot: slot.to_string(),
                        status: "started".to_string(),
                        container: container_name,
                        error: None,
                    });
                }
                Err(e) => {
                    self.logger.external_error(trace_id, "ProjectService", "Docker", "start_container", &e);
                    warn!("[{}] Failed to start container {}: {}", trace_id, container_name, e);
                    results.push(ContainerOperationResult {
                        slot: slot.to_string(),
                        status: "error".to_string(),
                        container: container_name,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        self.logger.service_exit(trace_id, "API", "ProjectService", "start_containers", timer.elapsed_ms());
        Ok(results)
    }

    /// 컨테이너 중지
    pub async fn stop_containers(&self, trace_id: &str, project_id: i64) -> Result<Vec<ContainerOperationResult>> {
        let timer = Timer::start();
        self.logger.service_entry(trace_id, "API", "ProjectService", "stop_containers", &project_id);

        // Get project
        self.logger.repo_call(trace_id, "ProjectService", "ProjectRepo", "get");
        let repo_timer = Timer::start();
        let project = self.project_repo.get(project_id).await?
            .context("Project not found")?;
        self.logger.repo_done(trace_id, "ProjectService", "ProjectRepo", "get", repo_timer.elapsed_ms());

        let mut results = Vec::new();

        // Stop both containers
        for slot in [Slot::Blue, Slot::Green] {
            let container_name = match slot {
                Slot::Blue => format!("project-{}-blue", project.id),
                Slot::Green => format!("project-{}-green", project.id),
            };

            self.logger.external_call(trace_id, "ProjectService", "Docker", "stop_container");
            let docker_timer = Timer::start();

            match self.docker.stop_container(&container_name).await {
                Ok(_) => {
                    self.logger.external_done(trace_id, "ProjectService", "Docker", "stop_container", docker_timer.elapsed_ms());
                    info!("[{}] Stopped container: {}", trace_id, container_name);
                    results.push(ContainerOperationResult {
                        slot: slot.to_string(),
                        status: "stopped".to_string(),
                        container: container_name,
                        error: None,
                    });
                }
                Err(e) => {
                    self.logger.external_error(trace_id, "ProjectService", "Docker", "stop_container", &e);
                    warn!("[{}] Failed to stop container {}: {}", trace_id, container_name, e);
                    results.push(ContainerOperationResult {
                        slot: slot.to_string(),
                        status: "error".to_string(),
                        container: container_name,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        self.logger.service_exit(trace_id, "API", "ProjectService", "stop_containers", timer.elapsed_ms());
        Ok(results)
    }

    /// 컨테이너 재시작
    pub async fn restart_containers(&self, trace_id: &str, project_id: i64) -> Result<Vec<ContainerOperationResult>> {
        let timer = Timer::start();
        self.logger.service_entry(trace_id, "API", "ProjectService", "restart_containers", &project_id);

        // Get project
        self.logger.repo_call(trace_id, "ProjectService", "ProjectRepo", "get");
        let repo_timer = Timer::start();
        let project = self.project_repo.get(project_id).await?
            .context("Project not found")?;
        self.logger.repo_done(trace_id, "ProjectService", "ProjectRepo", "get", repo_timer.elapsed_ms());

        let mut results = Vec::new();

        // Restart both containers
        for slot in [Slot::Blue, Slot::Green] {
            let container_name = match slot {
                Slot::Blue => format!("project-{}-blue", project.id),
                Slot::Green => format!("project-{}-green", project.id),
            };

            self.logger.external_call(trace_id, "ProjectService", "Docker", "restart_container");
            let docker_timer = Timer::start();

            match self.docker.restart_container(&container_name).await {
                Ok(_) => {
                    self.logger.external_done(trace_id, "ProjectService", "Docker", "restart_container", docker_timer.elapsed_ms());
                    info!("[{}] Restarted container: {}", trace_id, container_name);
                    results.push(ContainerOperationResult {
                        slot: slot.to_string(),
                        status: "restarted".to_string(),
                        container: container_name,
                        error: None,
                    });
                }
                Err(e) => {
                    self.logger.external_error(trace_id, "ProjectService", "Docker", "restart_container", &e);
                    warn!("[{}] Failed to restart container {}: {}", trace_id, container_name, e);
                    results.push(ContainerOperationResult {
                        slot: slot.to_string(),
                        status: "error".to_string(),
                        container: container_name,
                        error: Some(e.to_string()),
                    });
                }
            }
        }

        self.logger.service_exit(trace_id, "API", "ProjectService", "restart_containers", timer.elapsed_ms());
        Ok(results)
    }
}

/// 컨테이너 작업 결과
#[derive(Debug, Clone)]
pub struct ContainerOperationResult {
    pub slot: String,
    pub status: String,
    pub container: String,
    pub error: Option<String>,
}
