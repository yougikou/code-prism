use codeprism_database::Db;
use std::{
    collections::HashMap,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};
use tokio::sync::{Mutex, Semaphore};

/// In-memory coordination for scan work. Jobs remain durable in SQLite, while
/// cancellation signals and permits deliberately remain process-local.
#[derive(Clone)]
pub struct ScanScheduler {
    permits: Arc<Semaphore>,
    cancellations: Arc<Mutex<HashMap<i64, Arc<AtomicBool>>>>,
}

impl ScanScheduler {
    pub fn from_env() -> Self {
        let concurrency = std::env::var("CODEPRISM_MAX_CONCURRENT_SCANS")
            .ok()
            .and_then(|value| value.parse::<usize>().ok())
            .filter(|value| *value > 0)
            .unwrap_or(1);
        Self {
            permits: Arc::new(Semaphore::new(concurrency)),
            cancellations: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub async fn register(&self, job_id: i64) -> Arc<AtomicBool> {
        let token = Arc::new(AtomicBool::new(false));
        self.cancellations
            .lock()
            .await
            .insert(job_id, token.clone());
        token
    }

    pub async fn cancel(&self, job_id: i64) -> bool {
        if let Some(token) = self.cancellations.lock().await.get(&job_id) {
            token.store(true, Ordering::Release);
            true
        } else {
            false
        }
    }

    pub async fn acquire(
        &self,
        token: &Arc<AtomicBool>,
    ) -> Option<tokio::sync::OwnedSemaphorePermit> {
        if token.load(Ordering::Acquire) {
            return None;
        }
        let permit = self.permits.clone().acquire_owned().await.ok()?;
        (!token.load(Ordering::Acquire)).then_some(permit)
    }

    pub async fn finish(&self, job_id: i64) {
        self.cancellations.lock().await.remove(&job_id);
    }

    /// Work cannot safely be resumed because its request payload is not stored.
    /// Mark interrupted jobs terminally so clients never poll forever.
    pub async fn recover_interrupted_jobs(&self, db: &Db) -> anyhow::Result<()> {
        sqlx::query("UPDATE scan_jobs SET status = 'failed', progress = 100, error_message = 'Server restarted before scan completed', updated_at = CURRENT_TIMESTAMP WHERE status IN ('queued', 'running')")
            .execute(db.pool()).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn restart_recovery_finishes_incomplete_jobs() {
        let db = Db::new("sqlite::memory:").await.unwrap();
        db.migrate().await.unwrap();
        sqlx::query("INSERT INTO scan_jobs (project_name, scan_mode, status) VALUES ('a', 'snapshot', 'queued'), ('b', 'snapshot', 'running')")
            .execute(db.pool()).await.unwrap();
        ScanScheduler::from_env()
            .recover_interrupted_jobs(&db)
            .await
            .unwrap();
        let statuses: Vec<String> = sqlx::query_scalar("SELECT status FROM scan_jobs ORDER BY id")
            .fetch_all(db.pool())
            .await
            .unwrap();
        assert_eq!(statuses, ["failed", "failed"]);
    }
}
