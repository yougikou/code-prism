-- Make project name the canonical identifier:
--  - name becomes NOT NULL UNIQUE
--  - repo_path becomes nullable (project can exist without a repo)

CREATE TABLE IF NOT EXISTS projects_v2 (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    repo_path TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Migrate existing data, keeping the highest id per duplicate name
INSERT OR IGNORE INTO projects_v2 (id, name, repo_path, created_at)
SELECT id, name, repo_path, created_at FROM projects;

-- Handle any rows skipped by INSERT OR IGNORE due to duplicate names
-- Keep the row with the highest id for each duplicate name
INSERT OR REPLACE INTO projects_v2 (id, name, repo_path, created_at)
SELECT p.id, p.name, p.repo_path, p.created_at
FROM projects p
INNER JOIN (
    SELECT name, MAX(id) AS max_id
    FROM projects
    GROUP BY name
    HAVING COUNT(*) > 1
) dup ON p.name = dup.name AND p.id = dup.max_id;

DROP TABLE projects;
ALTER TABLE projects_v2 RENAME TO projects;

CREATE INDEX IF NOT EXISTS idx_scans_project ON scans(project_id);
