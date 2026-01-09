-- Add working_directory column to projects table
-- SQLite doesn't support IF NOT EXISTS for ALTER TABLE ADD COLUMN
-- We handle errors in the migration code instead
ALTER TABLE projects ADD COLUMN working_directory TEXT;

-- Update existing projects to use empty string (root directory)
UPDATE projects SET working_directory = '' WHERE working_directory IS NULL;
