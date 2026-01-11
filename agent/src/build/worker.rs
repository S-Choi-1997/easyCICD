use anyhow::{Context, Result};
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, info};
use uuid::Uuid;

use crate::state::AppContext;
use crate::application::ports::repositories::{ProjectRepository, BuildRepository};

pub async fn run_build_worker(context: AppContext) -> Result<()> {
    info!("Build worker started");

    loop {
        // Get all queued builds
        let queued_builds = context.build_queue.get_all_queued_builds().await;

        for (project_id, _build_ids) in queued_builds {
            // Skip if already processing this project
            if context.build_queue.is_processing(project_id).await {
                continue;
            }

            // Dequeue next build for this project
            if let Some(build_id) = context.build_queue.dequeue(project_id).await {
                info!("Processing build #{} for project {}", build_id, project_id);

                // Mark as processing
                context.build_queue.start_processing(project_id, build_id).await;

                // Spawn task to handle build
                let ctx = context.clone();

                tokio::spawn(async move {
                    // Generate trace ID for this build process
                    let trace_id = format!("worker-{}-{}", project_id, Uuid::new_v4());

                    if let Err(e) = process_build(ctx.clone(), &trace_id, project_id, build_id).await
                    {
                        error!("[{}] Build #{} failed: {}", trace_id, build_id, e);

                        // Update build status to Failed
                        if let Err(update_err) = ctx.build_repo.finish(build_id, crate::db::models::BuildStatus::Failed).await {
                            error!("[{}] Failed to update build status: {}", trace_id, update_err);
                        }
                    }

                    // Mark as finished and add small delay to prevent immediate re-processing
                    ctx.build_queue.finish_processing(project_id).await;

                    // Small delay to ensure state consistency before next build
                    sleep(Duration::from_millis(100)).await;
                });
            }
        }

        // Sleep for a bit before checking again
        sleep(Duration::from_secs(1)).await;
    }
}

async fn process_build(
    ctx: AppContext,
    trace_id: &str,
    project_id: i64,
    build_id: i64,
) -> Result<()> {
    // Fetch project and build from database
    let project_opt: Option<crate::db::models::Project> = ctx
        .project_repo
        .get(project_id)
        .await?;
    let project = project_opt.context(format!("Project not found: {}", project_id))?;

    let build_opt: Option<crate::db::models::Build> = ctx
        .build_repo
        .get(build_id)
        .await?;
    let build = build_opt.context(format!("Build not found: {}", build_id))?;

    info!(
        "[{}] Starting build #{} for project '{}'",
        trace_id, build.build_number, project.name
    );

    // Execute build using BuildService
    let output_path = ctx
        .build_service
        .execute_build(trace_id, build_id)
        .await?;

    info!(
        "[{}] Build completed, starting deployment for project '{}'",
        trace_id, project.name
    );

    // Deploy using DeploymentService
    ctx.deployment_service
        .deploy(trace_id, &project, &build, output_path)
        .await?;

    info!(
        "[{}] Build #{} for project '{}' completed successfully",
        trace_id, build.build_number, project.name
    );

    Ok(())
}
