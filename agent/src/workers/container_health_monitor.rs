use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tracing::{info, error};

use crate::state::AppContext;
use crate::events::Event;
use crate::db::models::Slot;
use crate::application::events::event_bus::EventBus;
use crate::application::ports::repositories::ProjectRepository;

/// Container health monitoring worker
///
/// Responsibilities:
/// - Poll Docker every 10 seconds to check container status
/// - Detect container crashes without API calls
/// - Emit ContainerStatus events when state changes
/// - Track previous state to avoid duplicate events
/// - Monitor both Blue and Green slots for all projects
pub async fn run_container_health_monitor(context: AppContext) -> Result<()> {
    info!("Container health monitor worker started");

    // Track previous state: (project_id, slot) -> is_running
    let previous_state: Arc<RwLock<HashMap<(i64, Slot), bool>>> = Arc::new(RwLock::new(HashMap::new()));

    loop {
        // Get all projects from database
        let projects = match context.project_repo.list().await {
            Ok(projects) => projects,
            Err(e) => {
                error!("Failed to list projects: {}", e);
                sleep(Duration::from_secs(10)).await;
                continue;
            }
        };

        for project in projects {
            // Check both Blue and Green slots
            for slot in [Slot::Blue, Slot::Green] {
                let container_id = match slot {
                    Slot::Blue => &project.blue_container_id,
                    Slot::Green => &project.green_container_id,
                };

                // If no container ID, it means no container exists for this slot
                let is_running = if let Some(cid) = container_id {
                    // Check if container is running via DockerClient method
                    context.docker.is_container_running(cid).await
                } else {
                    false
                };

                // Get previous state for this project+slot
                let key = (project.id, slot);
                let mut state_map = previous_state.write().await;
                let previous = state_map.get(&key).copied();

                // Emit event only if state has changed
                if previous != Some(is_running) {
                    let status = if is_running { "running" } else { "stopped" };
                    let container_name = format!("project-{}-{}", project.id, slot.to_string().to_lowercase());

                    info!(
                        "[Project:{}] Container {} slot status changed: {} -> {}",
                        project.name,
                        slot,
                        if previous.unwrap_or(false) { "running" } else { "stopped" },
                        status
                    );

                    // Emit ContainerStatus event
                    let event = Event::container_status(
                        project.id,
                        container_name,
                        slot,
                        status.to_string(),
                    );

                    context.event_bus.emit(event).await;

                    // Update state
                    state_map.insert(key, is_running);
                }
            }
        }

        // Check again in 10 seconds
        sleep(Duration::from_secs(10)).await;
    }
}
