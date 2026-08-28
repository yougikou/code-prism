use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GitRepo {
    pub path: String,
    pub git_url: String,
    pub current_branch: String,
    /// Only repositories cloned into CodePrism's managed cache may be removed
    /// from disk by API calls. Local repositories are registered by reference.
    #[serde(default)]
    pub managed: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
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
        if let Ok(repos) = self.repos.lock()
            && let Ok(file) = std::fs::File::create(&self.storage_path)
        {
            let _ = serde_json::to_writer(file, &*repos);
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

    /// Returns whether CodePrism owns `repo` and can safely remove it.
    ///
    /// The path containment check prevents a stale or tampered cache entry from
    /// turning a managed flag into permission to delete an arbitrary directory.
    pub fn can_remove_files(&self, repo: &GitRepo) -> bool {
        repo.managed && self.is_managed_path(&repo.path)
    }

    pub fn is_managed_path(&self, path: &str) -> bool {
        let Ok(base_dir) = std::fs::canonicalize(&self.base_dir) else {
            return false;
        };
        let Ok(repo_path) = std::fs::canonicalize(path) else {
            return false;
        };

        repo_path != base_dir && repo_path.starts_with(base_dir)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo(path: String, managed: bool) -> GitRepo {
        GitRepo {
            path,
            git_url: String::new(),
            current_branch: "main".to_string(),
            managed,
            project_name: None,
        }
    }

    #[test]
    fn only_managed_repositories_under_the_cache_can_be_deleted() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache_dir = temp_dir.path().join("cache");
        let managed_dir = cache_dir.join("clone");
        let local_dir = temp_dir.path().join("local");
        std::fs::create_dir_all(&managed_dir).unwrap();
        std::fs::create_dir_all(&local_dir).unwrap();
        let cache = GitCache::new(cache_dir);

        assert!(cache.can_remove_files(&repo(managed_dir.to_string_lossy().to_string(), true,)));
        assert!(!cache.can_remove_files(&repo(local_dir.to_string_lossy().to_string(), true,)));
        assert!(!cache.can_remove_files(&repo(local_dir.to_string_lossy().to_string(), false,)));
    }
}
