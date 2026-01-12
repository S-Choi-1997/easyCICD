use super::workflow_interpreter::{ExecutionPlan, ProjectType, TaskType};
use super::detector::ProjectConfig;

// =============================================================================
// ConfigBuilder
// =============================================================================

/// ExecutionPlan을 ProjectConfig로 변환합니다.
///
/// # 역할
/// - ExecutionPlan의 정보를 빌드 시스템이 이해하는 형식으로 변환
/// - Docker 이미지, 빌드 커맨드, 런타임 커맨드 생성
///
/// # 규칙
/// - ExecutionPlan에 있는 정보만 사용
/// - 가정이나 임의 판단 최소화
pub struct ConfigBuilder;

impl ConfigBuilder {
    /// ExecutionPlan을 ProjectConfig로 변환
    pub fn build(plan: &ExecutionPlan) -> Result<ProjectConfig, String> {
        let project_type_str = Self::format_project_type(&plan.project_type);

        let (build_image, runtime_image) = Self::determine_images(plan)?;

        let build_command = Self::generate_build_command(plan)?;

        let runtime_command = Self::generate_runtime_command(plan)?;

        let health_check_url = Self::determine_health_check(&plan.project_type);

        Ok(ProjectConfig {
            project_type: project_type_str,
            build_image,
            build_command,
            cache_type: Self::determine_cache_type(&plan.runtime.language),
            runtime_image,
            runtime_command,
            health_check_url,
            working_directory: None, // detector가 별도로 설정
            runtime_port: plan.detected_port.unwrap_or_else(|| Self::determine_default_port(&plan.project_type)),
        })
    }

    // =========================================================================
    // Private: 이미지 결정
    // =========================================================================

    fn determine_images(plan: &ExecutionPlan) -> Result<(String, String), String> {
        match plan.runtime.language.as_str() {
            "node" => {
                let version = &plan.runtime.version;
                let build_image = format!("node:{}", version);
                let runtime_image = match plan.project_type {
                    ProjectType::NodeJsFrontend => "nginx:alpine".to_string(),
                    ProjectType::NodeJsBackend => format!("node:{}-slim", version),
                    _ => format!("node:{}-slim", version),
                };
                Ok((build_image, runtime_image))
            }
            "java" => {
                let version = &plan.runtime.version;
                Ok((
                    format!("gradle:8-jdk{}", version),
                    format!("eclipse-temurin:{}-jre", version),
                ))
            }
            "python" => {
                let version = &plan.runtime.version;
                Ok((
                    format!("python:{}", version),
                    format!("python:{}-slim", version),
                ))
            }
            "go" => {
                let version = &plan.runtime.version;
                Ok((
                    format!("golang:{}", version),
                    "gcr.io/distroless/base-debian11".to_string(),
                ))
            }
            lang => Err(format!("지원하지 않는 언어: {}", lang)),
        }
    }

    // =========================================================================
    // Private: 빌드 커맨드 생성
    // =========================================================================

    fn generate_build_command(plan: &ExecutionPlan) -> Result<String, String> {
        let mut commands = Vec::new();

        // 1. 의존성 설치
        for task in &plan.tasks {
            if task.task_type == TaskType::InstallDependencies {
                commands.push(task.command.clone());
            }
        }

        // 2. 빌드
        let mut has_build_command = false;
        if plan.project_type != ProjectType::NodeJsBackend {
            for task in &plan.tasks {
                if task.task_type == TaskType::Build {
                    commands.push(task.command.clone());
                    has_build_command = true;
                }
            }
        }

        // 2-1. JavaSpringBoot의 경우 빌드 명령어가 없으면 기본 gradle 빌드 추가
        if plan.project_type == ProjectType::JavaSpringBoot && !has_build_command {
            commands.push("gradle clean build".to_string());
        }

        // 3. 출력 복사 (프로젝트 타입에 따라)
        commands.push(Self::generate_copy_command(plan));

        if commands.is_empty() {
            return Err("워크플로우에서 빌드 커맨드를 찾을 수 없습니다".to_string());
        }

        Ok(commands.join(" && "))
    }

    fn generate_copy_command(plan: &ExecutionPlan) -> String {
        match plan.project_type {
            ProjectType::NodeJsBackend => {
                // Backend: 소스 코드, 의존성, package.json 복사
                "cp -r src node_modules package*.json /output/".to_string()
            }
            ProjectType::NodeJsFrontend => {
                // Frontend: 빌드 결과물 복사
                "cp -r dist/* /output/ 2>/dev/null || cp -r build/* /output/".to_string()
            }
            ProjectType::JavaSpringBoot => {
                // JAR 파일 찾아서 복사
                "find build/libs target -name '*.jar' ! -name '*-plain.jar' -exec cp {} /output/app.jar \\;".to_string()
            }
            ProjectType::PythonDjango => {
                // Python 소스 전체 복사
                "cp -r . /output/".to_string()
            }
            ProjectType::GolangApi => {
                // 컴파일된 바이너리 복사
                "cp main /output/ 2>/dev/null || cp app /output/".to_string()
            }
            ProjectType::RustCargo => {
                // Rust 바이너리 복사
                "cp target/release/* /output/ 2>/dev/null || true".to_string()
            }
            ProjectType::Unknown => {
                // 기본: dist 또는 build 폴더
                "cp -r dist/* /output/ 2>/dev/null || cp -r build/* /output/ 2>/dev/null || cp -r . /output/".to_string()
            }
        }
    }

