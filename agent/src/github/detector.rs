use serde::{Deserialize, Serialize};
use super::GitHubClient;
use super::workflow_parser::WorkflowParser;
use super::config_builder::ConfigBuilder;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub project_type: String,
    pub build_image: String,
    pub build_command: String,
    pub cache_type: String,
    pub runtime_image: String,
    pub runtime_command: String,
    pub health_check_url: String,
    pub working_directory: Option<String>,
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
        workflow_path: Option<&str>,
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

        // Extract working_directory from path_filter
        // If path_filter is "project1/" or "project1", working_directory should be "project1"
        let working_directory = path_filter
            .map(|p| p.trim_end_matches('/').to_string())
            .filter(|s| !s.is_empty());

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

        // Check for GitHub Actions workflow first (highest priority - most accurate build info)
        let workflow_prefix = workflow_path.unwrap_or(".github/workflows/");
        let workflow_files: Vec<&String> = files.iter()
            .filter(|f| f.starts_with(workflow_prefix) && (f.ends_with(".yml") || f.ends_with(".yaml")))
            .collect();

        if !workflow_files.is_empty() {
            if let Ok(mut config) = self.detect_from_github_actions(owner, repo, branch, &workflow_files).await {
                config.working_directory = working_directory.clone();
                return Ok(config);
            }
        }

        // Check for Dockerfile (second priority)
        if files.iter().any(|f| f.ends_with("Dockerfile") || f.ends_with("dockerfile")) {
            let mut config = self.detect_from_dockerfile(owner, repo, branch, path_filter).await?;
            config.working_directory = working_directory.clone();
            return Ok(config);
        }

        // Detect by project files
        if files.iter().any(|f| f.ends_with("package.json")) {
            let mut config = self.detect_nodejs(&files).await?;
            config.working_directory = working_directory.clone();
            return Ok(config);
        }

        if files.iter().any(|f| f.ends_with("build.gradle") || f.ends_with("build.gradle.kts")) {
            let mut config = self.detect_gradle(&files).await?;
            config.working_directory = working_directory.clone();
            return Ok(config);
        }

        if files.iter().any(|f| f.ends_with("pom.xml")) {
            let mut config = self.detect_maven(&files).await?;
            config.working_directory = working_directory.clone();
            return Ok(config);
        }

        if files.iter().any(|f| f.ends_with("Cargo.toml")) {
            let mut config = self.detect_rust(&files).await?;
            config.working_directory = working_directory.clone();
            return Ok(config);
        }

        if files.iter().any(|f| f.ends_with("go.mod")) {
            let mut config = self.detect_go(&files).await?;
            config.working_directory = working_directory.clone();
            return Ok(config);
        }

        if files.iter().any(|f| f.ends_with("requirements.txt") || f.ends_with("pyproject.toml")) {
            let mut config = self.detect_python(&files).await?;
            config.working_directory = working_directory.clone();
            return Ok(config);
        }

        // Check for pre-built static sites (GitHub Pages, etc.)
        if files.iter().any(|f| f == "index.html") {
            let mut config = self.detect_static_site(&files).await?;
            config.working_directory = working_directory.clone();
            return Ok(config);
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
                build_command: "npm install && npm run build && cp -r dist/* /output/ 2>/dev/null || cp -r build/* /output/ 2>/dev/null || echo 'Build output copied'".to_string(),
                cache_type: "npm".to_string(),
                runtime_image: "nginx:alpine".to_string(),
                runtime_command: "nginx -c /app/nginx.conf".to_string(),
                health_check_url: "/".to_string(),
                working_directory: None,
            })
        } else {
            // Backend project (Express, NestJS, etc.)
            Ok(ProjectConfig {
                project_type: "Node.js (Backend)".to_string(),
                build_image: "node:20".to_string(),
                build_command: "npm install && npm run build && cp -r dist/* /output/ && cp package*.json /output/".to_string(),
                cache_type: "npm".to_string(),
                runtime_image: "node:20-slim".to_string(),
                runtime_command: "node dist/index.js".to_string(),
                health_check_url: "/health".to_string(),
                working_directory: None,
            })
        }
    }

    async fn detect_gradle(&self, _files: &[String]) -> Result<ProjectConfig, String> {
        Ok(ProjectConfig {
            project_type: "Gradle (Spring Boot)".to_string(),
            build_image: "gradle:8-jdk17".to_string(),
            build_command: "gradle clean build && find build/libs -name '*.jar' ! -name '*-plain.jar' -exec cp {} /output/app.jar \\;".to_string(),
            cache_type: "gradle".to_string(),
            runtime_image: "eclipse-temurin:17-jre".to_string(),
            runtime_command: "java -jar app.jar".to_string(),
            health_check_url: "/actuator/health".to_string(),
            working_directory: None,
        })
    }

    async fn detect_maven(&self, _files: &[String]) -> Result<ProjectConfig, String> {
        Ok(ProjectConfig {
            project_type: "Maven (Spring Boot)".to_string(),
            build_image: "maven:3.9-eclipse-temurin-17".to_string(),
            build_command: "mvn clean package -DskipTests && cp target/*.jar /output/app.jar".to_string(),
            cache_type: "maven".to_string(),
            runtime_image: "eclipse-temurin:17-jre".to_string(),
            runtime_command: "java -jar app.jar".to_string(),
            health_check_url: "/actuator/health".to_string(),
            working_directory: None,
        })
    }

    async fn detect_rust(&self, _files: &[String]) -> Result<ProjectConfig, String> {
        Ok(ProjectConfig {
            project_type: "Rust".to_string(),
            build_image: "rust:1.75".to_string(),
            build_command: "cargo build --release && find target/release -maxdepth 1 -type f -executable -exec cp {} /output/ \\;".to_string(),
            cache_type: "rust".to_string(),
            runtime_image: "debian:bookworm-slim".to_string(),
            runtime_command: "./app".to_string(),
            health_check_url: "/health".to_string(),
            working_directory: None,
        })
    }

    async fn detect_go(&self, _files: &[String]) -> Result<ProjectConfig, String> {
        Ok(ProjectConfig {
            project_type: "Go".to_string(),
            build_image: "golang:1.21".to_string(),
            build_command: "go build -o app . && cp app /output/".to_string(),
            cache_type: "go".to_string(),
            runtime_image: "debian:bookworm-slim".to_string(),
            runtime_command: "./app".to_string(),
            health_check_url: "/health".to_string(),
            working_directory: None,
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
            build_command: "pip install -r requirements.txt && cp -r . /output/".to_string(),
            cache_type: "pip".to_string(),
            runtime_image: "python:3.11-slim".to_string(),
            runtime_command,
            health_check_url: "/health".to_string(),
            working_directory: None,
        })
    }

    async fn detect_static_site(&self, _files: &[String]) -> Result<ProjectConfig, String> {
        // Pre-built static site (GitHub Pages, Netlify build output, etc.)
        Ok(ProjectConfig {
            project_type: "Static Site (pre-built)".to_string(),
            build_image: "alpine:latest".to_string(),
            build_command: "cp -r . /output/".to_string(),
            cache_type: "none".to_string(),
            runtime_image: "nginx:alpine".to_string(),
            runtime_command: "nginx -c /app/nginx.conf".to_string(),
            health_check_url: "/".to_string(),
            working_directory: None,
        })
    }

    /// Detect from Dockerfile
    async fn detect_from_dockerfile(
        &self,
        _owner: &str,
        _repo: &str,
        _branch: &str,
        _path_filter: Option<&str>,
    ) -> Result<ProjectConfig, String> {
        // For Dockerfile-based projects, we use docker build
        Ok(ProjectConfig {
            project_type: "Dockerfile".to_string(),
            build_image: "docker:24-cli".to_string(),
            build_command: "docker build -t app .".to_string(),
            cache_type: "none".to_string(),
            runtime_image: "app".to_string(),
            runtime_command: "".to_string(),
            health_check_url: "/".to_string(),
            working_directory: None,
        })
    }

    /// Detect from GitHub Actions workflow
    async fn detect_from_github_actions(
        &self,
        owner: &str,
        repo: &str,
        branch: &str,
        workflow_files: &[&String],
    ) -> Result<ProjectConfig, String> {
        // Priority order for workflow files
        let priority_names = ["build.yml", "deploy.yml", "ci.yml", "cd.yml", "main.yml"];

        // Sort workflows by priority
        let mut sorted_workflows: Vec<&String> = workflow_files.to_vec();
        sorted_workflows.sort_by(|a, b| {
            let a_priority = priority_names.iter().position(|&name| a.ends_with(name));
            let b_priority = priority_names.iter().position(|&name| b.ends_with(name));

            match (a_priority, b_priority) {
                (Some(a_idx), Some(b_idx)) => a_idx.cmp(&b_idx),
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => a.cmp(b),
            }
        });

        // Try to parse workflows in priority order
        for workflow_path in sorted_workflows.iter().take(5) {
            if let Ok(content) = self.client.get_file_content(owner, repo, workflow_path, branch).await {
                // Check if workflow is active and relevant
                if let Ok(true) = WorkflowParser::is_active_for_branch(&content, branch) {
                    // Parse workflow to extract actual information
                    if let Ok(workflow_info) = WorkflowParser::parse(&content) {
                        // Build config from parsed information (no assumptions)
                        if let Ok(builder_config) = ConfigBuilder::from_workflow(&workflow_info) {
                            // Convert to ProjectConfig
                            return Ok(ProjectConfig {
                                project_type: builder_config.project_type,
                                build_image: builder_config.build_image,
                                build_command: builder_config.build_command,
                                cache_type: builder_config.cache_type,
                                runtime_image: builder_config.runtime_image,
                                runtime_command: builder_config.runtime_command,
                                health_check_url: builder_config.health_check_url,
                                working_directory: builder_config.working_directory,
                            });
                        }
                    }
                }
            }
        }

        Err("Could not find active GitHub Actions workflow for this branch".to_string())
    }
}
