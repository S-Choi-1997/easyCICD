use serde::Deserialize;
use std::collections::HashMap;

// =============================================================================
// 1. 파싱 결과 데이터 구조 (출력)
// =============================================================================

/// GitHub Actions 워크플로우 파싱 결과
///
/// 이 구조체는 YAML 파일의 내용을 "있는 그대로" 담습니다.
/// 어떤 언어인지, 어떻게 빌드해야 하는지 등의 "해석"은 하지 않습니다.
#[derive(Debug, Clone, Default)]
pub struct WorkflowInfo {
    /// 워크플로우 이름 (name 필드)
    pub name: String,

    /// 모든 setup 액션들 (순서 보장)
    pub setup_actions: Vec<SetupAction>,

    /// 모든 run 커맨드들 (순서 보장)
    pub run_commands: Vec<RunCommand>,

    /// 워크플로우 트리거 정보
    pub triggers: Vec<String>,
}

/// Setup 액션 정보
#[derive(Debug, Clone)]
pub struct SetupAction {
    /// 스텝 이름 (예: "Setup Node.js")
    pub step_name: Option<String>,

    /// 사용된 액션 (예: "actions/setup-node@v4")
    pub uses: String,

    /// with 파라미터들
    pub with: HashMap<String, String>,
}

/// Run 커맨드 정보
#[derive(Debug, Clone)]
pub struct RunCommand {
    /// 스텝 이름 (예: "Install dependencies")
    pub step_name: Option<String>,

    /// 실행 커맨드 (멀티라인 가능)
    pub command: String,
}

// =============================================================================
// 2. YAML 구조 정의 (serde로 자동 파싱)
// =============================================================================

#[derive(Debug, Deserialize)]
struct GitHubWorkflow {
    #[serde(default)]
    name: String,

    #[serde(default)]
    on: WorkflowTrigger,

    #[serde(default)]
    jobs: HashMap<String, Job>,
}

#[derive(Debug, Deserialize, Default)]
#[serde(untagged)]
enum WorkflowTrigger {
    #[default]
    None,
    Simple(String),
    Array(Vec<String>),
    Object(HashMap<String, serde_yaml::Value>),
}

#[derive(Debug, Deserialize)]
struct Job {
    #[serde(default)]
    steps: Vec<Step>,
}

#[derive(Debug, Deserialize)]
struct Step {
    name: Option<String>,
    uses: Option<String>,
    run: Option<String>,

    #[serde(default)]
    with: HashMap<String, serde_yaml::Value>,
}

// =============================================================================
// 3. 파서 구현
// =============================================================================

pub struct WorkflowParser;

impl WorkflowParser {
    /// GitHub Actions 워크플로우 YAML을 파싱합니다.
    ///
    /// # 역할
    /// - YAML을 Rust 구조체로 변환
    /// - 데이터 추출 및 정리
    /// - 어떤 "해석"이나 "가정"도 하지 않음
    ///
    /// # 에러
    /// - YAML 문법 오류
    /// - 필수 필드 누락
    pub fn parse(content: &str) -> Result<WorkflowInfo, String> {
        let workflow: GitHubWorkflow = serde_yaml::from_str(content)
            .map_err(|e| format!("YAML 파싱 실패: {}", e))?;

        let mut info = WorkflowInfo {
            name: workflow.name,
            triggers: Self::extract_triggers(&workflow.on),
            ..Default::default()
        };

        // Jobs를 순회하며 steps 추출
        for (_job_name, job) in workflow.jobs {
            for step in job.steps {
                // Setup 액션 추출
                if let Some(uses) = step.uses {
                    info.setup_actions.push(SetupAction {
                        step_name: step.name.clone(),
                        uses,
                        with: Self::normalize_with_params(step.with.clone()),
                    });
                }

                // Run 커맨드 추출
                if let Some(command) = step.run {
                    info.run_commands.push(RunCommand {
                        step_name: step.name,
                        command,
                    });
                }
            }
        }

        Ok(info)
    }

    /// 특정 브랜치에서 이 워크플로우가 활성화되는지 확인
    pub fn is_active_for_branch(content: &str, branch: &str) -> Result<bool, String> {
        let workflow: GitHubWorkflow = serde_yaml::from_str(content)
            .map_err(|e| format!("YAML 파싱 실패: {}", e))?;

        match workflow.on {
            WorkflowTrigger::Simple(ref trigger) => {
                // on: push, on: pull_request 등
                Ok(trigger == "push" || trigger == "pull_request" || trigger == "workflow_dispatch")
            }
            WorkflowTrigger::Array(ref triggers) => {
                // on: [push, pull_request]
                Ok(triggers.iter().any(|t|
                    t == "push" || t == "pull_request" || t == "workflow_dispatch"
                ))
            }
            WorkflowTrigger::Object(ref obj) => {
                // on: { push: { branches: [main] } }
                if obj.contains_key("workflow_dispatch") {
                    return Ok(true);
                }

                if let Some(push) = obj.get("push") {
                    if let Some(branches) = push.get("branches") {
                        if let Some(branch_list) = branches.as_sequence() {
                            return Ok(branch_list.iter().any(|b| {
                                if let Some(b_str) = b.as_str() {
                                    b_str == branch || b_str == "**" || b_str == "*"
                                } else {
                                    false
                                }
                            }));
                        }
                    }
                    return Ok(true);
                }

                Ok(obj.contains_key("push") || obj.contains_key("pull_request"))
            }
            WorkflowTrigger::None => Ok(false),
        }
    }

    // =========================================================================
    // Private helpers
    // =========================================================================

    fn extract_triggers(trigger: &WorkflowTrigger) -> Vec<String> {
        match trigger {
            WorkflowTrigger::Simple(s) => vec![s.clone()],
            WorkflowTrigger::Array(arr) => arr.clone(),
            WorkflowTrigger::Object(obj) => obj.keys().cloned().collect(),
            WorkflowTrigger::None => vec![],
        }
    }

    fn normalize_with_params(params: HashMap<String, serde_yaml::Value>) -> HashMap<String, String> {
        params
            .into_iter()
            .filter_map(|(k, v)| {
                // Value를 String으로 변환
                let value_str = match v {
                    serde_yaml::Value::String(s) => s,
                    serde_yaml::Value::Number(n) => n.to_string(),
                    serde_yaml::Value::Bool(b) => b.to_string(),
                    _ => return None,
                };
                Some((k, value_str))
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_workflow() {
        let yaml = r#"
name: CI
on: push
jobs:
  build:
    steps:
      - uses: actions/checkout@v3
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
      - run: npm ci
      - run: npm test
"#;

        let info = WorkflowParser::parse(yaml).unwrap();
        assert_eq!(info.name, "CI");
        assert_eq!(info.setup_actions.len(), 2);
        assert_eq!(info.run_commands.len(), 2);
        assert_eq!(info.run_commands[0].command, "npm ci");
    }

    #[test]
    fn test_branch_detection() {
        let yaml = r#"
name: CI
on:
  push:
    branches: [main, develop]
jobs:
  build:
    steps:
      - run: echo "test"
"#;

        assert!(WorkflowParser::is_active_for_branch(yaml, "main").unwrap());
        assert!(WorkflowParser::is_active_for_branch(yaml, "develop").unwrap());
        assert!(!WorkflowParser::is_active_for_branch(yaml, "feature").unwrap());
    }
}
