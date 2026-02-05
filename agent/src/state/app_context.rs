use anyhow::Result;
use sqlx::SqlitePool;
use std::sync::Arc;
use tokio::sync::broadcast;

use crate::application::events::{BroadcastEventBus, Event};
use crate::application::events::event_bus::EventBus;
use crate::application::services::{BuildService, ContainerService, DeploymentService, ProjectService};
use crate::docker::DockerClient;
use crate::infrastructure::database::{
    SqliteBuildRepository, SqliteContainerRepository, SqliteProjectRepository, SqliteSettingsRepository,
    SqliteUserRepository, SqliteSessionRepository, SqliteGitHubPatRepository, SqliteDiscordWebhookRepository,
};
use crate::infrastructure::logging::BoundaryLogger;
use crate::state::{BuildQueue, WsConnections};
use crate::auth::OAuthConfig;

/// AppContext - 서비스 기반 DI 컨테이너 (AppState 완전 대체)
///
/// 모든 서비스와 인프라스트럭처 컴포넌트를 조립하여 제공합니다.
/// Trait 기반 의존성 주입을 통해 테스트 가능성을 높이고,
/// 명확한 레이어 분리를 달성합니다.
#[derive(Clone)]
pub struct AppContext {
    // Services (Application Layer)
    pub project_service: Arc<
        ProjectService<
            SqliteProjectRepository,
            SqliteBuildRepository,
            BroadcastEventBus,
        >,
    >,
    pub build_service: Arc<
        BuildService<
            SqliteBuildRepository,
            SqliteProjectRepository,
            SqliteSettingsRepository,
            SqliteGitHubPatRepository,
            BroadcastEventBus,
        >,
    >,
    pub deployment_service: Arc<
        DeploymentService<
            SqliteBuildRepository,
            SqliteProjectRepository,
            BroadcastEventBus,
        >,
    >,
    pub container_service: Arc<
        ContainerService<
            SqliteContainerRepository,
            BroadcastEventBus,
        >,
    >,

    // Repositories (Infrastructure Layer)
    pub project_repo: Arc<SqliteProjectRepository>,
    pub build_repo: Arc<SqliteBuildRepository>,
    pub settings_repo: Arc<SqliteSettingsRepository>,
    pub container_repo: Arc<SqliteContainerRepository>,
    pub user_repo: Arc<SqliteUserRepository>,
    pub session_repo: Arc<SqliteSessionRepository>,
    pub github_pat_repo: Arc<SqliteGitHubPatRepository>,
    pub discord_webhook_repo: Arc<SqliteDiscordWebhookRepository>,

    // Infrastructure
    pub event_bus: BroadcastEventBus,
    pub build_queue: Arc<BuildQueue>,
    pub ws_connections: Arc<WsConnections>,
    pub docker: DockerClient,
    pub logger: Arc<BoundaryLogger>,

    // Config
    pub gateway_ip: String,
    pub base_domain: Option<String>,

    // OAuth config (optional)
    pub oauth_config: Option<OAuthConfig>,
}

impl AppContext {
    /// Create a new AppContext with all dependencies wired up
    pub async fn new(
        pool: SqlitePool,
        docker: DockerClient,
        gateway_ip: String,
        base_domain: Option<String>,
    ) -> Result<Self> {
        // 1. Create Repositories
        let project_repo = Arc::new(SqliteProjectRepository::new(pool.clone()));
        let build_repo = Arc::new(SqliteBuildRepository::new(pool.clone()));
        let settings_repo = Arc::new(SqliteSettingsRepository::new(pool.clone()));
        let container_repo = Arc::new(SqliteContainerRepository::new(pool.clone()));
        let user_repo = Arc::new(SqliteUserRepository::new(pool.clone()));
        let session_repo = Arc::new(SqliteSessionRepository::new(pool.clone()));
        let github_pat_repo = Arc::new(SqliteGitHubPatRepository::new(pool.clone()));
        let discord_webhook_repo = Arc::new(SqliteDiscordWebhookRepository::new(pool.clone()));

        // Load OAuth config (optional - don't fail if not configured)
        let oauth_config = OAuthConfig::from_env().ok();

        // 2. Create Infrastructure components
        let logger = Arc::new(BoundaryLogger::new());
        let event_bus = BroadcastEventBus::new_default(logger.clone());

        // 3. Create Services with dependency injection
        let project_service = Arc::new(ProjectService::<SqliteProjectRepository, SqliteBuildRepository, BroadcastEventBus>::new(
            project_repo.clone(),
            build_repo.clone(),
            event_bus.clone(),
            docker.clone(),
            logger.clone(),
        ));

        let build_service = Arc::new(BuildService::<SqliteBuildRepository, SqliteProjectRepository, SqliteSettingsRepository, SqliteGitHubPatRepository, BroadcastEventBus>::new(
            build_repo.clone(),
            project_repo.clone(),
            settings_repo.clone(),
            github_pat_repo.clone(),
            event_bus.clone(),
            docker.clone(),
            logger.clone(),
        ));

        let deployment_service = Arc::new(DeploymentService::<SqliteBuildRepository, SqliteProjectRepository, BroadcastEventBus>::new(
            build_repo.clone(),
            project_repo.clone(),
            event_bus.clone(),
            docker.clone(),
            logger.clone(),
        ));

        let container_service = Arc::new(ContainerService::<SqliteContainerRepository, BroadcastEventBus>::new(
            container_repo.clone(),
            docker.clone(),
            logger.clone(),
            Arc::new(event_bus.clone()),
        ));

        Ok(Self {
            project_service,
            build_service,
            deployment_service,
            container_service,
            project_repo,
            build_repo,
            settings_repo,
            container_repo,
            user_repo,
            session_repo,
            github_pat_repo,
            discord_webhook_repo,
            event_bus,
            build_queue: Arc::new(BuildQueue::new()),
            ws_connections: Arc::new(WsConnections::new()),
            docker,
            logger,
            gateway_ip,
            base_domain,
            oauth_config,
        })
    }

    /// Subscribe to event bus (compatibility method for existing code)
    pub fn subscribe_events(&self) -> broadcast::Receiver<Event> {
        self.event_bus.subscribe()
    }
}
