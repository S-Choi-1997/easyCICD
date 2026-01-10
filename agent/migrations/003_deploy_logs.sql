-- Add deploy_log_path column to builds table
ALTER TABLE builds ADD COLUMN deploy_log_path TEXT;
