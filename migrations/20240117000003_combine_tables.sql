-- Add change metadata columns to metrics table
ALTER TABLE metrics ADD COLUMN change_type TEXT;
ALTER TABLE metrics ADD COLUMN old_file_path TEXT;

-- Drop file_changes table as it is being merged into metrics
DROP TABLE file_changes;
