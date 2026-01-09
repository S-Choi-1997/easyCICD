use super::workflow_parser::{WorkflowInfo, SetupActionType};

#[derive(Debug, Clone)]
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

pub struct ConfigBuilder;

impl ConfigBuilder {
    /// Build configuration from parsed workflow information
    /// Only uses what's actually in the workflow - NO ASSUMPTIONS
    pub fn from_workflow(info: &WorkflowInfo) -> Result<ProjectConfig, String> {
        // Find the primary language setup
        let (setup_type, setup_action) = info.setup_actions
            .iter()
            .next()
            .ok_or("No setup action found in workflow")?;

        match setup_action.action_type {
            SetupActionType::Java => Self::build_java_config(info, setup_action),
            SetupActionType::Node => Self::build_node_config(info, setup_action),
            SetupActionType::Python => Self::build_python_config(info, setup_action),
            SetupActionType::Go => Self::build_go_config(info, setup_action),
            SetupActionType::Rust => Self::build_rust_config(info, setup_action),
            _ => Err("Unsupported language setup action".to_string()),
        }
    }

    fn build_java_config(
        info: &WorkflowInfo,
        setup: &super::workflow_parser::SetupAction,
    ) -> Result<ProjectConfig, String> {
        let version = setup.version.as_deref().unwrap_or("17");

        // Determine if Gradle or Maven from actual commands
        let is_gradle = info.build_commands.iter().any(|cmd|
            cmd.contains("gradle") || cmd.contains("gradlew")
        );
        let is_maven = info.build_commands.iter().any(|cmd|
            cmd.contains("mvn")
        );

        if !is_gradle && !is_maven {
            return Err("No Gradle or Maven build command found in workflow".to_string());
        }

        // Build image based on actual tool
        let build_image = if is_gradle {
            format!("gradle:8-jdk{}", version)
        } else {
            format!("maven:3.9-eclipse-temurin-{}", version)
        };

        // Use actual build commands from workflow
        let build_command = if !info.build_commands.is_empty() {
            // Use the actual commands but ensure output copy
            let cmds = info.build_commands.join(" && ");
            // Add output copy if not present
            if cmds.contains("/output") {
                cmds
            } else if is_gradle {
                // Exclude *-plain.jar files that Spring Boot 2.5+ generates
                format!("{} && find build/libs -name '*.jar' ! -name '*-plain.jar' -exec cp {{}} /output/app.jar \\;", cmds)
            } else {
                format!("{} && cp target/*.jar /output/app.jar", cmds)
            }
        } else {
            return Err("No build commands found in workflow".to_string());
        };

        // Runtime image from distribution
        let runtime_image = match setup.distribution.as_deref() {
            Some("temurin") | Some("adopt") | Some("adoptium") => {
                format!("eclipse-temurin:{}-jre", version)
            }
            Some("corretto") => format!("amazoncorretto:{}", version),
            Some("zulu") => format!("azul/zulu-openjdk:{}-jre", version),
            Some(other) => {
                return Err(format!("Unknown Java distribution: {}", other));
            }
            None => {
                return Err("Java distribution not specified in workflow".to_string());
            }
        };

        Ok(ProjectConfig {
            project_type: format!("Java ({})", if is_gradle { "Gradle" } else { "Maven" }),
            build_image,
            build_command,
            cache_type: if is_gradle { "gradle" } else { "maven" }.to_string(),
            runtime_image,
            runtime_command: "java -jar app.jar".to_string(),
            health_check_url: "/actuator/health".to_string(),
            working_directory: None,
        })
    }

