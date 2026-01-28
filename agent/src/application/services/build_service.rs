use anyhow::{Context, Result};
use serde_json;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs;
use tokio::io::AsyncWriteExt;
use tracing::{info, warn};

use crate::application::ports::repositories::{BuildRepository, ProjectRepository, SettingsRepository};
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
pub struct BuildService<BR, PR, SR, EB>
where
    BR: BuildRepository,
    PR: ProjectRepository,
    SR: SettingsRepository,
    EB: EventBus,
{
    build_repo: Arc<BR>,
    project_repo: Arc<PR>,
    settings_repo: Arc<SR>,
    event_bus: EB,
    docker: DockerClient,
    logger: Arc<BoundaryLogger>,
}

impl<BR, PR, SR, EB> BuildService<BR, PR, SR, EB>
where
    BR: BuildRepository,
    PR: ProjectRepository,
    SR: SettingsRepository,
    EB: EventBus,
{
    pub fn new(
        build_repo: Arc<BR>,
        project_repo: Arc<PR>,
        settings_repo: Arc<SR>,
        event_bus: EB,
        docker: DockerClient,
        logger: Arc<BoundaryLogger>,
    ) -> Self {
        Self {
            build_repo,
            project_repo,
            settings_repo,
            event_bus,
            docker,
            logger,
        }
    }

    /// 빌드 실행
    pub async fn execute_build(&self, trace_id: &str, build_id: i64) -> Result<PathBuf> {
        let timer = Timer::start();
        let build_start_time = std::time::SystemTime::now();  // 빌드 시작 시간 기록 (산출물 검증용)
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

        // Setup paths (workspace removed - git clone happens inside container)
        let output_path = PathBuf::from("/data/output").join(format!("build{}", build.id));
        let cache_path = PathBuf::from("/data/cache").join(&project.cache_type);
        let log_path = PathBuf::from(&build.log_path);

        // Clean output directory before build (prevent stale artifacts from previous builds)
        if output_path.exists() {
            info!("[{}] Cleaning existing output directory: {}", trace_id, output_path.display());
            fs::remove_dir_all(&output_path).await
                .context("Failed to clean output directory")?;
        }

        // Create fresh directories
        fs::create_dir_all(&output_path).await.context("Failed to create output directory")?;
        fs::create_dir_all(&cache_path).await.context("Failed to create cache directory")?;
        fs::create_dir_all(log_path.parent().unwrap()).await.context("Failed to create log directory")?;

        // Get GitHub PAT for git authentication inside container
        let github_token = self.settings_repo.get("github_pat").await.ok().flatten();

        // Build authenticated repo URL
        let authenticated_repo = if let Some(token) = &github_token {
            if project.repo.starts_with("https://github.com/") {
                project.repo.replace("https://github.com/", &format!("https://{}@github.com/", token))
            } else {
                project.repo.clone()
            }
        } else {
            project.repo.clone()
        };

        // Construct full build command with git clone inside container
        let working_dir_path = if let Some(wd) = &project.working_directory {
            format!("/{}", wd)
        } else {
            String::new()
        };

        // Add environment variables for build
        // - CI=true: Treat warnings as errors (standard CI behavior)
        // - SKIP_PREFLIGHT_CHECK: Skip CRA version check (avoids false positives)
        let mut env_vars_list = vec!["CI=true".to_string(), "SKIP_PREFLIGHT_CHECK=true".to_string()];

        // Parse and add user-defined build environment variables (JSON format)
        if let Some(build_env_json) = &project.build_env_vars {
            if let Ok(parsed) = serde_json::from_str::<serde_json::Map<String, serde_json::Value>>(build_env_json) {
                for (key, value) in parsed {
                    let val_str = match value {
                        serde_json::Value::String(s) => s,
                        other => other.to_string().trim_matches('"').to_string(),
                    };
                    env_vars_list.push(format!("{}={}", key, val_str));
                }
            }
        }

        // 환경변수를 export 형태로 변환하여 전체 빌드 명령어에 적용되도록 함
        let env_exports = env_vars_list.iter()
            .map(|v| format!("export {}", v))
            .collect::<Vec<_>>()
            .join(" && ");

        // build_image 기반으로 프로젝트 타입 감지 (cache_type과 독립적으로 동작)
        let build_image_lower = project.build_image.to_lowercase();
        let build_cmd_lower = project.build_command.to_lowercase();

        let is_node_project = build_image_lower.contains("node")
            || build_cmd_lower.contains("npm")
            || build_cmd_lower.contains("yarn")
            || build_cmd_lower.contains("pnpm");
        let is_gradle_project = build_image_lower.contains("gradle")
            || build_cmd_lower.contains("gradle");
        let is_maven_project = build_image_lower.contains("maven")
            || build_cmd_lower.contains("mvn");

        // 프로젝트 타입에 따라 출력물 복사 명령어 자동 생성
        // build_command에 이미 /output/ 복사가 포함되어 있으면 중복 방지를 위해 스킵
        // 중요: fallback으로 전체 소스 복사하는 패턴 제거 - 빌드 실패 시 명확하게 에러 반환
        let output_copy_command = if project.build_command.contains("/output/") {
            // 이미 복사 명령이 있음 (detector.rs에서 자동 생성된 경우)
            ""
        } else if is_gradle_project {
            // Gradle: JAR 파일 복사 - 없으면 에러 (fallback 제거)
            "JAR_FILE=$(find build/libs -name '*.jar' ! -name '*-plain.jar' | head -1) && \
             if [ -n \"$JAR_FILE\" ]; then cp \"$JAR_FILE\" /output/app.jar; \
             else echo 'ERROR: No JAR file found in build/libs' >&2 && exit 1; fi"
        } else if is_maven_project {
            // Maven: JAR 파일 복사 - 없으면 에러 (fallback 제거)
            "JAR_FILE=$(find target -name '*.jar' ! -name '*-sources.jar' ! -name '*-javadoc.jar' | head -1) && \
             if [ -n \"$JAR_FILE\" ]; then cp \"$JAR_FILE\" /output/app.jar; \
             else echo 'ERROR: No JAR file found in target' >&2 && exit 1; fi"
        } else if is_node_project {
            // Node.js: 빌드 산출물 복사 - 산출물 디렉토리가 없으면 에러 (fallback 제거)
            // Next.js는 .next/ 디렉토리 사용
            "if [ -d dist ]; then cp -r dist/. /output/; \
             elif [ -d build ]; then cp -r build/. /output/; \
             elif [ -d out ]; then cp -r out/. /output/; \
             elif [ -d .next ]; then cp -r .next/. /output/; \
             else echo 'ERROR: No build output directory found (dist/, build/, out/, or .next/)' >&2 && exit 1; fi"
        } else {
            // 기타 (Python 등): 전체 복사 유지
            "cp -r . /output/"
        };

        // output_copy_command가 비어있으면 추가하지 않음 (이중 복사 방지)
        let full_build_command = if output_copy_command.is_empty() {
            format!(
                "git clone --depth 1 --branch {} {} /workspace && cd /workspace{} && {} && {}",
                project.branch,
                authenticated_repo,
                working_dir_path,
                env_exports,
                project.build_command
            )
        } else {
            format!(
                "git clone --depth 1 --branch {} {} /workspace && cd /workspace{} && {} && {} && {}",
                project.branch,
                authenticated_repo,
                working_dir_path,
                env_exports,
                project.build_command,
                output_copy_command
            )
        };

        info!("[{}] Build command: git clone + {}", trace_id, project.build_command);

        // Open log file
        let mut log_file = fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .await
            .context("Failed to open log file")?;

        // Create nginx config if needed
        if project.runtime_image.contains("nginx") {
            self.create_nginx_config(&output_path, project.runtime_port as u16).await?;
        }

        // Run build container (with git clone command included)
        self.logger.external_call(trace_id, "BuildService", "Docker", "run_build_container");
        let docker_timer = Timer::start();

        let build_result = self.docker.run_build_container(
            &project.build_image,
            &full_build_command,
            output_path.clone(),
            cache_path,
            &project.cache_type,
        ).await?;

        self.logger.external_done(trace_id, "BuildService", "Docker", "run_build_container", docker_timer.elapsed_ms());

        // Write logs and emit events (always, regardless of success/failure)
        for (idx, line) in build_result.logs.iter().enumerate() {
            log_file.write_all(line.as_bytes()).await.context("Failed to write log")?;
            log_file.write_all(b"\n").await.ok();

            // Emit log event immediately for real-time streaming
            self.event_bus.emit(Event::Log {
                build_id: build.id,
                line: line.clone(),
                line_number: idx,
                timestamp: Event::now(),
            }).await;
        }

        if let Err(e) = log_file.flush().await {
            warn!("[{}] Failed to flush log file: {}", trace_id, e);
        }

        if build_result.success {
            // Validate build output exists (with timestamp check for stale artifacts)
            let validation_result = self.validate_build_output(&output_path, &project, build_start_time).await;
            if let Err(e) = validation_result {
                warn!("[{}] Build output validation failed: {}", trace_id, e);

                // Update status to Failed
                self.build_repo.update_status(build.id, BuildStatus::Failed).await?;
                self.event_bus.emit(Event::BuildStatus {
                    build_id: build.id,
                    project_id: project.id,
                    status: BuildStatus::Failed,
                    timestamp: Event::now(),
                }).await;

                let error_msg = format!("Build output validation failed: {}", e);
                self.event_bus.emit(Event::Error {
                    project_id: Some(project.id),
                    build_id: Some(build.id),
                    message: error_msg.clone(),
                    timestamp: Event::now(),
                }).await;

                anyhow::bail!("{}", error_msg);
            }

            info!("[{}] Build #{} completed successfully", trace_id, build.build_number);
            self.logger.service_exit(trace_id, "API", "BuildService", "execute_build", timer.elapsed_ms());
            Ok(output_path)
        } else {
            warn!("[{}] Build #{} failed with exit code: {}", trace_id, build.build_number, build_result.exit_code);

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

            let error_msg = format!("Build failed with exit code: {}", build_result.exit_code);
            self.event_bus.emit(Event::Error {
                project_id: Some(project.id),
                build_id: Some(build.id),
                message: error_msg.clone(),
                timestamp: Event::now(),
            }).await;

            self.logger.service_error(trace_id, "API", "BuildService", "execute_build", &anyhow::anyhow!("{}", error_msg));
            anyhow::bail!("{}", error_msg);
        }
    }

    /// 빌드 산출물 검증 (강화된 버전)
    /// - 파일 존재 여부
    /// - 빌드 이후 수정된 파일 존재 여부 (stale artifact 방지)
    /// - 최소 파일 크기 검증
    /// - 프로젝트 타입별 필수 파일 검증
    async fn validate_build_output(
        &self,
        output_path: &PathBuf,
        project: &Project,
        build_start_time: std::time::SystemTime,
    ) -> Result<()> {
        // Check if output directory exists
        if !output_path.exists() {
            anyhow::bail!("Output directory does not exist: {}", output_path.display());
        }

        let mut entries = fs::read_dir(output_path).await
            .context("Failed to read output directory")?;

        let mut file_count = 0;
        let mut total_size: u64 = 0;
        let mut has_index_html = false;
        let mut has_jar_file = false;
        let mut has_recent_files = false;
        let mut has_any_content = false;

        while let Some(entry) = entries.next_entry().await? {
            file_count += 1;
            has_any_content = true;

            let file_name = entry.file_name();
            let name = file_name.to_string_lossy();

            // Get file metadata
            if let Ok(metadata) = entry.metadata().await {
                total_size += metadata.len();

                // Check if file was modified after build started
                if let Ok(modified) = metadata.modified() {
                    if modified >= build_start_time {
                        has_recent_files = true;
                    }
                }
            }

            // Check for specific artifact types
            if name == "index.html" {
                has_index_html = true;
            }
            if name.ends_with(".jar") {
                has_jar_file = true;
            }
            // nginx.conf는 우리가 생성한 것이므로 recent file로 카운트
            if name == "nginx.conf" {
                has_recent_files = true;
            }
        }

        // Validation 1: Must have content
        if !has_any_content {
            anyhow::bail!("Output directory is empty - build produced no files");
        }

        // Validation 2: Must have recently modified files (prevent stale artifacts)
        if !has_recent_files {
            anyhow::bail!(
                "No files were modified during this build. Output may contain stale artifacts from a previous build."
            );
        }

        // Validation 3: Minimum size check (prevent empty/stub files)
        let build_image_lower = project.build_image.to_lowercase();
        let runtime_image_lower = project.runtime_image.to_lowercase();

        let min_size: u64 = if runtime_image_lower.contains("nginx") {
            100  // HTML should be at least 100 bytes
        } else if runtime_image_lower.contains("jre") || runtime_image_lower.contains("java") || runtime_image_lower.contains("openjdk") {
            1000  // JAR should be at least 1KB
        } else {
            50   // Minimum for any artifact
        };

        if total_size < min_size {
            anyhow::bail!(
                "Build output too small ({} bytes). Expected at least {} bytes for this project type.",
                total_size, min_size
            );
        }

        // Validation 4: Project type specific checks

        // For nginx (static web apps), require index.html
        if runtime_image_lower.contains("nginx") {
            if !has_index_html {
                // Check in common subdirectories
                let common_dirs = ["build", "dist", "out", "public", ".next"];
                let mut found_index = false;

                for dir in common_dirs {
                    let sub_path = output_path.join(dir).join("index.html");
                    if sub_path.exists() {
                        found_index = true;
                        info!("Found index.html in {}/{}/", output_path.display(), dir);
                        break;
                    }
                }

                if !found_index {
                    anyhow::bail!(
                        "Static web app build missing index.html. Found {} files ({} bytes) but no index.html",
                        file_count, total_size
                    );
                }
            }
        }

        // For JVM projects, require JAR file
        if (build_image_lower.contains("gradle") || build_image_lower.contains("maven"))
            && (runtime_image_lower.contains("jre") || runtime_image_lower.contains("java") || runtime_image_lower.contains("openjdk"))
        {
            let jar_path = output_path.join("app.jar");
            if !jar_path.exists() && !has_jar_file {
                anyhow::bail!(
                    "Java project build missing JAR file. Expected app.jar in output directory."
                );
            }

            // Verify JAR is not corrupted (check for ZIP magic bytes)
            if jar_path.exists() {
                let jar_content = fs::read(&jar_path).await?;
                if jar_content.len() < 4 || &jar_content[0..4] != b"PK\x03\x04" {
                    anyhow::bail!("app.jar appears to be corrupted (invalid ZIP header)");
                }
            }
        }

        info!(
            "Build output validation passed: {} files, {} bytes total",
            file_count, total_size
        );
        Ok(())
    }

    /// Nginx 설정 파일 생성
    async fn create_nginx_config(&self, output_path: &PathBuf, runtime_port: u16) -> Result<()> {
        let nginx_conf = format!(r#"
daemon off;
worker_processes 1;
error_log /dev/stdout info;

events {{
    worker_connections 1024;
}}

http {{
    include /etc/nginx/mime.types;
    default_type application/octet-stream;
    access_log /dev/stdout;

    server {{
        listen {};
        root /app;
        index index.html;

        location / {{
            try_files $uri $uri/ /index.html;
        }}
    }}
}}
"#, runtime_port);

        let config_path = output_path.join("nginx.conf");
        fs::write(&config_path, nginx_conf).await.context("Failed to write nginx.conf")?;

        info!("Created nginx.conf at {} with listen port {}", config_path.display(), runtime_port);
        Ok(())
    }
}
