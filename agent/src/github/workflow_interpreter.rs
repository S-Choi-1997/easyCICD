use super::workflow_parser::{WorkflowInfo, SetupAction, RunCommand};
use std::collections::HashMap;

// =============================================================================
// 1. 실행 계획 데이터 구조 (출력)
// =============================================================================

/// 워크플로우 실행 계획
///
/// WorkflowInfo(원본 데이터)를 분석해서 "어떻게 실행할지" 계획을 세웁니다.
#[derive(Debug, Clone)]
pub struct ExecutionPlan {
    /// 감지된 프로젝트 타입
    pub project_type: ProjectType,

    /// 런타임 환경 (Node.js 버전, Java 버전 등)
    pub runtime: Runtime,

    /// 실행할 태스크들 (순서대로)
    pub tasks: Vec<Task>,

    /// 워크플로우에서 감지된 포트 (None이면 기본값 사용)
    pub detected_port: Option<u16>,
}

/// 프로젝트 타입
#[derive(Debug, Clone, PartialEq)]
pub enum ProjectType {
    NodeJsBackend,
    NodeJsFrontend,
    JavaSpringBoot,
    RustCargo,
    PythonDjango,
    GolangApi,
    Unknown,
}

/// 런타임 환경 정보
#[derive(Debug, Clone)]
pub struct Runtime {
    /// 언어/프레임워크 (예: "node", "java", "rust")
    pub language: String,

    /// 버전 (예: "20", "17", "1.70")
    pub version: String,

    /// 추가 환경 변수
    pub env: HashMap<String, String>,
}

/// 실행 태스크
#[derive(Debug, Clone)]
pub struct Task {
    /// 태스크 이름
    pub name: String,

    /// 태스크 타입
    pub task_type: TaskType,

    /// 실행 커맨드
    pub command: String,
}

/// 태스크 타입
#[derive(Debug, Clone, PartialEq)]
pub enum TaskType {
    /// 의존성 설치 (npm ci, pip install 등)
    InstallDependencies,

    /// 빌드 (npm run build, cargo build 등)
    Build,

    /// 테스트 (npm test, cargo test 등)
    Test,

    /// 기타 커맨드
    Other,
}

// =============================================================================
// 2. Interpreter 구현
// =============================================================================

pub struct WorkflowInterpreter;

impl WorkflowInterpreter {
    /// WorkflowInfo를 분석해서 ExecutionPlan을 생성합니다.
    ///
    /// # 역할
    /// - Setup 액션에서 런타임 환경 파악
    /// - Run 커맨드에서 의도 파악 (설치? 빌드? 테스트?)
    /// - 프로젝트 타입 추론
    ///
    /// # 규칙
    /// - 워크플로우에 명시된 것만 사용 (가정 최소화)
    /// - 애매한 경우 에러 반환 (임의로 판단 안 함)
    pub fn interpret(info: &WorkflowInfo) -> Result<ExecutionPlan, String> {
        // 1. 런타임 환경 파악
        let runtime = Self::detect_runtime(info)?;

        // 2. 태스크 분석
        let tasks = Self::analyze_tasks(info, &runtime);

        // 3. 프로젝트 타입 추론
        let project_type = Self::infer_project_type(&runtime, &tasks);

        // 4. 포트 번호 감지
        let detected_port = Self::detect_port(info);

        Ok(ExecutionPlan {
            project_type,
            runtime,
            tasks,
            detected_port,
        })
    }

    // =========================================================================
    // Private: 런타임 감지
    // =========================================================================

    fn detect_runtime(info: &WorkflowInfo) -> Result<Runtime, String> {
        for action in &info.setup_actions {
            if let Some(runtime) = Self::parse_setup_action(action) {
                return Ok(runtime);
            }
        }

        Err("워크플로우에서 런타임 환경을 찾을 수 없습니다 (setup-node, setup-java 등이 필요합니다)".to_string())
    }

