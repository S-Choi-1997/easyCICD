-- Add github_webhook_id to projects table for automatic webhook management
ALTER TABLE projects ADD COLUMN github_webhook_id INTEGER;
