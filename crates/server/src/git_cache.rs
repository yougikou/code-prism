use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GitRepo {
    pub path: String,
    pub git_url: String,
    pub current_branch: String,
}

#[derive(Clone, Debug)]
pub struct GitCache {
    repos: Arc<Mutex<HashMap<String, GitRepo>>>,
    base_dir: PathBuf,
    storage_path: PathBuf,
}

impl GitCache {
    /// Create a new GitCache with the given base directory for storing cloned repos.
    /// The cache will persist repo metadata to `base_dir/git_repos_cache.json`.
    /// Existing persisted data is loaded automatically.
    pub fn new(base_dir: PathBuf) -> Self {
        let storage_path = base_dir.join("git_repos_cache.json");

        // Load previously persisted repos
        let repos: HashMap<String, GitRepo> = if let Ok(file) = std::fs::File::open(&storage_path) {
            serde_json::from_reader(file).unwrap_or_default()
        } else {
            HashMap::new()
        };

        // Ensure the base directory exists
        let _ = std::fs::create_dir_all(&base_dir);

        Self {
            repos: Arc::new(Mutex::new(repos)),
            base_dir,
            storage_path,
        }
    }

    /// Persist the current state to disk
    fn save(&self) {
        if let Some(parent) = self.storage_path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(repos) = self.repos.lock() {
            if let Ok(file) = std::fs::File::create(&self.storage_path) {
                let _ = serde_json::to_writer(file, &*repos);
            }
        }
    }

    pub fn insert(&self, id: String, repo: GitRepo) {
        self.repos.lock().unwrap().insert(id, repo);
        self.save();
    }

    pub fn get(&self, id: &str) -> Option<GitRepo> {
        self.repos.lock().unwrap().get(id).cloned()
    }

    /// Remove a repo from the cache. Returns the removed repo if it existed.
    pub fn remove(&self, id: &str) -> Option<GitRepo> {
        let removed = self.repos.lock().unwrap().remove(id);
        if removed.is_some() {
            self.save();
        }
        removed
    }

    /// List all cached repos sorted by ID (insertion-independent order).
    pub fn list_all(&self) -> Vec<(String, GitRepo)> {
        let repos = self.repos.lock().unwrap();
        let mut list: Vec<_> = repos.iter().map(|(k, v)| (k.clone(), v.clone())).collect();
        list.sort_by(|a, b| a.0.cmp(&b.0));
        list
    }

    /// Get the directory where a repo with the given ID should be stored.
    pub fn repo_dir(&self, repo_id: &str) -> PathBuf {
        self.base_dir.join(repo_id)
    }
}
