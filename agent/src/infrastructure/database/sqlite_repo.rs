use async_trait::async_trait;
use anyhow::Result;
use sqlx::SqlitePool;

use crate::application::ports::repositories::*;
use crate::db::models::*;

/// SQLite implementation of ProjectRepository
#[derive(Clone)]
pub struct SqliteProjectRepository {
    pool: SqlitePool,
}

impl SqliteProjectRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ProjectRepository for SqliteProjectRepository {
    async fn create(&self, project: CreateProject) -> Result<Project> {
        // Find next available ports
        let max_port: Option<i32> = sqlx::query_scalar(
            "SELECT MAX(green_port) FROM projects"
        )
        .fetch_optional(&self.pool)
        .await?
        .flatten();

        let base_port = match max_port {
            Some(port) => port + 1,
            None => 10002,
        };

        let blue_port = base_port;
        let green_port = base_port + 1;

        let result = sqlx::query(
            r#"
            INSERT INTO projects (
                name, repo, path_filter, branch,
                build_image, build_command, cache_type, working_directory, build_env_vars,
                runtime_image, runtime_command, health_check_url, runtime_port, runtime_env_vars,
                blue_port, green_port, active_slot
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'Blue')
            "#
        )
        .bind(&project.name)
        .bind(&project.repo)
        .bind(&project.path_filter)
        .bind(&project.branch)
        .bind(&project.build_image)
        .bind(&project.build_command)
        .bind(&project.cache_type)
        .bind(&project.working_directory)
        .bind(&project.build_env_vars)
        .bind(&project.runtime_image)
        .bind(&project.runtime_command)
        .bind(&project.health_check_url)
        .bind(&project.runtime_port)
        .bind(&project.runtime_env_vars)
        .bind(blue_port)
        .bind(green_port)
        .execute(&self.pool)
        .await?;

        let id = result.last_insert_rowid();
        let project = sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;

        Ok(project)
    }

    async fn get(&self, id: i64) -> Result<Option<Project>> {
        let project = sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(project)
    }

    async fn get_by_name(&self, name: &str) -> Result<Option<Project>> {
        let project = sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;
        Ok(project)
    }

