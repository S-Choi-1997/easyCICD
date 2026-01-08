use anyhow::Result;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};

use crate::build::{BuildExecutor, Deployer};
use crate::docker::DockerClient;
use crate::state::AppState;

pub async fn run_build_worker(state: AppState) -> Result<()> {
    info!("Build worker started");

    let docker = DockerClient::new_with_host_path_detection().await?;
    let executor = BuildExecutor::new(state.clone(), docker.clone());
    let deployer = Deployer::new(state.clone(), docker);

    loop {
        // Get all queued builds
        let queued_builds = state.build_queue.get_all_queued_builds().await;

        for (project_id, _build_ids) in queued_builds {
            // Skip if already processing this project
            if state.build_queue.is_processing(project_id).await {
                continue;
            }

            // Dequeue next build for this project
            if let Some(build_id) = state.build_queue.dequeue(project_id).await {
                info!("Processing build #{} for project {}", build_id, project_id);

                // Mark as processing
                state.build_queue.start_processing(project_id, build_id).await;

                // Spawn task to handle build
                let state_clone = state.clone();
                let executor_clone = executor.clone();
                let deployer_clone = deployer.clone();

                tokio::spawn(async move {
                    if let Err(e) = process_build(
                        state_clone.clone(),
                        executor_clone,
                        deployer_clone,
                        project_id,
                        build_id,
                    )
                    .await
                    {
                        error!("Build #{} failed: {}", build_id, e);
                    }

                    // Mark as finished
                    state_clone.build_queue.finish_processing(project_id).await;
                });
            }
        }

        // Sleep for a bit before checking again
        sleep(Duration::from_secs(1)).await;
    }
}

async fn process_build(
    state: AppState,
    executor: BuildExecutor,
    deployer: Deployer,
    project_id: i64,
    build_id: i64,
) -> Result<()> {
    // Fetch project and build from database
    let project = state
        .db
        .get_project(project_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Project not found: {}", project_id))?;

    let build = state
        .db
        .get_build(build_id)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Build not found: {}", build_id))?;

    info!(
        "Starting build #{} for project '{}'",
        build.build_number, project.name
    );

    // Execute build
    let output_path = executor.execute_build(&project, &build).await?;

    // Deploy
    deployer.deploy(&project, &build, output_path).await?;

    info!(
        "Build #{} for project '{}' completed successfully",
        build.build_number, project.name
    );

    Ok(())
}

impl Clone for BuildExecutor {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            docker: self.docker.clone(),
        }
    }
}

impl Clone for Deployer {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            docker: self.docker.clone(),
        }
    }
}
