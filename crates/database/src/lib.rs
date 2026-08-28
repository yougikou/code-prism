use anyhow::Result;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::{Pool, Sqlite};
use std::str::FromStr;
use std::time::Duration;

#[derive(Clone)]
pub struct Db {
    pool: Pool<Sqlite>,
}

impl Db {
    pub async fn new(db_url: &str) -> Result<Self> {
        let options = SqliteConnectOptions::from_str(db_url)?
            .create_if_missing(true)
            .foreign_keys(true)
            .journal_mode(SqliteJournalMode::Wal)
            .busy_timeout(Duration::from_secs(5));

        let pool = SqlitePoolOptions::new().connect_with(options).await?;

        Ok(Self { pool })
    }

    pub async fn migrate(&self) -> Result<()> {
        let schema = include_str!("init.sql");
        sqlx::raw_sql(schema).execute(&self.pool).await?;
        // Successful finalization removes rows transactionally. Retain failed
        // analyzer inputs for diagnosis/retry, but bound their lifetime.
        let removed = sqlx::query(
            "DELETE FROM intermediate_blocks WHERE scan_id NOT IN (SELECT id FROM scans) \
             OR scan_id IN (SELECT id FROM scans WHERE scan_time < datetime('now', '-7 days'))",
        )
        .execute(&self.pool)
        .await?
        .rows_affected();
        if removed > 0 {
            eprintln!("Removed {} stale cross-file intermediate blocks", removed);
        }
        Ok(())
    }

    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn startup_cleanup_removes_only_intermediates_older_than_one_week() {
        let db = Db::new("sqlite::memory:").await.unwrap();
        db.migrate().await.unwrap();
        sqlx::query("INSERT INTO projects (name) VALUES ('cleanup')")
            .execute(db.pool())
            .await
            .unwrap();
        sqlx::query("INSERT INTO scans (project_id, commit_hash, scan_mode, scan_time) VALUES (1, 'old', 'SNAPSHOT', datetime('now', '-8 days')), (1, 'new', 'SNAPSHOT', datetime('now', '-6 days'))")
            .execute(db.pool()).await.unwrap();
        sqlx::query("INSERT INTO intermediate_blocks (scan_id, analyzer_id, file_path, group_key) VALUES (1, 'a', 'old.rs', 'x'), (2, 'a', 'new.rs', 'y')")
            .execute(db.pool()).await.unwrap();

        db.migrate().await.unwrap();

        let paths: Vec<String> = sqlx::query_scalar("SELECT file_path FROM intermediate_blocks")
            .fetch_all(db.pool())
            .await
            .unwrap();
        assert_eq!(paths, vec!["new.rs"]);
    }

    #[tokio::test]
    async fn connections_enable_foreign_key_enforcement() {
        let db = Db::new("sqlite::memory:").await.unwrap();
        let enabled: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(db.pool())
            .await
            .unwrap();
        assert_eq!(enabled, 1);
    }
}