    async fn list(&self) -> Result<Vec<Project>> {
        let projects = sqlx::query_as::<_, Project>("SELECT * FROM projects ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await?;
        Ok(projects)
    }

    async fn update(&self, id: i64, update: UpdateProject) -> Result<Project> {
        // Get current project
        let current = self.get(id).await?
            .ok_or_else(|| anyhow::anyhow!("Project not found"))?;

        // Merge with update values (use existing value if update is None)
        let name = update.name.unwrap_or(current.name);
        let repo = update.repo.unwrap_or(current.repo);
        let path_filter = update.path_filter.unwrap_or(current.path_filter);
        let branch = update.branch.unwrap_or(current.branch);
        let build_image = update.build_image.unwrap_or(current.build_image);
        let build_command = update.build_command.unwrap_or(current.build_command);
        let cache_type = update.cache_type.unwrap_or(current.cache_type);
        let working_directory = update.working_directory.or(current.working_directory);
        let build_env_vars = update.build_env_vars.or(current.build_env_vars);
        let runtime_image = update.runtime_image.unwrap_or(current.runtime_image);
        let runtime_command = update.runtime_command.unwrap_or(current.runtime_command);
        let health_check_url = update.health_check_url.unwrap_or(current.health_check_url);
        let runtime_port = update.runtime_port.unwrap_or(current.runtime_port);
        let runtime_env_vars = update.runtime_env_vars.or(current.runtime_env_vars);

        sqlx::query(
            r#"
            UPDATE projects SET
                name = ?,
                repo = ?,
                path_filter = ?,
                branch = ?,
                build_image = ?,
                build_command = ?,
                cache_type = ?,
                working_directory = ?,
                build_env_vars = ?,
                runtime_image = ?,
                runtime_command = ?,
                health_check_url = ?,
                runtime_port = ?,
                runtime_env_vars = ?,
                updated_at = datetime('now')
            WHERE id = ?
            "#
        )
        .bind(&name)
        .bind(&repo)
        .bind(&path_filter)
        .bind(&branch)
        .bind(&build_image)
        .bind(&build_command)
        .bind(&cache_type)
        .bind(&working_directory)
        .bind(&build_env_vars)
        .bind(&runtime_image)
        .bind(&runtime_command)
        .bind(&health_check_url)
        .bind(runtime_port)
        .bind(&runtime_env_vars)
        .bind(id)
        .execute(&self.pool)
        .await?;

        // Return updated project
        self.get(id).await?.ok_or_else(|| anyhow::anyhow!("Project not found after update"))
    }

    async fn update_active_slot(&self, id: i64, slot: Slot) -> Result<()> {
        sqlx::query("UPDATE projects SET active_slot = ? WHERE id = ?")
            .bind(slot.to_string())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn update_blue_container(&self, id: i64, container_id: Option<String>) -> Result<()> {
        sqlx::query("UPDATE projects SET blue_container_id = ? WHERE id = ?")
            .bind(container_id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn update_green_container(&self, id: i64, container_id: Option<String>) -> Result<()> {
        sqlx::query("UPDATE projects SET green_container_id = ? WHERE id = ?")
            .bind(container_id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn update_webhook_id(&self, id: i64, webhook_id: Option<i64>) -> Result<()> {
        sqlx::query("UPDATE projects SET github_webhook_id = ? WHERE id = ?")
            .bind(webhook_id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// SQLite implementation of BuildRepository
#[derive(Clone)]
pub struct SqliteBuildRepository {
    pool: SqlitePool,
}

impl SqliteBuildRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl BuildRepository for SqliteBuildRepository {
    async fn create(&self, build: CreateBuild) -> Result<Build> {
        // Get next build number
        let build_number: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(build_number), 0) + 1 FROM builds WHERE project_id = ?"
        )
        .bind(build.project_id)
        .fetch_one(&self.pool)
        .await?;

        let log_path = format!("/data/easycicd/logs/{}/{}.log", build.project_id, build_number);
        let deploy_log_path = format!("/data/easycicd/logs/{}/{}_deploy.log", build.project_id, build_number);
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();

        let result = sqlx::query(
            r#"
            INSERT INTO builds (
                project_id, build_number, commit_hash, commit_message, author,
                status, log_path, deploy_log_path, started_at
            ) VALUES (?, ?, ?, ?, ?, 'Queued', ?, ?, ?)
            "#
        )
        .bind(build.project_id)
        .bind(build_number)
        .bind(&build.commit_hash)
        .bind(&build.commit_message)
        .bind(&build.author)
        .bind(&log_path)
        .bind(&deploy_log_path)
        .bind(&now)
        .execute(&self.pool)
        .await?;

        let id = result.last_insert_rowid();
        let build = sqlx::query_as::<_, Build>("SELECT * FROM builds WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;

        Ok(build)
    }

    async fn get(&self, id: i64) -> Result<Option<Build>> {
        let build = sqlx::query_as::<_, Build>("SELECT * FROM builds WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(build)
    }

    async fn list(&self, limit: i64) -> Result<Vec<Build>> {
        let builds = sqlx::query_as::<_, Build>(
            "SELECT * FROM builds ORDER BY started_at DESC LIMIT ?"
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(builds)
    }

    async fn list_by_project(&self, project_id: i64, limit: i64) -> Result<Vec<Build>> {
        let builds = sqlx::query_as::<_, Build>(
            "SELECT * FROM builds WHERE project_id = ? ORDER BY started_at DESC LIMIT ?"
        )
        .bind(project_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(builds)
    }

    async fn get_latest_by_project(&self, project_id: i64) -> Result<Option<Build>> {
        let build = sqlx::query_as::<_, Build>(
            "SELECT * FROM builds WHERE project_id = ? ORDER BY started_at DESC LIMIT 1"
        )
        .bind(project_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(build)
    }

    async fn list_recent(&self, limit: i64) -> Result<Vec<Build>> {
        let builds = sqlx::query_as::<_, Build>(
            "SELECT * FROM builds ORDER BY started_at DESC LIMIT ?"
        )
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(builds)
    }

    async fn update_status(&self, id: i64, status: BuildStatus) -> Result<()> {
        sqlx::query("UPDATE builds SET status = ? WHERE id = ?")
            .bind(status.to_string())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn finish(&self, id: i64, status: BuildStatus) -> Result<()> {
        let now = chrono::Local::now().format("%Y-%m-%d %H:%M:%S").to_string();
        sqlx::query(
            "UPDATE builds SET status = ?, finished_at = ? WHERE id = ?"
        )
        .bind(status.to_string())
        .bind(&now)
        .bind(id)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn update_deployed_slot(&self, id: i64, slot: Option<String>) -> Result<()> {
        sqlx::query("UPDATE builds SET deployed_slot = ? WHERE id = ?")
            .bind(slot)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn update_deploy_log_path(&self, id: i64, path: String) -> Result<()> {
        sqlx::query("UPDATE builds SET deploy_log_path = ? WHERE id = ?")
            .bind(path)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// SQLite implementation of SettingsRepository
#[derive(Clone)]
pub struct SqliteSettingsRepository {
    pool: SqlitePool,
}

impl SqliteSettingsRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SettingsRepository for SqliteSettingsRepository {
    async fn get(&self, key: &str) -> Result<Option<String>> {
        let result: Option<(String,)> = sqlx::query_as(
            "SELECT value FROM settings WHERE key = ?"
        )
        .bind(key)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|(value,)| value).filter(|v| !v.is_empty()))
    }

    async fn set(&self, key: &str, value: &str) -> Result<()> {
        sqlx::query(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES (?, ?, datetime('now'))"
        )
        .bind(key)
        .bind(value)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    async fn delete(&self, key: &str) -> Result<()> {
        sqlx::query("DELETE FROM settings WHERE key = ?")
            .bind(key)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// SQLite implementation of ContainerRepository
#[derive(Clone)]
pub struct SqliteContainerRepository {
    pool: SqlitePool,
}

impl SqliteContainerRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ContainerRepository for SqliteContainerRepository {
    async fn create(&self, container: CreateContainer) -> Result<Container> {
        // Allocate port
        let port = self.allocate_port().await?;

        let persist_data_i64 = if container.persist_data { 1 } else { 0 };

        let result = sqlx::query(
            r#"
            INSERT INTO containers (name, port, container_port, image, env_vars, command, persist_data, protocol_type, status)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, 'stopped')
            "#
        )
        .bind(&container.name)
        .bind(port)
        .bind(container.container_port)
        .bind(&container.image)
        .bind(&container.env_vars)
        .bind(&container.command)
        .bind(persist_data_i64)
        .bind(container.protocol_type.to_string())
        .execute(&self.pool)
        .await?;

        let id = result.last_insert_rowid();
        let container = sqlx::query_as::<_, Container>("SELECT * FROM containers WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await?;

        Ok(container)
    }

    async fn get(&self, id: i64) -> Result<Option<Container>> {
        let container = sqlx::query_as::<_, Container>("SELECT * FROM containers WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(container)
    }

    async fn get_by_name(&self, name: &str) -> Result<Option<Container>> {
        let container = sqlx::query_as::<_, Container>("SELECT * FROM containers WHERE name = ?")
            .bind(name)
            .fetch_optional(&self.pool)
            .await?;
        Ok(container)
    }

    async fn list(&self) -> Result<Vec<Container>> {
        let containers = sqlx::query_as::<_, Container>(
            "SELECT * FROM containers ORDER BY created_at DESC"
        )
        .fetch_all(&self.pool)
        .await?;
        Ok(containers)
    }

    async fn update_status(&self, id: i64, status: ContainerStatus) -> Result<()> {
        sqlx::query("UPDATE containers SET status = ? WHERE id = ?")
            .bind(status.to_string())
            .bind(id)
            .execute(&self.pool)
            .await?;

        // Update port_allocations container_status
        sqlx::query(
            "UPDATE port_allocations SET container_status = ? WHERE owner_type = 'container' AND owner_id = ?"
        )
        .bind(status.to_string())
        .bind(id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    async fn update_container_id(&self, id: i64, container_id: Option<String>) -> Result<()> {
        sqlx::query("UPDATE containers SET container_id = ? WHERE id = ?")
            .bind(container_id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete(&self, id: i64) -> Result<()> {
        // Get port before deleting
        let container = self.get(id).await?;

        if let Some(c) = container {
            // Delete container
            sqlx::query("DELETE FROM containers WHERE id = ?")
                .bind(id)
                .execute(&self.pool)
                .await?;

            // Release port
            self.release_port(c.port).await?;
        }

        Ok(())
    }

    async fn allocate_port(&self) -> Result<i32> {
        let now = chrono::Local::now().to_rfc3339();

        // Get all unavailable ports (allocated or used by system)
        let unavailable_ports: Vec<i32> = sqlx::query_scalar(
            r#"
            SELECT port FROM port_allocations
            WHERE port_type = 'container'
            AND status IN ('allocated', 'used_by_system')
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        // Find first available port in range 15000-19999
        for port in 15000..20000 {
            if !unavailable_ports.contains(&port) {
                // Allocate port
                sqlx::query(
                    r#"
                    INSERT INTO port_allocations
                    (port, port_type, status, owner_type, last_checked_at)
                    VALUES (?, 'container', 'allocated', 'container', ?)
                    "#
                )
                .bind(port)
                .bind(&now)
                .execute(&self.pool)
                .await?;

                return Ok(port);
            }
        }

        anyhow::bail!("No available ports in Container range (15000-19999)")
    }

    async fn release_port(&self, port: i32) -> Result<()> {
        sqlx::query(
            "DELETE FROM port_allocations WHERE port = ? AND owner_type = 'container'"
        )
        .bind(port)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}

// ============================================================================
// Authentication Repositories
// ============================================================================

/// SQLite implementation of UserRepository
#[derive(Clone)]
pub struct SqliteUserRepository {
    pool: SqlitePool,
}

impl SqliteUserRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for SqliteUserRepository {
    async fn upsert(&self, user: CreateUser) -> Result<User> {
        // Insert or update based on google_id
        sqlx::query(
            r#"
            INSERT INTO users (google_id, email, name, picture)
            VALUES (?, ?, ?, ?)
            ON CONFLICT(google_id) DO UPDATE SET
                email = excluded.email,
                name = excluded.name,
                picture = excluded.picture,
                updated_at = datetime('now')
            "#
        )
        .bind(&user.google_id)
        .bind(&user.email)
        .bind(&user.name)
        .bind(&user.picture)
        .execute(&self.pool)
        .await?;

        self.get_by_google_id(&user.google_id).await?
            .ok_or_else(|| anyhow::anyhow!("User not found after upsert"))
    }

    async fn get(&self, id: i64) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(user)
    }

    async fn get_by_google_id(&self, google_id: &str) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE google_id = ?")
            .bind(google_id)
            .fetch_optional(&self.pool)
            .await?;
        Ok(user)
    }

    async fn get_by_email(&self, email: &str) -> Result<Option<User>> {
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE email = ?")
            .bind(email)
            .fetch_optional(&self.pool)
            .await?;
        Ok(user)
    }
}

/// SQLite implementation of SessionRepository
#[derive(Clone)]
pub struct SqliteSessionRepository {
    pool: SqlitePool,
}

impl SqliteSessionRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl SessionRepository for SqliteSessionRepository {
    async fn create(&self, session: CreateSession) -> Result<Session> {
        sqlx::query(
            "INSERT INTO sessions (id, user_id, expires_at) VALUES (?, ?, ?)"
        )
        .bind(&session.id)
        .bind(session.user_id)
        .bind(&session.expires_at)
        .execute(&self.pool)
        .await?;

        self.get(&session.id).await?
            .ok_or_else(|| anyhow::anyhow!("Session not found after create"))
    }

    async fn get(&self, id: &str) -> Result<Option<Session>> {
        let session = sqlx::query_as::<_, Session>(
            "SELECT * FROM sessions WHERE id = ? AND expires_at > datetime('now')"
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(session)
    }

    async fn get_with_user(&self, id: &str) -> Result<Option<(Session, User)>> {
        // First get the session
        let session = match self.get(id).await? {
            Some(s) => s,
            None => return Ok(None),
        };

        // Then get the user
        let user = sqlx::query_as::<_, User>("SELECT * FROM users WHERE id = ?")
            .bind(session.user_id)
            .fetch_optional(&self.pool)
            .await?;

        match user {
            Some(u) => Ok(Some((session, u))),
            None => Ok(None),
        }
    }

    async fn delete(&self, id: &str) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn delete_expired(&self) -> Result<u64> {
        let result = sqlx::query("DELETE FROM sessions WHERE expires_at <= datetime('now')")
            .execute(&self.pool)
            .await?;
        Ok(result.rows_affected())
    }

    async fn delete_by_user(&self, user_id: i64) -> Result<()> {
        sqlx::query("DELETE FROM sessions WHERE user_id = ?")
            .bind(user_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
