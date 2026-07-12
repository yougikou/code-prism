use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::str::FromStr;

#[derive(Clone)]
pub struct Db {
    pool: Pool<Sqlite>,
}

impl Db {
    pub async fn new(db_url: &str) -> Result<Self> {
        let options = SqliteConnectOptions::from_str(db_url)?.create_if_missing(true);

        let pool = SqlitePoolOptions::new().connect_with(options).await?;

        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<()> {
        let schema = include_str!("init.sql");
        sqlx::raw_sql(schema).execute(&self.pool).await?;
        // Migration: add progress_message column for existing databases
        let _ = sqlx::raw_sql("ALTER TABLE scan_jobs ADD COLUMN progress_message TEXT")
            .execute(&self.pool)
            .await;
        // Migration: make matches.file_path and line_number nullable (SQLite requires recreate)
        // Only needed if matches table still has NOT NULL constraints
        let still_not_null: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('matches') WHERE name = 'file_path' AND NOT nullable"
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(false);
        if still_not_null {
            sqlx::raw_sql("ALTER TABLE matches RENAME TO matches_old").execute(&self.pool).await?;
            // Recreate with nullable columns (definition from init.sql)
            let _ = sqlx::raw_sql("DROP TABLE IF EXISTS matches").execute(&self.pool).await;
            let schema = include_str!("init.sql");
            sqlx::raw_sql(schema).execute(&self.pool).await?;
            // Migrate data
            sqlx::raw_sql("
                INSERT INTO matches (id, scan_id, file_path, analyzer_id, line_number, column_start, column_end, matched_text, side, context_before, context_after)
                SELECT id, scan_id, file_path, analyzer_id, line_number, column_start, column_end, matched_text, side, context_before, context_after FROM matches_old
            ").execute(&self.pool).await?;
            sqlx::raw_sql("DROP TABLE matches_old").execute(&self.pool).await?;
        }
        // Migration: expression index for duplication content_hash lookup
        sqlx::raw_sql(
            "CREATE INDEX IF NOT EXISTS idx_metrics_dup_content_hash \
             ON metrics(scan_id, json_extract(tags, '$.content_hash')) \
             WHERE json_extract(tags, '$.metric') = 'duplicate_block'"
        )
        .execute(&self.pool)
        .await?;
        // Migration: rename content_blocks → intermediate_blocks
        // (intermediate_blocks is created by init.sql; old table may exist from before)
        let has_content_blocks: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='content_blocks'"
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(false);
        if has_content_blocks {
            // If intermediate_blocks doesn't exist yet, rename — otherwise just drop old
            let has_intermediate: bool = sqlx::query_scalar(
                "SELECT COUNT(*) > 0 FROM sqlite_master WHERE type='table' AND name='intermediate_blocks'"
            )
            .fetch_one(&self.pool)
            .await
            .unwrap_or(false);
            if !has_intermediate {
                // Rename old table to new schema — data loss is acceptable
                // (content_blocks is ephemeral, cleared after each scan)
                sqlx::raw_sql("DROP TABLE IF EXISTS content_blocks")
                    .execute(&self.pool)
                    .await?;
            } else {
                sqlx::raw_sql("DROP TABLE IF EXISTS content_blocks")
                    .execute(&self.pool)
                    .await?;
            }
            // Re-run CREATE from init.sql to ensure intermediate_blocks exists
            sqlx::raw_sql(
                "CREATE TABLE IF NOT EXISTS intermediate_blocks (
                    id INTEGER PRIMARY KEY AUTOINCREMENT,
                    scan_id INTEGER NOT NULL,
                    analyzer_id TEXT NOT NULL,
                    file_path TEXT NOT NULL,
                    group_key TEXT NOT NULL,
                    blob_data TEXT,
                    int_data1 INTEGER,
                    int_data2 INTEGER,
                    int_data3 INTEGER,
                    str_data1 TEXT,
                    str_data2 TEXT,
                    FOREIGN KEY(scan_id) REFERENCES scans(id)
                )"
            )
            .execute(&self.pool)
            .await?;
            sqlx::raw_sql(
                "CREATE INDEX IF NOT EXISTS idx_intermediate_scan_analyzer ON intermediate_blocks(scan_id, analyzer_id)"
            )
            .execute(&self.pool)
            .await?;
            sqlx::raw_sql(
                "CREATE INDEX IF NOT EXISTS idx_intermediate_group ON intermediate_blocks(scan_id, analyzer_id, group_key)"
            )
            .execute(&self.pool)
            .await?;
        }
        // Migration: add commit_timestamp for existing databases
        let has_commit_ts: bool = sqlx::query_scalar(
            "SELECT COUNT(*) > 0 FROM pragma_table_info('scans') WHERE name = 'commit_timestamp'"
        )
        .fetch_one(&self.pool)
        .await
        .unwrap_or(false);
        if !has_commit_ts {
            sqlx::raw_sql("ALTER TABLE scans ADD COLUMN commit_timestamp INTEGER")
                .execute(&self.pool)
                .await?;
        }
        Ok(())
    }

    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }
}
