-- Add 'pulling' and 'starting' status to containers table
-- SQLite doesn't support ALTER CONSTRAINT, so we need to recreate the table

-- Step 1: Create new table with updated constraint
CREATE TABLE containers_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    container_id TEXT,
    port INTEGER NOT NULL,
    container_port INTEGER,
    persist_data INTEGER NOT NULL DEFAULT 0,
    image TEXT NOT NULL,
    env_vars TEXT,
    command TEXT,
    protocol_type TEXT NOT NULL DEFAULT 'tcp' CHECK(protocol_type IN ('tcp', 'http')),
    status TEXT NOT NULL DEFAULT 'stopped' CHECK(status IN ('running', 'stopped', 'pulling', 'starting')),
    created_at TEXT NOT NULL DEFAULT (datetime('now')),
    updated_at TEXT NOT NULL DEFAULT (datetime('now'))
);

-- Step 2: Copy data from old table
INSERT INTO containers_new (id, name, container_id, port, container_port, persist_data, image, env_vars, command, protocol_type, status, created_at, updated_at)
SELECT id, name, container_id, port, container_port, persist_data, image, env_vars, command,
       COALESCE(protocol_type, 'tcp'), status, created_at, updated_at
FROM containers;

-- Step 3: Drop old table
DROP TABLE containers;

-- Step 4: Rename new table
ALTER TABLE containers_new RENAME TO containers;

-- Step 5: Update port_allocations table constraint as well
CREATE TABLE port_allocations_new (
    port INTEGER PRIMARY KEY,
    port_type TEXT NOT NULL CHECK(port_type IN ('application', 'container')),
    status TEXT NOT NULL CHECK(status IN ('allocated', 'used_by_system')),
    owner_type TEXT CHECK(owner_type IN ('project', 'container', 'external')),
    owner_id INTEGER,
    container_status TEXT CHECK(container_status IN ('running', 'stopped', 'pulling', 'starting')),
    last_checked_at TEXT NOT NULL DEFAULT (datetime('now')),
    created_at TEXT NOT NULL DEFAULT (datetime('now'))
);

INSERT INTO port_allocations_new (port, port_type, status, owner_type, owner_id, container_status, last_checked_at, created_at)
SELECT port, port_type, status, owner_type, owner_id, container_status, last_checked_at, created_at
FROM port_allocations;

DROP TABLE port_allocations;

ALTER TABLE port_allocations_new RENAME TO port_allocations;

CREATE INDEX IF NOT EXISTS idx_port_allocations_status ON port_allocations(status);
