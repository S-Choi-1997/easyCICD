use serde::{Deserialize, Serialize};
use crate::db::{BuildStatus, Slot};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum Event {
    #[serde(rename = "build_status")]
    BuildStatus {
        build_id: i64,
        project_id: i64,
        status: BuildStatus,
        timestamp: String,
    },

    #[serde(rename = "log")]
    Log {
        build_id: i64,
        line: String,
        line_number: usize,
        timestamp: String,
    },

    #[serde(rename = "deployment")]
    Deployment {
        project_id: i64,
        project_name: String,
        build_id: i64,
        status: String,
        slot: Slot,
        url: String,
        timestamp: String,
    },

    #[serde(rename = "health_check")]
    HealthCheck {
        project_id: i64,
        build_id: i64,
        attempt: usize,
        max_attempts: usize,
        status: String,
        url: String,
        timestamp: String,
    },

    #[serde(rename = "container_status")]
    ContainerStatus {
        project_id: i64,
        container_id: String,
        slot: Slot,
        status: String,
        timestamp: String,
    },

    #[serde(rename = "error")]
    Error {
        build_id: Option<i64>,
        project_id: Option<i64>,
        message: String,
        timestamp: String,
    },
}

impl Event {
    pub fn now() -> String {
        chrono::Utc::now().to_rfc3339()
    }

    pub fn build_status(build_id: i64, project_id: i64, status: BuildStatus) -> Self {
        Event::BuildStatus {
            build_id,
            project_id,
            status,
            timestamp: Self::now(),
        }
    }

    pub fn log(build_id: i64, line: String, line_number: usize) -> Self {
        Event::Log {
            build_id,
            line,
            line_number,
            timestamp: Self::now(),
        }
    }

    pub fn deployment(
        project_id: i64,
        project_name: String,
        build_id: i64,
        status: String,
        slot: Slot,
        url: String,
    ) -> Self {
        Event::Deployment {
            project_id,
            project_name,
            build_id,
            status,
            slot,
            url,
            timestamp: Self::now(),
        }
    }

    pub fn health_check(
        project_id: i64,
        build_id: i64,
        attempt: usize,
        max_attempts: usize,
        status: String,
        url: String,
    ) -> Self {
        Event::HealthCheck {
            project_id,
            build_id,
            attempt,
            max_attempts,
            status,
            url,
            timestamp: Self::now(),
        }
    }

    pub fn container_status(project_id: i64, container_id: String, slot: Slot, status: String) -> Self {
        Event::ContainerStatus {
            project_id,
            container_id,
            slot,
            status,
            timestamp: Self::now(),
        }
    }

    pub fn error(build_id: Option<i64>, project_id: Option<i64>, message: String) -> Self {
        Event::Error {
            build_id,
            project_id,
            message,
            timestamp: Self::now(),
        }
    }
}