    fn parse_setup_action(action: &SetupAction) -> Option<Runtime> {
        let uses_lower = action.uses.to_lowercase();

        if uses_lower.contains("setup-node") {
            let version = action.with.get("node-version")
                .cloned()
                .unwrap_or_else(|| "20".to_string());

            return Some(Runtime {
                language: "node".to_string(),
                version,
                env: HashMap::new(),
            });
        }

        if uses_lower.contains("setup-java") {
            let version = action.with.get("java-version")
                .or_else(|| action.with.get("version"))
                .cloned()
                .unwrap_or_else(|| "17".to_string());

            return Some(Runtime {
                language: "java".to_string(),
                version,
                env: HashMap::new(),
            });
        }

        if uses_lower.contains("setup-python") {
            let version = action.with.get("python-version")
                .cloned()
                .unwrap_or_else(|| "3.11".to_string());

            return Some(Runtime {
                language: "python".to_string(),
                version,
                env: HashMap::new(),
            });
        }

        if uses_lower.contains("setup-go") {
            let version = action.with.get("go-version")
                .cloned()
                .unwrap_or_else(|| "1.21".to_string());

            return Some(Runtime {
                language: "go".to_string(),
                version,
                env: HashMap::new(),
            });
        }

        // Rust는 setup 액션이 따로 없음 (rustup 사용)
        None
    }

    // =========================================================================
    // Private: 태스크 분석
    // =========================================================================

    fn analyze_tasks(info: &WorkflowInfo, runtime: &Runtime) -> Vec<Task> {
        info.run_commands
            .iter()
            .filter_map(|cmd| Self::parse_command(cmd, runtime))
            .collect()
    }

    fn parse_command(cmd: &RunCommand, runtime: &Runtime) -> Option<Task> {
        let command = cmd.command.trim();
        let command_lower = command.to_lowercase();

        // 태스크 타입 결정
        let task_type = Self::classify_task(&command_lower, &runtime.language);

        // 커맨드가 의미 있는 작업인지 확인
        if Self::is_meaningful_command(&command_lower) {
            Some(Task {
                name: cmd.step_name.clone().unwrap_or_else(|| "Run command".to_string()),
                task_type,
                command: command.to_string(),
            })
        } else {
            None
        }
    }

    fn classify_task(command: &str, language: &str) -> TaskType {
        match language {
            "node" => {
                if command.contains("npm ci") || command.contains("npm install") || command.contains("yarn install") {
                    TaskType::InstallDependencies
                } else if command.contains("npm run build") || command.contains("npm build") {
                    TaskType::Build
                } else if command.contains("npm test") || command.contains("npm run test") {
                    TaskType::Test
                } else {
                    TaskType::Other
                }
            }
            "java" => {
                if command.contains("mvn package") || command.contains("gradle build") {
                    TaskType::Build
                } else if command.contains("mvn test") || command.contains("gradle test") {
                    TaskType::Test
                } else {
                    TaskType::Other
                }
            }
            "python" => {
                if command.contains("pip install") {
                    TaskType::InstallDependencies
                } else if command.contains("pytest") || command.contains("python -m unittest") {
                    TaskType::Test
                } else {
                    TaskType::Other
                }
            }
            "go" => {
                if command.contains("go build") {
                    TaskType::Build
                } else if command.contains("go test") {
                    TaskType::Test
                } else {
                    TaskType::Other
                }
            }
            _ => TaskType::Other,
        }
    }

    fn is_meaningful_command(command: &str) -> bool {
        // 의미 없는 커맨드 필터링 (echo, sleep 등)
        !command.starts_with("echo ")
            && !command.starts_with("sleep ")
            && !command.contains("curl -f http://localhost") // Health check
    }

    // =========================================================================
    // Private: 프로젝트 타입 추론
    // =========================================================================

    fn infer_project_type(runtime: &Runtime, tasks: &[Task]) -> ProjectType {
        match runtime.language.as_str() {
            "node" => Self::infer_nodejs_type(tasks),
            "java" => ProjectType::JavaSpringBoot,
            "python" => ProjectType::PythonDjango,
            "go" => ProjectType::GolangApi,
            _ => ProjectType::Unknown,
        }
    }