    fn build_node_config(
        info: &WorkflowInfo,
        setup: &super::workflow_parser::SetupAction,
    ) -> Result<ProjectConfig, String> {
        let version = setup.version.as_deref().unwrap_or("20");

        if info.build_commands.is_empty() {
            return Err("No build commands found in workflow".to_string());
        }

        // Check if it's a static site or server app
        // Backend indicators: node execution commands
        let is_backend = info.run_commands.iter().any(|cmd|
            cmd.contains("node ") ||  // node src/index.js, node dist/index.js, etc.
            cmd.contains("npm start") ||
            cmd.contains("npm run start") ||
            cmd.contains("npm run dev")
        );

        // Static site indicators: build tools without node execution
        let is_static = !is_backend && info.run_commands.iter().any(|cmd|
            cmd.contains("vite build") ||
            cmd.contains("webpack") ||
            cmd.contains("gh-pages")
        );

        // Determine build command based on project type
        let build_command = if is_backend {
            // Backend: install dependencies and copy source code
            "npm ci && cp -r src package*.json /output/".to_string()
        } else {
            // Frontend: use workflow build commands
            let cmds = info.build_commands.join(" && ");
            if cmds.contains("/output") {
                cmds
            } else {
                format!("{} && cp -r dist/* /output/ 2>/dev/null || cp -r build/* /output/", cmds)
            }
        };

        // Extract runtime command for backend
        let runtime_command = if is_static {
            "nginx -c /app/nginx.conf".to_string()
        } else {
            // Try to find actual node execution command from workflow
            let node_cmd = info.run_commands.iter()
                .find(|cmd| cmd.contains("node "))
                .and_then(|cmd| {
                    // Extract "node xxx.js" from multiline command
                    cmd.lines()
                        .map(|line| line.trim())
                        .filter(|line| !line.is_empty() && !line.starts_with('#'))
                        .find(|line| line.starts_with("node "))
                        .map(|line| {
                            // Remove any trailing characters like & or ;
                            line.split_whitespace()
                                .take_while(|part| !part.starts_with('#') && *part != "&" && *part != ";")
                                .collect::<Vec<_>>()
                                .join(" ")
                        })
                });

            node_cmd.unwrap_or_else(|| "node dist/index.js".to_string())
        };

        Ok(ProjectConfig {
            project_type: format!("Node.js {}", if is_static { "(Static)" } else { "(Server)" }),
            build_image: format!("node:{}", version),
            build_command,
            cache_type: "npm".to_string(),
            runtime_image: if is_static {
                "nginx:alpine".to_string()
            } else {
                format!("node:{}-slim", version)
            },
            runtime_command,
            health_check_url: "/".to_string(),
            working_directory: None,
        })
    }

    fn build_python_config(
        info: &WorkflowInfo,
        setup: &super::workflow_parser::SetupAction,
    ) -> Result<ProjectConfig, String> {
        let version = setup.version.as_deref().unwrap_or("3.11");

        if info.build_commands.is_empty() && info.run_commands.is_empty() {
            return Err("No commands found in workflow".to_string());
        }

        let build_command = if !info.build_commands.is_empty() {
            info.build_commands.join(" && ")
        } else {
            "pip install -r requirements.txt && cp -r . /output/".to_string()
        };

        Ok(ProjectConfig {
            project_type: "Python".to_string(),
            build_image: format!("python:{}", version),
            build_command,
            cache_type: "pip".to_string(),
            runtime_image: format!("python:{}-slim", version),
            runtime_command: "python main.py".to_string(),
            health_check_url: "/health".to_string(),
            working_directory: None,
        })
    }

    fn build_go_config(
        info: &WorkflowInfo,
        setup: &super::workflow_parser::SetupAction,
    ) -> Result<ProjectConfig, String> {
        let version = setup.version.as_deref().unwrap_or("1.21");

        if info.build_commands.is_empty() {
            return Err("No build commands found in workflow".to_string());
        }

        let build_command = {
            let cmds = info.build_commands.join(" && ");
            if cmds.contains("/output") {
                cmds
            } else {
                format!("{} && cp app /output/", cmds)
            }
        };

        Ok(ProjectConfig {
            project_type: "Go".to_string(),
            build_image: format!("golang:{}", version),
            build_command,
            cache_type: "go".to_string(),
            runtime_image: "debian:bookworm-slim".to_string(),
            runtime_command: "./app".to_string(),
            health_check_url: "/health".to_string(),
            working_directory: None,
        })
    }

    fn build_rust_config(
        info: &WorkflowInfo,
        setup: &super::workflow_parser::SetupAction,
    ) -> Result<ProjectConfig, String> {
        if info.build_commands.is_empty() {
            return Err("No build commands found in workflow".to_string());
        }

        let build_command = {
            let cmds = info.build_commands.join(" && ");
            if cmds.contains("/output") {
                cmds
            } else {
                format!("{} && find target/release -maxdepth 1 -type f -executable -exec cp {{}} /output/ \\;", cmds)
            }
        };

        Ok(ProjectConfig {
            project_type: "Rust".to_string(),
            build_image: "rust:1.75".to_string(),
            build_command,
            cache_type: "rust".to_string(),
            runtime_image: "debian:bookworm-slim".to_string(),
            runtime_command: "./app".to_string(),
            health_check_url: "/health".to_string(),
            working_directory: None,
        })
    }
}
