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
                build_image, build_command, cache_type, working_directory,
                runtime_image, runtime_command, health_check_url, runtime_port,
                blue_port, green_port, active_slot
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'Blue')
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
        .bind(&project.runtime_image)
        .bind(&project.runtime_command)
        .bind(&project.health_check_url)
        .bind(&project.runtime_port)
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

    async fn list(&self) -> Result<Vec<Project>> {
        let projects = sqlx::query_as::<_, Project>("SELECT * FROM projects ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await?;
        Ok(projects)
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

        let result = sqlx::query(
            r#"
            INSERT INTO builds (
                project_id, build_number, commit_hash, commit_message, author,
                status, log_path, deploy_log_path
            ) VALUES (?, ?, ?, ?, ?, 'Queued', ?, ?)
            "#
        )
        .bind(build.project_id)
        .bind(build_number)
        .bind(&build.commit_hash)
        .bind(&build.commit_message)
        .bind(&build.author)
        .bind(&log_path)
        .bind(&deploy_log_path)
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

    async fn update_status(&self, id: i64, status: BuildStatus) -> Result<()> {
        sqlx::query("UPDATE builds SET status = ? WHERE id = ?")
            .bind(status.to_string())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    async fn finish(&self, id: i64, status: BuildStatus) -> Result<()> {
        sqlx::query(
            "UPDATE builds SET status = ?, finished_at = datetime('now') WHERE id = ?"
        )
        .bind(status.to_string())
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
