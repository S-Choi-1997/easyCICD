pub mod client;
pub mod models;
pub mod detector;

pub use client::GitHubClient;
pub use models::*;
pub use detector::{ProjectDetector, ProjectConfig};
