-- CodePrism database schema
-- Consolidated from all migrations, applied atomically on init.

-- Projects
CREATE TABLE IF NOT EXISTS projects (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    name TEXT NOT NULL UNIQUE,
    repo_path TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Scan records
CREATE TABLE IF NOT EXISTS scans (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_id INTEGER NOT NULL,
    commit_hash TEXT NOT NULL,
    branch_name TEXT,
    scan_mode TEXT NOT NULL,
    base_commit_hash TEXT,
    scan_time DATETIME DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY(project_id) REFERENCES projects(id)
);

-- Metrics (atomic metric storage with JSON tags)
CREATE TABLE IF NOT EXISTS metrics (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_id INTEGER NOT NULL,
    file_path TEXT NOT NULL,
    change_type TEXT,
    old_file_path TEXT,
    tech_stack TEXT,
    analyzer_id TEXT NOT NULL,
    tags TEXT NOT NULL DEFAULT '{}',
    value_before REAL,
    value_after REAL,
    scope TEXT,
    FOREIGN KEY(scan_id) REFERENCES scans(id)
);

-- Scan jobs (async scan tracking)
CREATE TABLE IF NOT EXISTS scan_jobs (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    project_name TEXT NOT NULL,
    scan_mode TEXT NOT NULL,
    status TEXT NOT NULL DEFAULT 'queued',
    progress INTEGER NOT NULL DEFAULT 0,
    progress_message TEXT,
    error_message TEXT,
    scan_id INTEGER,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP,
    updated_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Scan summaries (per-scan execution stats)
CREATE TABLE IF NOT EXISTS scan_summaries (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_id INTEGER NOT NULL REFERENCES scans(id),
    total_files_scanned INTEGER NOT NULL DEFAULT 0,
    total_analyzers_loaded INTEGER NOT NULL DEFAULT 0,
    total_analyzers_executed INTEGER NOT NULL DEFAULT 0,
    total_analyzer_executions INTEGER NOT NULL DEFAULT 0,
    total_errors INTEGER NOT NULL DEFAULT 0,
    load_errors TEXT,
    analyzer_stats TEXT,
    created_at DATETIME DEFAULT CURRENT_TIMESTAMP
);

-- Indices
CREATE INDEX IF NOT EXISTS idx_scans_project ON scans(project_id);
CREATE INDEX IF NOT EXISTS idx_metrics_scan ON metrics(scan_id);
CREATE INDEX IF NOT EXISTS idx_metrics_analyzer ON metrics(analyzer_id);
CREATE INDEX IF NOT EXISTS idx_metrics_file_prop ON metrics(tech_stack, change_type);
CREATE INDEX IF NOT EXISTS idx_scan_jobs_status ON scan_jobs(status);
CREATE INDEX IF NOT EXISTS idx_scan_jobs_project ON scan_jobs(project_name);
CREATE INDEX IF NOT EXISTS idx_scan_summaries_scan ON scan_summaries(scan_id);

-- Match details (per-match locations like regex matches)
-- No tags column: tag info is available via analyzer_id referencing the analyzer config.
CREATE TABLE IF NOT EXISTS matches (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_id INTEGER NOT NULL,
    file_path TEXT NOT NULL,
    analyzer_id TEXT NOT NULL,
    line_number INTEGER NOT NULL,
    column_start INTEGER,
    column_end INTEGER,
    matched_text TEXT NOT NULL,
    context_before TEXT,
    context_after TEXT,
    FOREIGN KEY(scan_id) REFERENCES scans(id)
);

CREATE INDEX IF NOT EXISTS idx_matches_scan ON matches(scan_id);
CREATE INDEX IF NOT EXISTS idx_matches_scan_file ON matches(scan_id, file_path);
CREATE INDEX IF NOT EXISTS idx_matches_analyzer ON matches(scan_id, analyzer_id);
