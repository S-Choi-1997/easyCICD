use anyhow::Result;
use sqlx::{SqlitePool, Row};
use crate::infrastructure::notifications::DiscordWebhookConfig;

#[derive(Clone)]
pub struct SqliteDiscordWebhookRepository {
    pool: SqlitePool,
}

impl SqliteDiscordWebhookRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn list_enabled(&self) -> Result<Vec<DiscordWebhookConfig>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, label, webhook_url, enabled,
                notify_on_build_start, notify_on_build_success, notify_on_build_failure,
                notify_on_deploy_start, notify_on_deploy_success, notify_on_deploy_failure,
                mention_user_ids, mention_role_ids, mention_on_failure_only
            FROM discord_webhooks
            WHERE enabled = 1
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let configs = rows
            .into_iter()
            .map(|row| {
                let mention_user_ids: Vec<String> = row
                    .try_get::<Option<String>, _>("mention_user_ids")
                    .ok()
                    .flatten()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default();

                let mention_role_ids: Vec<String> = row
                    .try_get::<Option<String>, _>("mention_role_ids")
                    .ok()
                    .flatten()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default();

                DiscordWebhookConfig {
                    id: row.try_get("id").unwrap_or(0),
                    label: row.try_get("label").unwrap_or_default(),
                    webhook_url: row.try_get("webhook_url").unwrap_or_default(),
                    enabled: row.try_get::<i32, _>("enabled").unwrap_or(0) != 0,
                    notify_on_build_start: row.try_get::<i32, _>("notify_on_build_start").unwrap_or(0) != 0,
                    notify_on_build_success: row.try_get::<i32, _>("notify_on_build_success").unwrap_or(0) != 0,
                    notify_on_build_failure: row.try_get::<i32, _>("notify_on_build_failure").unwrap_or(0) != 0,
                    notify_on_deploy_start: row.try_get::<i32, _>("notify_on_deploy_start").unwrap_or(0) != 0,
                    notify_on_deploy_success: row.try_get::<i32, _>("notify_on_deploy_success").unwrap_or(0) != 0,
                    notify_on_deploy_failure: row.try_get::<i32, _>("notify_on_deploy_failure").unwrap_or(0) != 0,
                    mention_user_ids,
                    mention_role_ids,
                    mention_on_failure_only: row.try_get::<i32, _>("mention_on_failure_only").unwrap_or(0) != 0,
                }
            })
            .collect();

        Ok(configs)
    }

    pub async fn get(&self, id: i64) -> Result<Option<DiscordWebhookConfig>> {
        let row = sqlx::query(
            r#"
            SELECT
                id, label, webhook_url, enabled,
                notify_on_build_start, notify_on_build_success, notify_on_build_failure,
                notify_on_deploy_start, notify_on_deploy_success, notify_on_deploy_failure,
                mention_user_ids, mention_role_ids, mention_on_failure_only
            FROM discord_webhooks
            WHERE id = ?
            "#
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(row.map(|row| {
            let mention_user_ids: Vec<String> = row
                .try_get::<Option<String>, _>("mention_user_ids")
                .ok()
                .flatten()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();

            let mention_role_ids: Vec<String> = row
                .try_get::<Option<String>, _>("mention_role_ids")
                .ok()
                .flatten()
                .and_then(|s| serde_json::from_str(&s).ok())
                .unwrap_or_default();

            DiscordWebhookConfig {
                id: row.try_get("id").unwrap_or(0),
                label: row.try_get("label").unwrap_or_default(),
                webhook_url: row.try_get("webhook_url").unwrap_or_default(),
                enabled: row.try_get::<i32, _>("enabled").unwrap_or(0) != 0,
                notify_on_build_start: row.try_get::<i32, _>("notify_on_build_start").unwrap_or(0) != 0,
                notify_on_build_success: row.try_get::<i32, _>("notify_on_build_success").unwrap_or(0) != 0,
                notify_on_build_failure: row.try_get::<i32, _>("notify_on_build_failure").unwrap_or(0) != 0,
                notify_on_deploy_start: row.try_get::<i32, _>("notify_on_deploy_start").unwrap_or(0) != 0,
                notify_on_deploy_success: row.try_get::<i32, _>("notify_on_deploy_success").unwrap_or(0) != 0,
                notify_on_deploy_failure: row.try_get::<i32, _>("notify_on_deploy_failure").unwrap_or(0) != 0,
                mention_user_ids,
                mention_role_ids,
                mention_on_failure_only: row.try_get::<i32, _>("mention_on_failure_only").unwrap_or(0) != 0,
            }
        }))
    }

    pub async fn list_all(&self) -> Result<Vec<DiscordWebhookConfig>> {
        let rows = sqlx::query(
            r#"
            SELECT
                id, label, webhook_url, enabled,
                notify_on_build_start, notify_on_build_success, notify_on_build_failure,
                notify_on_deploy_start, notify_on_deploy_success, notify_on_deploy_failure,
                mention_user_ids, mention_role_ids, mention_on_failure_only
            FROM discord_webhooks
            ORDER BY created_at DESC
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        let configs = rows
            .into_iter()
            .map(|row| {
                let mention_user_ids: Vec<String> = row
                    .try_get::<Option<String>, _>("mention_user_ids")
                    .ok()
                    .flatten()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default();

                let mention_role_ids: Vec<String> = row
                    .try_get::<Option<String>, _>("mention_role_ids")
                    .ok()
                    .flatten()
                    .and_then(|s| serde_json::from_str(&s).ok())
                    .unwrap_or_default();

                DiscordWebhookConfig {
                    id: row.try_get("id").unwrap_or(0),
                    label: row.try_get("label").unwrap_or_default(),
                    webhook_url: row.try_get("webhook_url").unwrap_or_default(),
                    enabled: row.try_get::<i32, _>("enabled").unwrap_or(0) != 0,
                    notify_on_build_start: row.try_get::<i32, _>("notify_on_build_start").unwrap_or(0) != 0,
                    notify_on_build_success: row.try_get::<i32, _>("notify_on_build_success").unwrap_or(0) != 0,
                    notify_on_build_failure: row.try_get::<i32, _>("notify_on_build_failure").unwrap_or(0) != 0,
                    notify_on_deploy_start: row.try_get::<i32, _>("notify_on_deploy_start").unwrap_or(0) != 0,
                    notify_on_deploy_success: row.try_get::<i32, _>("notify_on_deploy_success").unwrap_or(0) != 0,
                    notify_on_deploy_failure: row.try_get::<i32, _>("notify_on_deploy_failure").unwrap_or(0) != 0,
                    mention_user_ids,
                    mention_role_ids,
                    mention_on_failure_only: row.try_get::<i32, _>("mention_on_failure_only").unwrap_or(0) != 0,
                }
            })
            .collect();

        Ok(configs)
    }

    pub async fn create(&self, config: CreateDiscordWebhook) -> Result<DiscordWebhookConfig> {
        let mention_user_ids_json = serde_json::to_string(&config.mention_user_ids)?;
        let mention_role_ids_json = serde_json::to_string(&config.mention_role_ids)?;

        let result = sqlx::query(
            r#"
            INSERT INTO discord_webhooks (
                label, webhook_url, enabled,
                notify_on_build_start, notify_on_build_success, notify_on_build_failure,
                notify_on_deploy_start, notify_on_deploy_success, notify_on_deploy_failure,
                mention_user_ids, mention_role_ids, mention_on_failure_only
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            "#
        )
        .bind(&config.label)
        .bind(&config.webhook_url)
        .bind(if config.enabled { 1 } else { 0 })
        .bind(if config.notify_on_build_start { 1 } else { 0 })
        .bind(if config.notify_on_build_success { 1 } else { 0 })
        .bind(if config.notify_on_build_failure { 1 } else { 0 })
        .bind(if config.notify_on_deploy_start { 1 } else { 0 })
        .bind(if config.notify_on_deploy_success { 1 } else { 0 })
        .bind(if config.notify_on_deploy_failure { 1 } else { 0 })
        .bind(&mention_user_ids_json)
        .bind(&mention_role_ids_json)
        .bind(if config.mention_on_failure_only { 1 } else { 0 })
        .execute(&self.pool)
        .await?;

        let id = result.last_insert_rowid();
        self.get(id).await?.ok_or_else(|| anyhow::anyhow!("Failed to retrieve created webhook"))
    }

    pub async fn update(&self, id: i64, config: UpdateDiscordWebhook) -> Result<DiscordWebhookConfig> {
        // Get current config
        let current = self.get(id).await?
            .ok_or_else(|| anyhow::anyhow!("Discord webhook not found"))?;

        // Merge with update values
        let label = config.label.unwrap_or(current.label);
        let webhook_url = config.webhook_url.unwrap_or(current.webhook_url);
        let enabled = config.enabled.unwrap_or(current.enabled);
        let notify_on_build_start = config.notify_on_build_start.unwrap_or(current.notify_on_build_start);
        let notify_on_build_success = config.notify_on_build_success.unwrap_or(current.notify_on_build_success);
        let notify_on_build_failure = config.notify_on_build_failure.unwrap_or(current.notify_on_build_failure);
        let notify_on_deploy_start = config.notify_on_deploy_start.unwrap_or(current.notify_on_deploy_start);
        let notify_on_deploy_success = config.notify_on_deploy_success.unwrap_or(current.notify_on_deploy_success);
        let notify_on_deploy_failure = config.notify_on_deploy_failure.unwrap_or(current.notify_on_deploy_failure);
        let mention_user_ids = config.mention_user_ids.unwrap_or(current.mention_user_ids);
        let mention_role_ids = config.mention_role_ids.unwrap_or(current.mention_role_ids);
        let mention_on_failure_only = config.mention_on_failure_only.unwrap_or(current.mention_on_failure_only);

        let mention_user_ids_json = serde_json::to_string(&mention_user_ids)?;
        let mention_role_ids_json = serde_json::to_string(&mention_role_ids)?;

        sqlx::query(
            r#"
            UPDATE discord_webhooks SET
                label = ?,
                webhook_url = ?,
                enabled = ?,
                notify_on_build_start = ?,
                notify_on_build_success = ?,
                notify_on_build_failure = ?,
                notify_on_deploy_start = ?,
                notify_on_deploy_success = ?,
                notify_on_deploy_failure = ?,
                mention_user_ids = ?,
                mention_role_ids = ?,
                mention_on_failure_only = ?,
                updated_at = datetime('now')
            WHERE id = ?
            "#
        )
        .bind(&label)
        .bind(&webhook_url)
        .bind(if enabled { 1 } else { 0 })
        .bind(if notify_on_build_start { 1 } else { 0 })
        .bind(if notify_on_build_success { 1 } else { 0 })
        .bind(if notify_on_build_failure { 1 } else { 0 })
        .bind(if notify_on_deploy_start { 1 } else { 0 })
        .bind(if notify_on_deploy_success { 1 } else { 0 })
        .bind(if notify_on_deploy_failure { 1 } else { 0 })
        .bind(&mention_user_ids_json)
        .bind(&mention_role_ids_json)
        .bind(if mention_on_failure_only { 1 } else { 0 })
        .bind(id)
        .execute(&self.pool)
        .await?;

        self.get(id).await?.ok_or_else(|| anyhow::anyhow!("Discord webhook not found after update"))
    }

    pub async fn delete(&self, id: i64) -> Result<()> {
        sqlx::query("DELETE FROM discord_webhooks WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}

/// Create Discord webhook request
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CreateDiscordWebhook {
    pub label: String,
    pub webhook_url: String,
    pub enabled: bool,
    pub notify_on_build_start: bool,
    pub notify_on_build_success: bool,
    pub notify_on_build_failure: bool,
    pub notify_on_deploy_start: bool,
    pub notify_on_deploy_success: bool,
    pub notify_on_deploy_failure: bool,
    pub mention_user_ids: Vec<String>,
    pub mention_role_ids: Vec<String>,
    pub mention_on_failure_only: bool,
}

/// Update Discord webhook request
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct UpdateDiscordWebhook {
    pub label: Option<String>,
    pub webhook_url: Option<String>,
    pub enabled: Option<bool>,
    pub notify_on_build_start: Option<bool>,
    pub notify_on_build_success: Option<bool>,
    pub notify_on_build_failure: Option<bool>,
    pub notify_on_deploy_start: Option<bool>,
    pub notify_on_deploy_success: Option<bool>,
    pub notify_on_deploy_failure: Option<bool>,
    pub mention_user_ids: Option<Vec<String>>,
    pub mention_role_ids: Option<Vec<String>>,
    pub mention_on_failure_only: Option<bool>,
}

