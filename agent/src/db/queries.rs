use sqlx::{SqlitePool, sqlite::SqliteConnectOptions};
use std::str::FromStr;
use crate::db::models::*;

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let options = SqliteConnectOptions::from_str(database_url)?
            .create_if_missing(true);
        let pool = SqlitePool::connect_with(options).await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<(), sqlx::Error> {
        // Create schema_migrations table if not exists
        sqlx::raw_sql(
            "CREATE TABLE IF NOT EXISTS schema_migrations (
                version INTEGER PRIMARY KEY,
                applied_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
            )"
        )
        .execute(&self.pool)
        .await?;

        // Helper function to check if migration was applied
        let is_applied = |version: i32| async move {
            let result: Option<(i32,)> = sqlx::query_as(
                "SELECT version FROM schema_migrations WHERE version = ?"
            )
            .bind(version)
            .fetch_optional(&self.pool)
            .await?;
            Ok::<bool, sqlx::Error>(result.is_some())
        };

        // Helper function to mark migration as applied
        let mark_applied = |version: i32| async move {
            sqlx::query("INSERT INTO schema_migrations (version) VALUES (?)")
                .bind(version)
                .execute(&self.pool)
                .await?;
            Ok::<(), sqlx::Error>(())
        };

        // Migration 1: Initial schema
        if !is_applied(1).await? {
            let migration1 = include_str!("../../migrations/001_initial.sql");
            sqlx::raw_sql(migration1).execute(&self.pool).await?;
            mark_applied(1).await?;
        }

        // Migration 2: Settings table
        if !is_applied(2).await? {
            let migration2 = include_str!("../../migrations/002_settings.sql");
            sqlx::raw_sql(migration2).execute(&self.pool).await?;
            mark_applied(2).await?;
        }

        // Migration 3: GitHub PAT
        if !is_applied(3).await? {
            let migration3 = include_str!("../../migrations/003_github_pat.sql");
            sqlx::raw_sql(migration3).execute(&self.pool).await?;
            mark_applied(3).await?;
        }

        // Migration 4: Working directory
        if !is_applied(4).await? {
            let migration4 = include_str!("../../migrations/004_working_directory.sql");
            sqlx::raw_sql(migration4).execute(&self.pool).await?;
            mark_applied(4).await?;
        }

        Ok(())
    }

    // GitHub PAT operations (stored in settings)
    pub async fn get_github_pat(&self) -> Result<Option<String>, sqlx::Error> {
        let result: Option<(String,)> = sqlx::query_as(
            "SELECT value FROM settings WHERE key = 'github_pat'"
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|(value,)| value).filter(|v| !v.is_empty()))
    }

    pub async fn set_github_pat(&self, pat: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES ('github_pat', ?, datetime('now'))"
        )
        .bind(pat)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // Domain operations (stored in settings)
    pub async fn get_domain(&self) -> Result<Option<String>, sqlx::Error> {
        let result: Option<(String,)> = sqlx::query_as(
            "SELECT value FROM settings WHERE key = 'domain'"
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|(value,)| value).filter(|v| !v.is_empty()))
    }

    pub async fn set_domain(&self, domain: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT OR REPLACE INTO settings (key, value, updated_at) VALUES ('domain', ?, datetime('now'))"
        )
        .bind(domain)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    // Project operations
    pub async fn create_project(&self, project: CreateProject) -> Result<Project, sqlx::Error> {
        // Find next available ports by checking existing projects
        // Start from 10002 (10000 is API server, 10001 reserved)
        let max_port: Option<i32> = sqlx::query_scalar(
            "SELECT MAX(green_port) FROM projects"
        )
        .fetch_optional(&self.pool)
        .await?
        .flatten(); // Handle NULL result from MAX

        let base_port = match max_port {
            Some(port) => port + 1, // Next port after highest green_port
            None => 10002, // First project starts at 10002/10003
        };

        let blue_port = base_port;
        let green_port = base_port + 1;

        let result = sqlx::query(
            r#"
            INSERT INTO projects (
                name, repo, path_filter, branch,
                build_image, build_command, cache_type, working_directory,
                runtime_image, runtime_command, health_check_url,
                blue_port, green_port, active_slot
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'Blue')
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
        .bind(blue_port)
        .bind(green_port)
        .execute(&self.pool)
        .await?;

        let id = result.last_insert_rowid();
        self.get_project_by_id(id).await
    }

    pub async fn get_project(&self, id: i64) -> Result<Option<Project>, sqlx::Error> {
        sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn get_project_by_id(&self, id: i64) -> Result<Project, sqlx::Error> {
        sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
    }

    pub async fn get_project_by_name(&self, name: &str) -> Result<Project, sqlx::Error> {
        sqlx::query_as::<_, Project>("SELECT * FROM projects WHERE name = ?")
            .bind(name)
            .fetch_one(&self.pool)
            .await
    }

    pub async fn list_projects(&self) -> Result<Vec<Project>, sqlx::Error> {
        sqlx::query_as::<_, Project>("SELECT * FROM projects ORDER BY created_at DESC")
            .fetch_all(&self.pool)
            .await
    }

    pub async fn delete_project(&self, id: i64) -> Result<(), sqlx::Error> {
        sqlx::query("DELETE FROM projects WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_project_active_slot(&self, id: i64, slot: Slot) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE projects SET active_slot = ? WHERE id = ?")
            .bind(slot.to_string())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_project_container_id(&self, id: i64, slot: Slot, container_id: String) -> Result<(), sqlx::Error> {
        let column = match slot {
            Slot::Blue => "blue_container_id",
            Slot::Green => "green_container_id",
        };

        let query = format!("UPDATE projects SET {} = ? WHERE id = ?", column);
        sqlx::query(&query)
            .bind(container_id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    // Build operations
    pub async fn create_build(&self, build: CreateBuild) -> Result<Build, sqlx::Error> {
        // Get next build number for this project
        let build_number: i64 = sqlx::query_scalar(
            "SELECT COALESCE(MAX(build_number), 0) + 1 FROM builds WHERE project_id = ?"
        )
        .bind(build.project_id)
        .fetch_one(&self.pool)
        .await?;

        // Use project ID for log path (not name, to avoid conflicts and special characters)
        let log_path = format!("/data/easycicd/logs/{}/{}.log", build.project_id, build_number);

        let result = sqlx::query(
            r#"
            INSERT INTO builds (
                project_id, build_number, commit_hash, commit_message, author,
                status, log_path
            ) VALUES (?, ?, ?, ?, ?, 'Queued', ?)
            "#
        )
        .bind(build.project_id)
        .bind(build_number)
        .bind(&build.commit_hash)
        .bind(&build.commit_message)
        .bind(&build.author)
        .bind(&log_path)
        .execute(&self.pool)
        .await?;

        let id = result.last_insert_rowid();
        self.get_build_by_id(id).await
    }

    pub async fn get_build(&self, id: i64) -> Result<Option<Build>, sqlx::Error> {
        sqlx::query_as::<_, Build>("SELECT * FROM builds WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
    }

    pub async fn get_build_by_id(&self, id: i64) -> Result<Build, sqlx::Error> {
        sqlx::query_as::<_, Build>("SELECT * FROM builds WHERE id = ?")
            .bind(id)
            .fetch_one(&self.pool)
            .await
    }

    pub async fn list_builds_by_project(&self, project_id: i64, limit: i64) -> Result<Vec<Build>, sqlx::Error> {
        sqlx::query_as::<_, Build>(
            "SELECT * FROM builds WHERE project_id = ? ORDER BY started_at DESC LIMIT ?"
        )
        .bind(project_id)
        .bind(limit)
        .fetch_all(&self.pool)
        .await
    }

    pub async fn list_recent_builds(&self, limit: i64) -> Result<Vec<Build>, sqlx::Error> {
        sqlx::query_as::<_, Build>("SELECT * FROM builds ORDER BY started_at DESC LIMIT ?")
            .bind(limit)
            .fetch_all(&self.pool)
            .await
    }

    pub async fn update_build_status(&self, id: i64, status: BuildStatus) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE builds SET status = ? WHERE id = ?")
            .bind(status.to_string())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_build_output_path(&self, id: i64, output_path: String) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE builds SET output_path = ? WHERE id = ?")
            .bind(output_path)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_build_deployed_slot(&self, id: i64, slot: Option<String>) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE builds SET deployed_slot = ? WHERE id = ?")
            .bind(slot)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_project_blue_container(&self, id: i64, container_id: Option<String>) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE projects SET blue_container_id = ? WHERE id = ?")
            .bind(container_id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn update_project_green_container(&self, id: i64, container_id: Option<String>) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE projects SET green_container_id = ? WHERE id = ?")
            .bind(container_id)
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn finish_build(&self, id: i64, status: BuildStatus) -> Result<(), sqlx::Error> {
        sqlx::query("UPDATE builds SET status = ?, finished_at = datetime('now') WHERE id = ?")
            .bind(status.to_string())
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_queued_builds(&self) -> Result<Vec<Build>, sqlx::Error> {
        sqlx::query_as::<_, Build>(
            "SELECT * FROM builds WHERE status = 'Queued' ORDER BY started_at ASC"
        )
        .fetch_all(&self.pool)
        .await
    }

    // Settings operations
    pub async fn get_setting(&self, key: &str) -> Result<Option<String>, sqlx::Error> {
        let value: Option<String> = sqlx::query_scalar("SELECT value FROM settings WHERE key = ?")
            .bind(key)
            .fetch_optional(&self.pool)
            .await?;
        Ok(value)
    }

    pub async fn set_setting(&self, key: &str, value: &str) -> Result<(), sqlx::Error> {
        sqlx::query(
            "INSERT INTO settings (key, value) VALUES (?, ?)
             ON CONFLICT(key) DO UPDATE SET value = ?, updated_at = datetime('now')"
        )
        .bind(key)
        .bind(value)
        .bind(value)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
