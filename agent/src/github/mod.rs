pub mod client;
pub mod models;
pub mod detector;
pub mod workflow_parser;
pub mod workflow_interpreter;
pub mod config_builder;

pub use client::GitHubClient;
pub use models::*;
pub use detector::{ProjectDetector, ProjectConfig};
