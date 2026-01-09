use serde_yaml::Value as YamlValue;
use std::collections::HashMap;

/// Parsed information from GitHub Actions workflow
#[derive(Debug, Clone, Default)]
pub struct WorkflowInfo {
    pub setup_actions: HashMap<String, SetupAction>,
    pub run_commands: Vec<String>,
    pub build_commands: Vec<String>,
    pub test_commands: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct SetupAction {
    pub action_type: SetupActionType,
    pub version: Option<String>,
    pub distribution: Option<String>,
    pub other_params: HashMap<String, String>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SetupActionType {
    Node,
    Java,
    Python,
    Go,
    Rust,
    Unknown,
}

pub struct WorkflowParser;

impl WorkflowParser {
    /// Parse GitHub Actions workflow YAML and extract only what's actually there
    pub fn parse(content: &str) -> Result<WorkflowInfo, String> {
        let yaml: YamlValue = serde_yaml::from_str(content)
            .map_err(|e| format!("Failed to parse YAML: {}", e))?;

        let mut info = WorkflowInfo::default();

        // Extract information from jobs
        if let Some(jobs) = yaml.get("jobs").and_then(|j| j.as_mapping()) {
            for (_job_name, job) in jobs {
                if let Some(steps) = job.get("steps").and_then(|s| s.as_sequence()) {
                    for step in steps {
                        // Parse setup actions
                        if let Some(uses) = step.get("uses").and_then(|u| u.as_str()) {
                            if let Some(setup) = Self::parse_setup_action(uses, step) {
                                let key = format!("{:?}", setup.action_type);
                                info.setup_actions.insert(key, setup);
                            }
                        }

                        // Parse run commands
                        if let Some(run) = step.get("run").and_then(|r| r.as_str()) {
                            info.run_commands.push(run.to_string());

                            // Categorize commands
                            let run_lower = run.to_lowercase();
                            if run_lower.contains("build") ||
                               run_lower.contains("gradle") ||
                               run_lower.contains("gradlew") ||
                               run_lower.contains("mvn") ||
                               run_lower.contains("npm run build") ||
                               run_lower.contains("cargo build") {
                                info.build_commands.push(run.to_string());
                            }
                            if run_lower.contains("test") {
                                info.test_commands.push(run.to_string());
                            }
                        }
                    }
                }
            }
        }

        Ok(info)
    }

    fn parse_setup_action(uses: &str, step: &YamlValue) -> Option<SetupAction> {
        let action_type = if uses.contains("setup-node") {
            SetupActionType::Node
        } else if uses.contains("setup-java") {
            SetupActionType::Java
        } else if uses.contains("setup-python") {
            SetupActionType::Python
        } else if uses.contains("setup-go") {
            SetupActionType::Go
        } else if uses.contains("setup-rust") {
            SetupActionType::Rust
        } else {
            return None;
        };

        let mut setup = SetupAction {
            action_type,
            version: None,
            distribution: None,
            other_params: HashMap::new(),
        };

        // Extract 'with' parameters
        if let Some(with) = step.get("with").and_then(|w| w.as_mapping()) {
            for (key, value) in with {
                if let (Some(key_str), Some(value_str)) = (key.as_str(), value.as_str()) {
                    match key_str {
                        "node-version" | "java-version" | "python-version" | "go-version" => {
                            setup.version = Some(value_str.to_string());
                        }
                        "distribution" => {
                            setup.distribution = Some(value_str.to_string());
                        }
                        _ => {
                            setup.other_params.insert(key_str.to_string(), value_str.to_string());
                        }
                    }
                }
            }
        }

        Some(setup)
    }

    /// Check if workflow is active for the given branch
    pub fn is_active_for_branch(content: &str, branch: &str) -> Result<bool, String> {
        let yaml: YamlValue = serde_yaml::from_str(content)
            .map_err(|e| format!("Failed to parse YAML: {}", e))?;

        if let Some(on) = yaml.get("on") {
            // Simple trigger like "on: push"
            if on.as_str().is_some() {
                return Ok(true);
            }

            // Object trigger
            if let Some(on_map) = on.as_mapping() {
                if on_map.contains_key("push") || on_map.contains_key("pull_request") {
                    if let Some(push) = on_map.get("push") {
                        if let Some(branches) = push.get("branches") {
                            if let Some(branch_list) = branches.as_sequence() {
                                for b in branch_list {
                                    if let Some(branch_name) = b.as_str() {
                                        if branch_name == branch || branch_name == "**" || branch_name == "*" {
                                            return Ok(true);
                                        }
                                    }
                                }
                                return Ok(false);
                            }
                        }
                        return Ok(true);
                    }
                    return Ok(true);
                }

                // workflow_dispatch means manual trigger
                if on_map.contains_key("workflow_dispatch") {
                    return Ok(true);
                }
            }

            // Array trigger
            if let Some(on_array) = on.as_sequence() {
                for trigger in on_array {
                    if let Some(trigger_str) = trigger.as_str() {
                        if trigger_str == "push" || trigger_str == "pull_request" || trigger_str == "workflow_dispatch" {
                            return Ok(true);
                        }
                    }
                }
            }
        }

        Ok(false)
    }
}
