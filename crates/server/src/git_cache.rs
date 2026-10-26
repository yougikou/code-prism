use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::Write;
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
    /// Set only by the successful clone flow, then validated against the cache
    /// entry ID. Older entries remain displayable but cannot delete files.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub managed_origin_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub project_name: Option<String>,
}

#[derive(Clone, Debug)]
pub struct GitCache {
    repos: Arc<Mutex<HashMap<String, GitRepo>>>,
    base_dir: PathBuf,
    storage_path: PathBuf,
    load_error: Option<String>,
}

impl GitCache {
    /// Create a new GitCache with the given base directory for storing cloned repos.
    /// The cache will persist repo metadata to `base_dir/git_repos_cache.json`.
    /// Existing persisted data is loaded automatically.
    pub fn new(base_dir: PathBuf) -> Self {
        let storage_path = base_dir.join("git_repos_cache.json");

        // Load previously persisted repos
        let (repos, load_error): (HashMap<String, GitRepo>, Option<String>) =
            match std::fs::File::open(&storage_path) {
                Ok(file) => match serde_json::from_reader(file) {
                    Ok(repos) => (repos, None),
                    Err(error) => {
                        eprintln!(
                            "Repository cache '{}' is invalid and was left untouched: {error}",
                            storage_path.display()
                        );
                        (HashMap::new(), Some(error.to_string()))
                    }
                },
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                    (HashMap::new(), None)
                }
                Err(error) => (HashMap::new(), Some(error.to_string())),
            };

        // Ensure the base directory exists
        let _ = std::fs::create_dir_all(&base_dir);

        Self {
            repos: Arc::new(Mutex::new(repos)),
            base_dir,
            storage_path,
            load_error,
        }
    }

    /// Persist a complete new snapshot before exposing it in memory.
    fn save(&self, repos: &HashMap<String, GitRepo>) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.load_error.is_none(),
            "Repository cache could not be loaded and was not overwritten: {}",
            self.load_error.as_deref().unwrap_or_default()
        );
        if let Some(parent) = self.storage_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let bytes = serde_json::to_vec(repos)?;
        let temp_path = self
            .storage_path
            .with_extension(format!("tmp-{}", uuid::Uuid::new_v4()));
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temp_path)?;
        let result = (|| -> anyhow::Result<()> {
            file.write_all(&bytes)?;
            file.sync_all()?;
            crate::config_persistence::replace_file(&temp_path, &self.storage_path)?;
            Ok(())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&temp_path);
        }
        result
    }

    pub fn insert(&self, id: String, repo: GitRepo) -> anyhow::Result<()> {
        let mut repos = self.repos.lock().unwrap();
        let mut next = repos.clone();
        next.insert(id, repo);
        self.save(&next)?;
        *repos = next;
        Ok(())
    }

    pub fn get(&self, id: &str) -> Option<GitRepo> {
        self.repos.lock().unwrap().get(id).cloned()
    }

    /// Remove a repo from the cache. Returns the removed repo if it existed.
    pub fn remove(&self, id: &str) -> anyhow::Result<Option<GitRepo>> {
        let mut repos = self.repos.lock().unwrap();
        let mut next = repos.clone();
        let removed = next.remove(id);
        if removed.is_some() {
            self.save(&next)?;
            *repos = next;
        }
        Ok(removed)
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

    pub fn storage_path(&self) -> &std::path::Path {
        &self.storage_path
    }

    /// Returns whether CodePrism owns the exact directory assigned to this
    /// cache entry. Both the entry ID and path must match the clone location.
    pub fn can_remove_files(&self, repo_id: &str, repo: &GitRepo) -> bool {
        self.owns_repo_path(repo_id, repo)
    }

    /// File deletion is allowed only for the exact directory assigned to this
    /// cache entry. Being some descendant of the cache root is insufficient.
    pub fn owns_repo_path(&self, repo_id: &str, repo: &GitRepo) -> bool {
        if !repo.managed || repo.managed_origin_id.as_deref() != Some(repo_id) {
            return false;
        }
        let expected = self.repo_dir(repo_id);
        if is_link_or_reparse_point(&self.base_dir) {
            return false;
        }
        let Ok(expected_canonical) = std::fs::canonicalize(&expected) else {
            return false;
        };
        let Ok(repo_canonical) = std::fs::canonicalize(&repo.path) else {
            return false;
        };
        let Ok(metadata) = std::fs::symlink_metadata(&expected) else {
            return false;
        };
        !metadata.file_type().is_symlink()
            && !is_link_or_reparse_point(&expected)
            && expected_canonical == repo_canonical
    }
}

fn is_link_or_reparse_point(path: &std::path::Path) -> bool {
    if std::fs::symlink_metadata(path).is_ok_and(|metadata| metadata.file_type().is_symlink()) {
        return true;
    }
    #[cfg(windows)]
    {
        use std::os::windows::ffi::OsStrExt;
        use windows_sys::Win32::Storage::FileSystem::{
            FILE_ATTRIBUTE_REPARSE_POINT, GetFileAttributesW, INVALID_FILE_ATTRIBUTES,
        };
        let path: Vec<u16> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        let attributes = unsafe { GetFileAttributesW(path.as_ptr()) };
        attributes != INVALID_FILE_ATTRIBUTES && attributes & FILE_ATTRIBUTE_REPARSE_POINT != 0
    }
    #[cfg(not(windows))]
    false
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
            managed_origin_id: managed.then(|| "clone".to_string()),
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

        assert!(cache.can_remove_files(
            "clone",
            &repo(managed_dir.to_string_lossy().to_string(), true,)
        ));
        assert!(!cache.can_remove_files(
            "different-id",
            &repo(managed_dir.to_string_lossy().to_string(), true,)
        ));
        assert!(!cache.can_remove_files(
            "clone",
            &repo(local_dir.to_string_lossy().to_string(), true,)
        ));
        assert!(!cache.can_remove_files(
            "clone",
            &repo(managed_dir.to_string_lossy().to_string(), false,)
        ));
    }

    #[test]
    fn invalid_cache_is_not_silently_overwritten() {
        let temp_dir = tempfile::tempdir().unwrap();
        let cache_dir = temp_dir.path().join("cache");
        std::fs::create_dir_all(&cache_dir).unwrap();
        let storage_path = cache_dir.join("git_repos_cache.json");
        std::fs::write(&storage_path, b"not json").unwrap();
        let cache = GitCache::new(cache_dir);
        let before = std::fs::read(&storage_path).unwrap();

        assert!(
            cache
                .insert("repo".into(), repo("/tmp/repo".into(), false))
                .is_err()
        );
        assert_eq!(std::fs::read(storage_path).unwrap(), before);
        assert!(cache.get("repo").is_none());
    }
}
