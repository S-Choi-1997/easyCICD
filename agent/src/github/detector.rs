use serde::{Deserialize, Serialize};
use super::GitHubClient;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project_type: String,
    pub build_image: String,
    pub build_command: String,
    pub cache_type: String,
    pub runtime_image: String,
    pub runtime_command: String,
    pub health_check_url: String,
}

#[derive(Debug, Clone)]
pub struct ProjectDetector {
    client: GitHubClient,
}

impl ProjectDetector {
    pub fn new(client: GitHubClient) -> Self {
        Self { client }
    }

    /// Detect project type and generate configuration
    pub async fn detect(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
        path_filter: Option<&str>,
    ) -> Result<ProjectConfig, String> {
        // Get commit SHA for the branch
        let branches = self.client.list_branches(owner, repo).await
            .map_err(|e| format!("Failed to fetch branches: {}", e))?;

        let branch_info = branches.iter()
            .find(|b| b.name == branch)
            .ok_or_else(|| format!("Branch '{}' not found", branch))?;

        let sha = &branch_info.commit.sha;

        // Get repository tree
        let tree = self.client.get_tree(owner, repo, sha).await
            .map_err(|e| format!("Failed to fetch tree: {}", e))?;

        // Filter by path if specified
        let files: Vec<String> = tree.tree.iter()
            .map(|item| item.path.clone())
            .filter(|path| {
                if let Some(prefix) = path_filter {
                    path.starts_with(prefix)
                } else {
                    true
                }
            })
            .collect();

        // Check for Dockerfile first (highest priority)
        if files.iter().any(|f| f.ends_with("Dockerfile") || f.ends_with("dockerfile")) {
            return Ok(ProjectConfig {
                project_type: "Dockerfile".to_string(),
                build_image: "docker:24-cli".to_string(),
                build_command: "docker build -t app .".to_string(),
                cache_type: "none".to_string(),
                runtime_image: "app".to_string(),
                runtime_command: "".to_string(),
                health_check_url: "/".to_string(),
            });
        }

        // Detect by project files
        if files.iter().any(|f| f.ends_with("package.json")) {
            return self.detect_nodejs(&files).await;
        }

        if files.iter().any(|f| f.ends_with("build.gradle") || f.ends_with("build.gradle.kts")) {
            return self.detect_gradle(&files).await;
        }

        if files.iter().any(|f| f.ends_with("pom.xml")) {
            return self.detect_maven(&files).await;
        }

        if files.iter().any(|f| f.ends_with("Cargo.toml")) {
            return self.detect_rust(&files).await;
        }

        if files.iter().any(|f| f.ends_with("go.mod")) {
            return self.detect_go(&files).await;
        }

        if files.iter().any(|f| f.ends_with("requirements.txt") || f.ends_with("pyproject.toml")) {
            return self.detect_python(&files).await;
        }

        Err("Unable to detect project type. Please configure manually.".to_string())
    }

    async fn detect_nodejs(&self, files: &[String]) -> Result<ProjectConfig, String> {
        // Check if it's a frontend or backend project
        let has_src = files.iter().any(|f| f.contains("/src/"));
        let has_public = files.iter().any(|f| f.contains("/public/"));
        let has_index_html = files.iter().any(|f| f.ends_with("index.html"));

        if has_index_html || has_public {
            // Frontend project (React, Vue, Svelte, etc.)
            Ok(ProjectConfig {
                project_type: "Node.js (Frontend)".to_string(),
                build_image: "node:20".to_string(),
                build_command: "npm install && npm run build".to_string(),
                cache_type: "npm".to_string(),
                runtime_image: "nginx:alpine".to_string(),
                runtime_command: "nginx -g 'daemon off;'".to_string(),
                health_check_url: "/".to_string(),
            })
        } else {
            // Backend project (Express, NestJS, etc.)
            Ok(ProjectConfig {
                project_type: "Node.js (Backend)".to_string(),
                build_image: "node:20".to_string(),
                build_command: "npm install && npm run build".to_string(),
                cache_type: "npm".to_string(),
                runtime_image: "node:20-slim".to_string(),
                runtime_command: "node dist/index.js".to_string(),
                health_check_url: "/health".to_string(),
            })
        }
    }

    async fn detect_gradle(&self, _files: &[String]) -> Result<ProjectConfig, String> {
        Ok(ProjectConfig {
            project_type: "Gradle (Spring Boot)".to_string(),
            build_image: "gradle:8-jdk17".to_string(),
            build_command: "gradle bootJar".to_string(),
            cache_type: "gradle".to_string(),
            runtime_image: "openjdk:17-jre-slim".to_string(),
            runtime_command: "java -jar build/libs/*.jar".to_string(),
            health_check_url: "/actuator/health".to_string(),
        })
    }

    async fn detect_maven(&self, _files: &[String]) -> Result<ProjectConfig, String> {
        Ok(ProjectConfig {
            project_type: "Maven (Spring Boot)".to_string(),
            build_image: "maven:3.9-eclipse-temurin-17".to_string(),
            build_command: "mvn clean package -DskipTests".to_string(),
            cache_type: "maven".to_string(),
            runtime_image: "openjdk:17-jre-slim".to_string(),
            runtime_command: "java -jar target/*.jar".to_string(),
            health_check_url: "/actuator/health".to_string(),
        })
    }

    async fn detect_rust(&self, _files: &[String]) -> Result<ProjectConfig, String> {
        Ok(ProjectConfig {
            project_type: "Rust".to_string(),
            build_image: "rust:1.75".to_string(),
            build_command: "cargo build --release".to_string(),
            cache_type: "rust".to_string(),
            runtime_image: "debian:bookworm-slim".to_string(),
            runtime_command: "./target/release/app".to_string(),
            health_check_url: "/health".to_string(),
        })
    }

    async fn detect_go(&self, _files: &[String]) -> Result<ProjectConfig, String> {
        Ok(ProjectConfig {
            project_type: "Go".to_string(),
            build_image: "golang:1.21".to_string(),
            build_command: "go build -o app .".to_string(),
            cache_type: "go".to_string(),
            runtime_image: "debian:bookworm-slim".to_string(),
            runtime_command: "./app".to_string(),
            health_check_url: "/health".to_string(),
        })
    }

    async fn detect_python(&self, files: &[String]) -> Result<ProjectConfig, String> {
        let has_flask = files.iter().any(|f| f.contains("flask"));
        let has_django = files.iter().any(|f| f.contains("django"));
        let has_fastapi = files.iter().any(|f| f.contains("fastapi"));

        let runtime_command = if has_django {
            "python manage.py runserver 0.0.0.0:8000".to_string()
        } else if has_fastapi {
            "uvicorn main:app --host 0.0.0.0 --port 8000".to_string()
        } else if has_flask {
            "python app.py".to_string()
        } else {
            "python main.py".to_string()
        };

        Ok(ProjectConfig {
            project_type: "Python".to_string(),
            build_image: "python:3.11".to_string(),
            build_command: "pip install -r requirements.txt".to_string(),
            cache_type: "pip".to_string(),
            runtime_image: "python:3.11-slim".to_string(),
            runtime_command,
            health_check_url: "/health".to_string(),
        })
    }
}
