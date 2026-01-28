-- Add environment variables columns to projects table
-- build_env_vars: JSON string for build-time environment variables
-- runtime_env_vars: JSON string for runtime environment variables

ALTER TABLE projects ADD COLUMN build_env_vars TEXT;
ALTER TABLE projects ADD COLUMN runtime_env_vars TEXT;
