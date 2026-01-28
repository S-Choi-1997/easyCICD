use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum Slot {
    Blue,
    Green,
}

impl std::fmt::Display for Slot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Slot::Blue => write!(f, "Blue"),
            Slot::Green => write!(f, "Green"),
        }
    }
}

impl std::str::FromStr for Slot {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Blue" => Ok(Slot::Blue),
            "Green" => Ok(Slot::Green),
            _ => Err(format!("Invalid slot: {}", s)),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum BuildStatus {
    Queued,
    Building,
    Success,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeploymentStatus {
    NotDeployed,
    Deploying,
    Deployed,
    Failed,
}

impl std::fmt::Display for BuildStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildStatus::Queued => write!(f, "Queued"),
            BuildStatus::Building => write!(f, "Building"),
            BuildStatus::Success => write!(f, "Success"),
            BuildStatus::Failed => write!(f, "Failed"),
        }
    }
}

impl std::str::FromStr for BuildStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "Queued" => Ok(BuildStatus::Queued),
            "Building" => Ok(BuildStatus::Building),
            "Success" => Ok(BuildStatus::Success),
            "Failed" => Ok(BuildStatus::Failed),
            // 하위 호환성: 기존 Deploying 상태는 Success로 처리
            "Deploying" => Ok(BuildStatus::Success),
            _ => Err(format!("Invalid build status: {}", s)),
        }
    }
}

impl std::fmt::Display for DeploymentStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DeploymentStatus::NotDeployed => write!(f, "NotDeployed"),
            DeploymentStatus::Deploying => write!(f, "Deploying"),
            DeploymentStatus::Deployed => write!(f, "Deployed"),
            DeploymentStatus::Failed => write!(f, "Failed"),
        }
    }
}

impl std::str::FromStr for DeploymentStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "NotDeployed" => Ok(DeploymentStatus::NotDeployed),
            "Deploying" => Ok(DeploymentStatus::Deploying),
            "Deployed" => Ok(DeploymentStatus::Deployed),
            "Failed" => Ok(DeploymentStatus::Failed),
            _ => Err(format!("Invalid deployment status: {}", s)),
        }
    }
}

impl TryFrom<String> for DeploymentStatus {
    type Error = String;

