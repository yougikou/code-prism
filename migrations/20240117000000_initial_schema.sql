-- 1. Projects Metadata Table
CREATE TABLE IF NOT EXISTS projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL,
    repo_path TEXT NOT NULL UNIQUE,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- 2. Scan Records Table
CREATE TABLE IF NOT EXISTS scans (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL,
    commit_hash TEXT NOT NULL,    -- Target Commit Hash for this scan
    branch_name TEXT,             -- Branch name (optional)
    scan_mode TEXT NOT NULL,      -- 'SNAPSHOT' or 'DIFF'
    base_commit_hash TEXT,        -- If DIFF mode, the baseline commit
    scan_time DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(project_id) REFERENCES projects(id)
);

-- 3. File Changes Table (New - tracks Diff results)
CREATE TABLE IF NOT EXISTS file_changes (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_id INTEGER NOT NULL,
    file_path TEXT NOT NULL,
    change_type TEXT NOT NULL,    -- 'A' (Add), 'M' (Modify), 'D' (Delete), 'N' (Snapshot/Normal)
    old_file_path TEXT,           -- For Renames
    FOREIGN KEY(scan_id) REFERENCES scans(id)
);

-- 4. Atomic Metrics Storage
CREATE TABLE IF NOT EXISTS metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_id INTEGER NOT NULL,
    file_path TEXT NOT NULL,
    
    -- Dimensions
    analyzer_id TEXT NOT NULL,    -- e.g., "loc_counter"
    metric_key TEXT NOT NULL,     -- e.g., "lines"
    category TEXT,                -- e.g., "java"
    
    -- Values
    value REAL NOT NULL,          -- e.g., 150.0
    
    -- Metadata
    scope TEXT,                   -- e.g., "function:main"
    
    FOREIGN KEY(scan_id) REFERENCES scans(id)
);

-- Indices
CREATE INDEX IF NOT EXISTS idx_file_changes_scan ON file_changes(scan_id);
CREATE INDEX IF NOT EXISTS idx_metrics_scan ON metrics(scan_id);
CREATE INDEX IF NOT EXISTS idx_metrics_lookup ON metrics(analyzer_id, metric_key);
CREATE INDEX IF NOT EXISTS idx_metrics_category ON metrics(category);
