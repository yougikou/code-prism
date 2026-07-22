use crate::{config::AppConfig, git_cache::GitCache};
use codeprism_core::CodePrismConfig;
use codeprism_database::Db;
use std::sync::{Arc, RwLock};

#[derive(Clone)]
pub struct AppState {
    pub(crate) config: Arc<RwLock<AppConfig>>,
    pub(crate) db: Db,
    pub(crate) core_config: Arc<RwLock<CodePrismConfig>>,
    pub(crate) git_cache: GitCache,
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
            config_path,
        }
    }
}
