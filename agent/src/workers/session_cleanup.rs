use anyhow::Result;
use std::sync::Arc;
use tokio::time::{interval, Duration};
use tracing::{info, warn};

use crate::infrastructure::database::SqliteSessionRepository;
use crate::application::ports::repositories::SessionRepository;

/// Run session cleanup worker
/// Removes expired sessions every hour
pub async fn run_session_cleanup(session_repo: Arc<SqliteSessionRepository>) -> Result<()> {
    let mut cleanup_interval = interval(Duration::from_secs(3600)); // 1 hour

    info!("Session cleanup worker started");

    loop {
        cleanup_interval.tick().await;

        match session_repo.delete_expired().await {
            Ok(count) => {
                if count > 0 {
                    info!("Cleaned up {} expired sessions", count);
                }
            }
            Err(e) => {
                warn!("Failed to cleanup expired sessions: {}", e);
            }
        }
    }
}
