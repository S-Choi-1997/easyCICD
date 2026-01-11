-- ============================================================================
-- EasyCI/CD Initial Schema
-- ============================================================================

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
    working_directory TEXT,

    -- Deploy configuration
    runtime_image TEXT NOT NULL,
    runtime_command TEXT NOT NULL,
    health_check_url TEXT NOT NULL,
    runtime_port INTEGER NOT NULL DEFAULT 8080,

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

CREATE INDEX IF NOT EXISTS idx_builds_project_id ON builds(project_id);
CREATE INDEX IF NOT EXISTS idx_builds_status ON builds(status);
CREATE INDEX IF NOT EXISTS idx_builds_started_at ON builds(started_at DESC);
CREATE UNIQUE INDEX IF NOT EXISTS idx_builds_project_build_number ON builds(project_id, build_number);

-- Settings table
CREATE TABLE IF NOT EXISTS settings (
    key TEXT PRIMARY KEY,
    value TEXT NOT NULL,
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Containers table (standalone containers: DB, Redis, etc.)
CREATE TABLE IF NOT EXISTS containers (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    container_id TEXT,
    port INTEGER NOT NULL,
    image TEXT NOT NULL,
    env_vars TEXT,
    command TEXT,
    status TEXT NOT NULL DEFAULT 'stopped' CHECK(status IN ('running', 'stopped')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Port Allocations table (centralized port management)
CREATE TABLE IF NOT EXISTS port_allocations (
    port INTEGER PRIMARY KEY,
    port_type TEXT NOT NULL CHECK(port_type IN ('application', 'container')),
    status TEXT NOT NULL CHECK(status IN ('allocated', 'used_by_system')),
    owner_type TEXT CHECK(owner_type IN ('project', 'container', 'external')),
    owner_id INTEGER,
    container_status TEXT CHECK(container_status IN ('running', 'stopped')),
    last_checked_at TEXT NOT NULL DEFAULT (datetime('now')),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

CREATE INDEX IF NOT EXISTS idx_port_allocations_status ON port_allocations(status);
CREATE INDEX IF NOT EXISTS idx_port_allocations_type ON port_allocations(port_type);
CREATE INDEX IF NOT EXISTS idx_port_allocations_owner ON port_allocations(owner_type, owner_id);

-- Triggers
CREATE TRIGGER IF NOT EXISTS update_project_timestamp
AFTER UPDATE ON projects
FOR EACH ROW
BEGIN
    UPDATE projects SET updated_at = datetime('now') WHERE id = OLD.id;
END;

CREATE TRIGGER IF NOT EXISTS update_settings_timestamp
AFTER UPDATE ON settings
FOR EACH ROW
BEGIN
    UPDATE settings SET updated_at = datetime('now') WHERE key = OLD.key;
END;

CREATE TRIGGER IF NOT EXISTS update_container_timestamp
AFTER UPDATE ON containers
FOR EACH ROW
BEGIN
    UPDATE containers SET updated_at = datetime('now') WHERE id = OLD.id;
END;

-- Initial data migration: Register existing Application ports
INSERT OR IGNORE INTO port_allocations (port, port_type, status, owner_type, owner_id, last_checked_at)
SELECT
    blue_port,
    'application',
    'allocated',
    'project',
    id,
    datetime('now')
FROM projects
WHERE blue_port IS NOT NULL;

INSERT OR IGNORE INTO port_allocations (port, port_type, status, owner_type, owner_id, last_checked_at)
SELECT
    green_port,
    'application',
    'allocated',
    'project',
    id,
    datetime('now')
FROM projects
WHERE green_port IS NOT NULL;
