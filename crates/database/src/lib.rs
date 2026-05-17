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
        Ok(())
    }

    pub fn pool(&self) -> &Pool<Sqlite> {
        &self.pool
    }
}
