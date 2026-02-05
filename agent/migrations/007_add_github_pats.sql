-- Multiple GitHub PATs support
CREATE TABLE IF NOT EXISTS github_pats (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    label TEXT NOT NULL,
    token TEXT NOT NULL,
    github_username TEXT,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_github_pats_label ON github_pats(label);

-- Add PAT reference to projects (nullable for backward compat)
ALTER TABLE projects ADD COLUMN github_pat_id INTEGER REFERENCES github_pats(id) ON DELETE SET NULL;

-- Migrate existing global PAT into github_pats table (skip if already migrated)
INSERT OR IGNORE INTO github_pats (label, token, github_username)
SELECT 'Default PAT', value, NULL
FROM settings
WHERE key = 'github_pat' AND value IS NOT NULL AND value != '';

-- Link all existing projects to the migrated PAT
UPDATE projects
SET github_pat_id = (SELECT id FROM github_pats WHERE label = 'Default PAT')
WHERE github_pat_id IS NULL
AND EXISTS (SELECT 1 FROM github_pats WHERE label = 'Default PAT');
