use crate::{config::AppConfig, git_cache::GitCache, scan_scheduler::ScanScheduler};
use codeprism_core::CodePrismConfig;
use codeprism_database::Db;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex, RwLock},
};
use tokio::sync::{Mutex as AsyncMutex, OwnedMutexGuard};

#[derive(Clone)]
pub struct AppState {
    pub(crate) config: Arc<RwLock<AppConfig>>,
    pub(crate) db: Db,
    pub(crate) core_config: Arc<RwLock<CodePrismConfig>>,
    pub(crate) git_cache: GitCache,
    pub(crate) api_token: Option<Arc<str>>,
    pub(crate) scan_scheduler: ScanScheduler,
    pub(crate) project_locks: Arc<Mutex<HashMap<String, Arc<AsyncMutex<()>>>>>,
    pub(crate) config_writer: Arc<AsyncMutex<()>>,
    pub config_path: String,
}

impl AppState {
    pub fn new(
        config: Arc<RwLock<AppConfig>>,
        db: Db,
        core_config: Arc<RwLock<CodePrismConfig>>,
        config_path: String,
    ) -> Self {
        let cloned_repos_dir = std::env::current_dir()
            .unwrap_or_else(|_| std::path::PathBuf::from("."))
            .join("cloned_repos");
        Self {
            config,
            db,
            core_config,
            git_cache: GitCache::new(cloned_repos_dir),
            api_token: None,
            scan_scheduler: ScanScheduler::from_env(),
            project_locks: Arc::new(Mutex::new(HashMap::new())),
            config_writer: Arc::new(AsyncMutex::new(())),
            config_path,
        }
    }

    /// Acquire an exclusive in-process lifecycle lock without waiting. Scans
    /// hold it from job creation through terminal status; lifecycle mutations
    /// fail with a conflict while any scan or mutation owns it.
    pub(crate) fn try_project_lock(&self, name: &str) -> Option<OwnedMutexGuard<()>> {
        let lock = self
            .project_locks
            .lock()
            .unwrap()
            .entry(name.to_owned())
            .or_insert_with(|| Arc::new(AsyncMutex::new(())))
            .clone();
        lock.try_lock_owned().ok()
    }

    pub(crate) async fn project_deletion_pending(&self, name: &str) -> anyhow::Result<bool> {
        sqlx::query_scalar(
            "SELECT EXISTS(SELECT 1 FROM project_deletion_operations WHERE project_name = ? AND stage != 'completed')",
        )
        .bind(name)
        .fetch_one(self.db.pool())
        .await
        .map_err(Into::into)
    }

    pub(crate) async fn pending_project_deletion(
        &self,
        name: &str,
    ) -> anyhow::Result<Option<(String, Option<String>)>> {
        sqlx::query_as(
            "SELECT operation_id, last_error FROM project_deletion_operations \
             WHERE project_name = ? AND stage != 'completed' \
             ORDER BY created_at DESC LIMIT 1",
        )
        .bind(name)
        .fetch_optional(self.db.pool())
        .await
        .map_err(Into::into)
    }

    pub(crate) async fn lock_config_writer(&self) -> OwnedMutexGuard<()> {
        self.config_writer.clone().lock_owned().await
    }
}
