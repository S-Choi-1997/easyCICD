-- Projects table
CREATE TABLE IF NOT EXISTS projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,

    -- Project identification
    name TEXT NOT NULL UNIQUE,
    repo TEXT NOT NULL,
    path_filter TEXT NOT NULL DEFAULT '**',
    branch TEXT NOT NULL DEFAULT 'main',

    -- Build configuration
    build_image TEXT NOT NULL,
    build_command TEXT NOT NULL,
    cache_type TEXT NOT NULL,

    -- Deploy configuration
    runtime_image TEXT NOT NULL,
    runtime_command TEXT NOT NULL,
    health_check_url TEXT NOT NULL,

    -- Port allocation
    blue_port INTEGER NOT NULL,
    green_port INTEGER NOT NULL,
    active_slot TEXT NOT NULL CHECK(active_slot IN ('Blue', 'Green')) DEFAULT 'Blue',

    -- Container IDs
    blue_container_id TEXT,
    green_container_id TEXT,

    -- Timestamps
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Index on project name for fast lookups
CREATE INDEX IF NOT EXISTS idx_projects_name ON projects(name);

-- Builds table
CREATE TABLE IF NOT EXISTS builds (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL,
    build_number INTEGER NOT NULL,
    commit_hash TEXT NOT NULL,
    commit_message TEXT,
    author TEXT,

    -- Build status
    status TEXT NOT NULL CHECK(status IN ('Queued', 'Building', 'Deploying', 'Success', 'Failed')) DEFAULT 'Queued',

    -- Paths
    log_path TEXT NOT NULL,
    output_path TEXT,

    -- Deployment info
    deployed_slot TEXT CHECK(deployed_slot IN ('Blue', 'Green')),

    -- Timestamps
    started_at TEXT NOT NULL DEFAULT (datetime('now')),
    finished_at TEXT,

    FOREIGN KEY (project_id) REFERENCES projects(id) ON DELETE CASCADE
);

-- Indexes for builds
CREATE INDEX IF NOT EXISTS idx_builds_project_id ON builds(project_id);
CREATE INDEX IF NOT EXISTS idx_builds_status ON builds(status);
CREATE INDEX IF NOT EXISTS idx_builds_started_at ON builds(started_at DESC);

-- Unique constraint for build numbers per project
CREATE UNIQUE INDEX IF NOT EXISTS idx_builds_project_build_number ON builds(project_id, build_number);

-- Trigger to update project updated_at
CREATE TRIGGER IF NOT EXISTS update_project_timestamp
AFTER UPDATE ON projects
FOR EACH ROW
BEGIN
    UPDATE projects SET updated_at = datetime('now') WHERE id = OLD.id;
END;
