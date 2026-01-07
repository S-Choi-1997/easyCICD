use sqlx::SqlitePool;
use crate::db::models::*;

#[derive(Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    pub async fn connect(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = SqlitePool::connect(database_url).await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<(), sqlx::Error> {
        let migration1 = include_str!("../../migrations/001_initial.sql");
        sqlx::raw_sql(migration1).execute(&self.pool).await?;

        let migration2 = include_str!("../../migrations/002_settings.sql");
        sqlx::raw_sql(migration2).execute(&self.pool).await?;

        Ok(())
    }

    // Project operations
    pub async fn create_project(&self, project: CreateProject) -> Result<Project, sqlx::Error> {
        // Calculate ports based on next available project ID
        let next_id: i64 = sqlx::query_scalar("SELECT COALESCE(MAX(id), 0) + 1 FROM projects")
            .fetch_one(&self.pool)
            .await?;

        let base_port = 9999;
        let blue_port = base_port + (next_id as i32 * 2);
        let green_port = base_port + (next_id as i32 * 2) + 1;

        let result = sqlx::query(
            r#"
            INSERT INTO projects (
                name, repo, path_filter, branch,
                build_image, build_command, cache_type,
                runtime_image, runtime_command, health_check_url,
                blue_port, green_port, active_slot
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 'Blue')
            "#
        )
        .bind(&project.name)
        .bind(&project.repo)
        .bind(&project.path_filter)
        .bind(&project.branch)
        .bind(&project.build_image)
        .bind(&project.build_command)
        .bind(&project.cache_type)
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

        // Get project to determine log path
        let project = self.get_project_by_id(build.project_id).await?;
        let log_path = format!("/data/logs/{}/{}.log", project.name, build_number);

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
