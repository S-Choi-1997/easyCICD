-- Discord webhook configurations
CREATE TABLE IF NOT EXISTS discord_webhooks (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    -- Webhook 정보
    label TEXT NOT NULL,
    webhook_url TEXT NOT NULL,

    -- 알림 필터링 설정
    enabled INTEGER NOT NULL DEFAULT 1,
    notify_on_build_start INTEGER NOT NULL DEFAULT 0,
    notify_on_build_success INTEGER NOT NULL DEFAULT 1,
    notify_on_build_failure INTEGER NOT NULL DEFAULT 1,
    notify_on_deploy_start INTEGER NOT NULL DEFAULT 0,
    notify_on_deploy_success INTEGER NOT NULL DEFAULT 1,
    notify_on_deploy_failure INTEGER NOT NULL DEFAULT 1,

    -- 멘션 설정 (선택사항)
    mention_user_ids TEXT,
    mention_role_ids TEXT,
    mention_on_failure_only INTEGER NOT NULL DEFAULT 1,

    -- Timestamps
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_discord_webhooks_label ON discord_webhooks(label);

-- Trigger for updated_at
CREATE TRIGGER IF NOT EXISTS update_discord_webhook_timestamp
AFTER UPDATE ON discord_webhooks
FOR EACH ROW
BEGIN
    UPDATE discord_webhooks SET updated_at = datetime('now') WHERE id = OLD.id;
END;

-- 프로젝트별 Discord webhook 설정 (선택사항)
ALTER TABLE projects ADD COLUMN discord_webhook_id INTEGER REFERENCES discord_webhooks(id) ON DELETE SET NULL;
