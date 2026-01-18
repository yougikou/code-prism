-- Recreate table cleanly to handle column changes and data migration
CREATE TABLE metrics_new (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    scan_id INTEGER NOT NULL,
    file_path TEXT NOT NULL,
    change_type TEXT, 
    old_file_path TEXT,
    tech_stack TEXT,
    analyzer_id TEXT NOT NULL,
    metric_key TEXT NOT NULL,
    category TEXT,
    value_before REAL,
    value_after REAL, 
    scope TEXT,
    FOREIGN KEY(scan_id) REFERENCES scans(id)
);

-- Copy data. Note: previous schema had `value`, copy it to `value_after`
INSERT INTO metrics_new (id, scan_id, file_path, change_type, old_file_path, tech_stack, analyzer_id, metric_key, category, value_after, value_before, scope)
SELECT id, scan_id, file_path, change_type, old_file_path, tech_stack, analyzer_id, metric_key, category, value, NULL, scope
FROM metrics;

DROP TABLE metrics;
ALTER TABLE metrics_new RENAME TO metrics;

CREATE INDEX idx_metrics_scan ON metrics(scan_id);
CREATE INDEX idx_metrics_lookup ON metrics(analyzer_id, metric_key);
CREATE INDEX idx_metrics_file_prop ON metrics(tech_stack, change_type);