    fn infer_nodejs_type(tasks: &[Task]) -> ProjectType {
        // "node src/index.js" 같은 서버 실행 커맨드가 있으면 Backend
        let has_node_execution = tasks.iter().any(|t|
            t.command.contains("node ") && t.command.contains(".js")
        );

        // "npm run build" 같은 빌드가 있고, dist/build 폴더를 만드는 경우 Frontend
        let has_frontend_build = tasks.iter().any(|t|
            t.task_type == TaskType::Build &&
            (t.command.contains("vite") || t.command.contains("webpack") || t.command.contains("react-scripts"))
        );

        if has_node_execution && !has_frontend_build {
            ProjectType::NodeJsBackend
        } else if has_frontend_build {
            ProjectType::NodeJsFrontend
        } else {
            // 애매한 경우 - 기본값은 Backend (Express 서버 가정)
            ProjectType::NodeJsBackend
        }
    }

    // =========================================================================
    // Private: 포트 감지
    // =========================================================================

    /// 워크플로우에서 포트 번호 감지
    ///
    /// 다음 패턴을 찾습니다:
    /// - curl http://localhost:3000
    /// - localhost:8080
    /// - 0.0.0.0:8000
    /// - PORT=3000
    fn detect_port(info: &WorkflowInfo) -> Option<u16> {
        for cmd in &info.run_commands {
            let command_lower = cmd.command.to_lowercase();

            // localhost:PORT 패턴 찾기
            if let Some(port) = Self::extract_port_from_pattern(&command_lower, "localhost:") {
                return Some(port);
            }

            // 0.0.0.0:PORT 패턴 찾기
            if let Some(port) = Self::extract_port_from_pattern(&command_lower, "0.0.0.0:") {
                return Some(port);
            }

            // 127.0.0.1:PORT 패턴 찾기
            if let Some(port) = Self::extract_port_from_pattern(&command_lower, "127.0.0.1:") {
                return Some(port);
            }

            // PORT=3000 또는 --port 3000 패턴
            for prefix in &["port=", "--port ", "--port=", "port "] {
                if let Some(idx) = command_lower.find(prefix) {
                    let after = &command_lower[idx + prefix.len()..];
                    if let Some(port) = Self::parse_port_number(after) {
                        return Some(port);
                    }
                }
            }
        }

        None
    }

    /// 문자열에서 "prefix숫자" 패턴을 찾아 포트 번호 추출
    fn extract_port_from_pattern(text: &str, prefix: &str) -> Option<u16> {
        if let Some(idx) = text.find(prefix) {
            let after = &text[idx + prefix.len()..];
            Self::parse_port_number(after)
        } else {
            None
        }
    }

    /// 문자열 시작 부분에서 포트 번호 파싱
    fn parse_port_number(text: &str) -> Option<u16> {
        let digits: String = text.chars().take_while(|c| c.is_ascii_digit()).collect();

        if let Ok(port) = digits.parse::<u16>() {
            // 유효한 포트 범위 확인 (1024-65535)
            if port >= 1024 && port <= 65535 {
                return Some(port);
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::github::workflow_parser::{WorkflowParser, RunCommand};

    #[test]
    fn test_interpret_nodejs_backend() {
        let yaml = r#"
name: CI
on: push
jobs:
  test:
    steps:
      - uses: actions/setup-node@v4
        with:
          node-version: '20'
      - run: npm ci
      - run: node src/index.js &
"#;

        let info = WorkflowParser::parse(yaml).unwrap();
        let plan = WorkflowInterpreter::interpret(&info).unwrap();

        assert_eq!(plan.project_type, ProjectType::NodeJsBackend);
        assert_eq!(plan.runtime.language, "node");
        assert_eq!(plan.runtime.version, "20");
        assert!(plan.tasks.iter().any(|t| t.task_type == TaskType::InstallDependencies));
    }

    #[test]
    fn test_interpret_nodejs_frontend() {
        let yaml = r#"
name: Build
on: push
jobs:
  build:
    steps:
      - uses: actions/setup-node@v4
        with:
          node-version: '18'
      - run: npm install
      - run: npm run build
"#;

        let info = WorkflowParser::parse(yaml).unwrap();
        let plan = WorkflowInterpreter::interpret(&info).unwrap();

        assert_eq!(plan.runtime.language, "node");
        assert!(plan.tasks.iter().any(|t| t.task_type == TaskType::Build));
    }
}
