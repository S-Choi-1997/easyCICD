use anyhow::Result;
use futures::StreamExt;
use std::collections::HashSet;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::time::{sleep, Duration};
use tracing::{info, error, warn};

use crate::state::AppContext;
use crate::events::Event;
use crate::db::models::ContainerStatus;
use crate::application::events::event_bus::EventBus;
use crate::application::ports::repositories::ContainerRepository;

/// Container log streaming worker
///
/// Responsibilities:
/// - Monitor all running standalone containers
/// - Stream logs in real-time for each running container
/// - Emit ContainerLog events to WebSocket clients
/// - Auto-restart streaming when containers start/stop
/// - Track active streams to prevent duplicates
pub async fn run_container_log_streamer(context: AppContext) -> Result<()> {
    info!("Container log streamer worker started");

    // Track which containers are currently being streamed
    let active_streams: Arc<RwLock<HashSet<i64>>> = Arc::new(RwLock::new(HashSet::new()));

    loop {
        // Get all running containers from database
        let containers = match context.container_repo.list().await {
            Ok(containers) => containers,
            Err(e) => {
                error!("Failed to list containers: {}", e);
                sleep(Duration::from_secs(5)).await;
                continue;
            }
        };

        // Spawn log streaming tasks for running containers
        for container in containers {
            if container.status != ContainerStatus::Running {
                // Remove from active streams if it was running before
                active_streams.write().await.remove(&container.id);
                continue;
            }

            // Check if we're already streaming this container
            if active_streams.read().await.contains(&container.id) {
                continue;
            }

            if let Some(docker_id_ref) = &container.container_id {
                let ctx = context.clone();
                let container_id = container.id;
                let container_name = container.name.clone();
                let docker_id = docker_id_ref.clone(); // Clone to owned String for async move
                let streams = active_streams.clone();

                // Mark as active
                streams.write().await.insert(container_id);

                tokio::spawn(async move {
                    info!("[Container:{}] Starting log stream for docker:{}", container_name, docker_id);

                    match ctx.docker.stream_container_logs(&docker_id).await {
                        Ok(mut log_stream) => {
                            while let Some(log_result) = log_stream.next().await {
                                match log_result {
                                    Ok(bytes) => {
                                        // Convert bytes to string
                                        let line = String::from_utf8_lossy(&bytes).to_string();
                                        if !line.is_empty() {
                                            // Emit log event to WebSocket
                                            let event = Event::container_log(
                                                container_id,
                                                container_name.clone(),
                                                line,
                                            );
                                            ctx.event_bus.emit(event).await;
                                        }
                                    }
                                    Err(e) => {
                                        warn!("[Container:{}] Log stream error: {}", container_name, e);
                                        break;
                                    }
                                }
                            }
                        }
                        Err(e) => {
                            error!("[Container:{}] Failed to start log stream: {}", container_name, e);
                        }
                    }

                    info!("[Container:{}] Log stream ended", container_name);

                    // Remove from active streams when done
                    streams.write().await.remove(&container_id);
                });
            }
        }

        // Check for new containers every 5 seconds
        sleep(Duration::from_secs(5)).await;
    }
}
