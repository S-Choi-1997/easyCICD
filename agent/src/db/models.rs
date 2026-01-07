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
    Deploying,
    Success,
    Failed,
}

impl std::fmt::Display for BuildStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildStatus::Queued => write!(f, "Queued"),
            BuildStatus::Building => write!(f, "Building"),
            BuildStatus::Deploying => write!(f, "Deploying"),
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
            "Deploying" => Ok(BuildStatus::Deploying),
            "Success" => Ok(BuildStatus::Success),
            "Failed" => Ok(BuildStatus::Failed),
            _ => Err(format!("Invalid build status: {}", s)),
        }
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

    // Deploy configuration
    pub runtime_image: String,
    pub runtime_command: String,
    pub health_check_url: String,

    // Port allocation
    pub blue_port: i32,
    pub green_port: i32,
    #[sqlx(try_from = "String")]
    pub active_slot: Slot,

    // Container IDs
    pub blue_container_id: Option<String>,
    pub green_container_id: Option<String>,

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

    pub runtime_image: String,
    pub runtime_command: String,
    pub health_check_url: String,
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
