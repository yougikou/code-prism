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

CREATE INDEX IF NOT EXISTS idx_scan_summaries_scan ON scan_summaries(scan_id);