    // =========================================================================
    // Private: 런타임 커맨드 생성
    // =========================================================================

    fn generate_runtime_command(plan: &ExecutionPlan) -> Result<String, String> {
        match plan.project_type {
            ProjectType::NodeJsFrontend => {
                Ok("nginx -c /app/nginx.conf".to_string())
            }
            ProjectType::NodeJsBackend => {
                // 워크플로우에서 "node xxx.js" 커맨드 찾기
                let node_cmd = plan.tasks.iter()
                    .find(|t| t.command.contains("node ") && t.command.contains(".js"))
                    .and_then(|t| Self::extract_node_command(&t.command));

                Ok(node_cmd.unwrap_or_else(|| "node src/index.js".to_string()))
            }
            ProjectType::JavaSpringBoot => {
                Ok("java -jar /app/app.jar".to_string())
            }
            ProjectType::PythonDjango => {
                Ok("python manage.py runserver 0.0.0.0:8000".to_string())
            }
            ProjectType::GolangApi => {
                Ok("./main".to_string())
            }
            ProjectType::RustCargo => {
                Ok("./main".to_string())
            }
            ProjectType::Unknown => {
                Err("프로젝트 타입을 알 수 없어 런타임 커맨드를 생성할 수 없습니다".to_string())
            }
        }
    }

    /// "node src/index.js &" 같은 커맨드에서 "node src/index.js" 추출
    fn extract_node_command(command: &str) -> Option<String> {
        command
            .lines()
            .map(|line| line.trim())
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .find(|line| line.starts_with("node "))
            .map(|line| {
                // "node src/index.js &" -> "node src/index.js"
                line.split_whitespace()
                    .take_while(|part| !part.starts_with('#') && *part != "&" && *part != ";")
                    .collect::<Vec<_>>()
                    .join(" ")
            })
    }

    // =========================================================================
    // Private: 기타 헬퍼
    // =========================================================================

    fn format_project_type(project_type: &ProjectType) -> String {
        match project_type {
            ProjectType::NodeJsBackend => "Node.js (Backend)".to_string(),
            ProjectType::NodeJsFrontend => "Node.js (Frontend)".to_string(),
            ProjectType::JavaSpringBoot => "Java (Spring Boot)".to_string(),
            ProjectType::PythonDjango => "Python (Django)".to_string(),
            ProjectType::GolangApi => "Go (API)".to_string(),
            ProjectType::RustCargo => "Rust (Cargo)".to_string(),
            ProjectType::Unknown => "Unknown".to_string(),
        }
    }

    fn determine_cache_type(language: &str) -> String {
        match language {
            "node" => "npm".to_string(),
            "java" => "gradle".to_string(),
            "python" => "pip".to_string(),
            "go" => "go".to_string(),
            _ => "none".to_string(),
        }
    }

    fn determine_health_check(project_type: &ProjectType) -> String {
        match project_type {
            ProjectType::NodeJsBackend => "/health".to_string(),
            ProjectType::JavaSpringBoot => "/actuator/health".to_string(),
            ProjectType::PythonDjango => "/health/".to_string(),
            _ => "/".to_string(),
        }
    }

    fn determine_default_port(project_type: &ProjectType) -> u16 {
        match project_type {
            ProjectType::NodeJsBackend => 3000,       // Express, Fastify 등 기본 포트
            ProjectType::NodeJsFrontend => 80,        // nginx 기본 포트
            ProjectType::JavaSpringBoot => 8080,      // Spring Boot 기본 포트
            ProjectType::PythonDjango => 8000,        // Django 기본 포트
            ProjectType::GolangApi => 8080,           // Go 일반적인 포트
            ProjectType::RustCargo => 8080,           // Rust 일반적인 포트
            ProjectType::Unknown => 8080,             // 기본값
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::workflow_interpreter::{Runtime, Task};
    use std::collections::HashMap;

    #[test]
    fn test_build_nodejs_backend_config() {
        let plan = ExecutionPlan {
            project_type: ProjectType::NodeJsBackend,
            runtime: Runtime {
                language: "node".to_string(),
                version: "20".to_string(),
                env: HashMap::new(),
            },
            tasks: vec![
                Task {
                    name: "Install".to_string(),
                    task_type: TaskType::InstallDependencies,
                    command: "npm ci".to_string(),
                },
                Task {
                    name: "Start".to_string(),
                    task_type: TaskType::Other,
                    command: "node src/index.js &".to_string(),
                },
            ],
            detected_port: None,
        };

        let config = ConfigBuilder::build(&plan).unwrap();

        assert_eq!(config.project_type, "Node.js (Backend)");
        assert_eq!(config.build_image, "node:20");
        assert_eq!(config.runtime_image, "node:20-slim");
        assert!(config.build_command.contains("npm ci"));
        assert!(config.build_command.contains("cp -r src"));
        assert_eq!(config.runtime_command, "node src/index.js");
        assert_eq!(config.runtime_port, 3000); // Default Node.js Backend port
    }
}
