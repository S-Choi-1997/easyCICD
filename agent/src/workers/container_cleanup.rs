use anyhow::Result;
use std::collections::HashSet;
use tokio::time::{interval, Duration};
use tracing::{info, warn};

use crate::state::AppContext;
use crate::docker::DockerClient;
use crate::application::ports::repositories::{ProjectRepository, ContainerRepository};

/// Container cleanup worker
///
/// Runs periodically to clean up orphaned and stale containers:
/// - Build containers (build-*) that have exited
/// - Project containers (project-*-blue/green) without matching DB entries
/// - Standalone containers (container-*) without matching DB entries
/// - Stopped containers that are no longer needed
pub async fn run_container_cleanup(context: AppContext) -> Result<()> {
    info!("Starting container cleanup worker (runs every 30 minutes)");

    // Run every 30 minutes
    let mut ticker = interval(Duration::from_secs(30 * 60));

    loop {
        ticker.tick().await;

        info!("🧹 Running periodic container cleanup...");

        if let Err(e) = cleanup_containers(&context).await {
            warn!("Container cleanup failed: {}", e);
        } else {
            info!("✅ Container cleanup completed successfully");
        }
    }
}

/// Perform container cleanup
async fn cleanup_containers(context: &AppContext) -> Result<()> {
    let docker = &context.docker;

    // Get all containers (including stopped ones)
    #[allow(deprecated)]
    let all_containers = docker.docker_api()
        .list_containers(Some(bollard::container::ListContainersOptions::<String> {
            all: true,
            ..Default::default()
        }))
        .await?;

    // Get valid project IDs from database
    let projects = context.project_repo.list().await?;
    let valid_project_ids: HashSet<i64> = projects.iter().map(|p| p.id).collect();

    // Get valid container names from database
    let db_containers = context.container_repo.list().await?;
    let valid_container_names: HashSet<String> = db_containers.iter()
        .map(|c| c.name.clone())
        .collect();

    let mut cleaned_count = 0;

    for container in all_containers {
        let container_id = match &container.id {
            Some(id) => id,
            None => continue,
        };

        let container_names = match &container.names {
            Some(names) => names,
            None => continue,
        };

        for name in container_names {
            let name = name.trim_start_matches('/');

            // Get container state - ONLY clean up stopped containers
            let state = container.state.as_ref().map(|s| s.as_ref()).unwrap_or("");
            let is_stopped = state == "exited" || state == "dead" || state == "created";

            // SAFETY: Skip running containers - we never touch running containers
            if !is_stopped {
                continue;
            }

            // 1. Clean up build containers (build-*)
            // Build containers should be removed immediately after build completes
            // If we find any stopped build containers here, they're orphaned
            if name.starts_with("build-") {
                info!("🧹 Removing stopped build container: {} (state: {})", name, state);
                docker.remove_container(container_id).await.ok();
                cleaned_count += 1;
            }

            // 2. Clean up orphaned project containers (project-{id}-{slot})
            // Only remove if project doesn't exist in DB AND container is stopped
            else if let Some(rest) = name.strip_prefix("project-") {
                let parts: Vec<&str> = rest.split('-').collect();
                if parts.len() == 2 {
                    if let Ok(project_id) = parts[0].parse::<i64>() {
                        let slot = parts[1];
                        if (slot == "blue" || slot == "green") && !valid_project_ids.contains(&project_id) {
                            info!("🧹 Removing stopped orphaned project container: {} (project {} not in DB, state: {})",
                                name, project_id, state);
                            docker.remove_container(container_id).await.ok();
                            cleaned_count += 1;
                        }
                    }
                }
            }

            // 3. Clean up orphaned standalone containers (container-{name})
            // Only remove if container name not in DB AND container is stopped
            else if let Some(rest) = name.strip_prefix("container-") {
                if !valid_container_names.contains(rest) {
                    info!("🧹 Removing stopped orphaned standalone container: {} (name '{}' not in DB, state: {})",
                        name, rest, state);
                    docker.remove_container(container_id).await.ok();
                    cleaned_count += 1;
                }
            }
        }
    }

    if cleaned_count > 0 {
        info!("🧹 Cleaned up {} orphaned container(s)", cleaned_count);
    } else {
        info!("✨ No orphaned containers found");
    }

    Ok(())
}