    fn try_from(s: String) -> Result<Self, Self::Error> {
        s.parse()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Project {
    pub id: i64,

    // Project identification
    pub name: String,
    pub repo: String,
    pub path_filter: String,
    pub branch: String,

    // Build configuration
    pub build_image: String,
    pub build_command: String,
    pub cache_type: String,
    pub working_directory: Option<String>,

    // Deploy configuration
    pub runtime_image: String,
    pub runtime_command: String,
    pub health_check_url: String,
    pub runtime_port: i32,  // 컨테이너 내부에서 앱이 listen하는 포트

    // Environment variables (JSON string)
    pub build_env_vars: Option<String>,
    pub runtime_env_vars: Option<String>,

    // Port allocation
    pub blue_port: i32,
    pub green_port: i32,
    #[sqlx(try_from = "String")]
    pub active_slot: Slot,

    // Container IDs
    pub blue_container_id: Option<String>,
    pub green_container_id: Option<String>,

    // Status
    #[sqlx(try_from = "String")]
    pub deployment_status: DeploymentStatus,

    // GitHub webhook
    pub github_webhook_id: Option<i64>,

    // Timestamps
    pub created_at: String,
    pub updated_at: String,
}

impl Project {
    pub fn get_active_port(&self) -> i32 {
        match self.active_slot {
            Slot::Blue => self.blue_port,
            Slot::Green => self.green_port,
        }
    }

    pub fn get_inactive_slot(&self) -> Slot {
        match self.active_slot {
            Slot::Blue => Slot::Green,
            Slot::Green => Slot::Blue,
        }
    }

    pub fn get_inactive_port(&self) -> i32 {
        match self.active_slot {
            Slot::Blue => self.green_port,
            Slot::Green => self.blue_port,
        }
    }

    pub fn get_slot_port(&self, slot: &Slot) -> i32 {
        match slot {
            Slot::Blue => self.blue_port,
            Slot::Green => self.green_port,
        }
    }

    pub fn get_container_id(&self, slot: &Slot) -> Option<&String> {
        match slot {
            Slot::Blue => self.blue_container_id.as_ref(),
            Slot::Green => self.green_container_id.as_ref(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Build {
    pub id: i64,
    pub project_id: i64,
    pub build_number: i64,
    pub commit_hash: String,
    pub commit_message: Option<String>,
    pub author: Option<String>,

    #[sqlx(try_from = "String")]
    pub status: BuildStatus,

    pub log_path: String,
    pub deploy_log_path: Option<String>,
    pub output_path: Option<String>,

    pub deployed_slot: Option<String>,

    pub started_at: String,
    pub finished_at: Option<String>,
}

impl Build {
    pub fn get_deployed_slot(&self) -> Option<Slot> {
        self.deployed_slot.as_ref().and_then(|s| s.parse().ok())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateProject {
    pub name: String,
    pub repo: String,
    pub path_filter: String,
    pub branch: String,

    pub build_image: String,
    pub build_command: String,
    pub cache_type: String,
    pub working_directory: Option<String>,
    pub build_env_vars: Option<String>,

    pub runtime_image: String,
    pub runtime_command: String,
    pub health_check_url: String,
    pub runtime_port: i32,
    pub runtime_env_vars: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateProject {
    pub name: Option<String>,
    pub repo: Option<String>,
    pub path_filter: Option<String>,
    pub branch: Option<String>,

    pub build_image: Option<String>,
    pub build_command: Option<String>,
    pub cache_type: Option<String>,
    pub working_directory: Option<String>,
    pub build_env_vars: Option<String>,

    pub runtime_image: Option<String>,
    pub runtime_command: Option<String>,
    pub health_check_url: Option<String>,
    pub runtime_port: Option<i32>,
    pub runtime_env_vars: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateBuild {
    pub project_id: i64,
    pub commit_hash: String,
    pub commit_message: Option<String>,
    pub author: Option<String>,
}

// Try conversion for Slot from String (for sqlx)
impl sqlx::Type<sqlx::Sqlite> for Slot {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <String as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for Slot {
    fn encode_by_ref(&self, args: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        args.push(sqlx::sqlite::SqliteArgumentValue::Text(
            std::borrow::Cow::Owned(self.to_string()),
        ));
        Ok(sqlx::encode::IsNull::No)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for Slot {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s: String = <String as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
        s.parse().map_err(|e: String| Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)) as Box<dyn std::error::Error + Send + Sync>)
    }
}

impl From<String> for Slot {
    fn from(s: String) -> Self {
        s.parse().unwrap_or(Slot::Blue)
    }
}

// Try conversion for BuildStatus from String (for sqlx)
impl sqlx::Type<sqlx::Sqlite> for BuildStatus {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <String as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for BuildStatus {
    fn encode_by_ref(&self, args: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        args.push(sqlx::sqlite::SqliteArgumentValue::Text(
            std::borrow::Cow::Owned(self.to_string()),
        ));
        Ok(sqlx::encode::IsNull::No)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for BuildStatus {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s: String = <String as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
        s.parse().map_err(|e: String| Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)) as Box<dyn std::error::Error + Send + Sync>)
    }
}

impl From<String> for BuildStatus {
    fn from(s: String) -> Self {
        s.parse().unwrap_or(BuildStatus::Queued)
    }
}

// Container status enum
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ContainerStatus {
    Running,
    Stopped,
    Pulling,   // 이미지 풀링 중
    Starting,  // 컨테이너 시작 중
}

impl std::fmt::Display for ContainerStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ContainerStatus::Running => write!(f, "running"),
            ContainerStatus::Stopped => write!(f, "stopped"),
            ContainerStatus::Pulling => write!(f, "pulling"),
            ContainerStatus::Starting => write!(f, "starting"),
        }
    }
}

impl std::str::FromStr for ContainerStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "running" => Ok(ContainerStatus::Running),
            "stopped" => Ok(ContainerStatus::Stopped),
            "pulling" => Ok(ContainerStatus::Pulling),
            "starting" => Ok(ContainerStatus::Starting),
            _ => Err(format!("Invalid container status: {}", s)),
        }
    }
}

impl sqlx::Type<sqlx::Sqlite> for ContainerStatus {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <String as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for ContainerStatus {
    fn encode_by_ref(&self, args: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        args.push(sqlx::sqlite::SqliteArgumentValue::Text(
            std::borrow::Cow::Owned(self.to_string()),
        ));
        Ok(sqlx::encode::IsNull::No)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for ContainerStatus {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s: String = <String as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
        s.parse().map_err(|e: String| Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)) as Box<dyn std::error::Error + Send + Sync>)
    }
}

impl From<String> for ContainerStatus {
    fn from(s: String) -> Self {
        s.parse().unwrap_or(ContainerStatus::Stopped)
    }
}

// Protocol type for containers
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProtocolType {
    #[serde(rename = "tcp")]
    Tcp,
    #[serde(rename = "http")]
    Http,
}

impl Default for ProtocolType {
    fn default() -> Self {
        ProtocolType::Tcp
    }
}

impl std::fmt::Display for ProtocolType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProtocolType::Tcp => write!(f, "tcp"),
            ProtocolType::Http => write!(f, "http"),
        }
    }
}

impl std::str::FromStr for ProtocolType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "tcp" => Ok(ProtocolType::Tcp),
            "http" => Ok(ProtocolType::Http),
            _ => Err(format!("Invalid protocol type: {}", s)),
        }
    }
}

impl sqlx::Type<sqlx::Sqlite> for ProtocolType {
    fn type_info() -> sqlx::sqlite::SqliteTypeInfo {
        <String as sqlx::Type<sqlx::Sqlite>>::type_info()
    }
}

impl<'q> sqlx::Encode<'q, sqlx::Sqlite> for ProtocolType {
    fn encode_by_ref(&self, args: &mut Vec<sqlx::sqlite::SqliteArgumentValue<'q>>) -> Result<sqlx::encode::IsNull, Box<dyn std::error::Error + Send + Sync>> {
        args.push(sqlx::sqlite::SqliteArgumentValue::Text(
            std::borrow::Cow::Owned(self.to_string()),
        ));
        Ok(sqlx::encode::IsNull::No)
    }
}

impl<'r> sqlx::Decode<'r, sqlx::Sqlite> for ProtocolType {
    fn decode(value: sqlx::sqlite::SqliteValueRef<'r>) -> Result<Self, sqlx::error::BoxDynError> {
        let s: String = <String as sqlx::Decode<sqlx::Sqlite>>::decode(value)?;
        s.parse().map_err(|e: String| Box::new(std::io::Error::new(std::io::ErrorKind::InvalidData, e)) as Box<dyn std::error::Error + Send + Sync>)
    }
}

impl From<String> for ProtocolType {
    fn from(s: String) -> Self {
        s.parse().unwrap_or(ProtocolType::Tcp)
    }
}

// Container model
#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Container {
    pub id: i64,
    pub name: String,
    pub container_id: Option<String>,
    pub port: i32,  // Host port (외부 접속 포트)
    pub container_port: Option<i32>,  // Container port (컨테이너 내부 포트)
    pub image: String,
    pub env_vars: Option<String>,  // JSON string
    pub command: Option<String>,
    pub persist_data: i64,  // 0 or 1 (boolean)
    #[sqlx(try_from = "String")]
    pub protocol_type: ProtocolType,  // tcp or http
    #[sqlx(try_from = "String")]
    pub status: ContainerStatus,
    pub created_at: String,
    pub updated_at: String,
}

// Create container request
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateContainer {
    pub name: String,
    pub image: String,
    pub container_port: i32,  // 컨테이너 내부 포트 (필수)
    pub env_vars: Option<String>,  // JSON string
    pub command: Option<String>,
    pub persist_data: bool,  // 데이터 영구 저장 여부
    #[serde(default)]
    pub protocol_type: ProtocolType,  // tcp or http (기본값: tcp)
}
