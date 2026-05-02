use crate::routes::AppState;
use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use git2::Repository;
use serde::{Deserialize, Serialize};

// ─── Request / Response types ────────────────────────────────────────────

#[derive(Deserialize)]
pub struct CloneRequest {
    pub git_url: String,
}

#[derive(Serialize)]
pub struct BranchInfo {
    pub name: String,
    pub is_head: bool,
    pub is_remote: bool,
}

#[derive(Serialize)]
pub struct CloneResponse {
    pub repo_id: String,
    pub branches: Vec<BranchInfo>,
    pub current_branch: String,
}

#[derive(Serialize)]
pub struct BranchesResponse {
    pub branches: Vec<BranchInfo>,
    pub current_branch: String,
}

#[derive(Deserialize)]
pub struct CheckoutRequest {
    pub branch: String,
}

#[derive(Serialize)]
pub struct CheckoutResponse {
    pub branch: String,
    pub message: String,
}

#[derive(Deserialize)]
pub struct CommitsParams {
    #[serde(rename = "ref")]
    pub ref_: Option<String>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    pub search: Option<String>,
}

#[derive(Serialize)]
pub struct CommitInfo {
    pub hash: String,
    pub short_hash: String,
    pub message: String,
    pub author: String,
    pub timestamp: i64,
}

#[derive(Serialize)]
pub struct CommitsResponse {
    pub commits: Vec<CommitInfo>,
    pub has_more: bool,
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
}

// ─── Helpers ─────────────────────────────────────────────────────────────

fn err_response(status: StatusCode, msg: String) -> Response {
    (status, Json(ErrorResponse { error: msg })).into_response()
}

fn extract_branches(repo: &Repository) -> Result<(Vec<BranchInfo>, String), String> {
    let mut branches: Vec<BranchInfo> = Vec::new();
    let mut current_branch = String::new();

    let head = repo.head().ok();
    let head_shorthand = head.as_ref().and_then(|h| h.shorthand()).map(|s| s.to_string());

    // Collect local branches
    let local_iter = repo.branches(Some(git2::BranchType::Local)).map_err(|e| e.message().to_string())?;
    for branch_result in local_iter {
        let (branch, _) = branch_result.map_err(|e| e.message().to_string())?;
        let name = branch
            .name()
            .map_err(|e| e.message().to_string())?
            .ok_or("Branch name is not valid UTF-8")?
            .to_string();

        let is_head = if let Some(ref sh) = head_shorthand {
            sh == &name
        } else {
            false
        };

        if is_head {
            current_branch = name.clone();
        }

        branches.push(BranchInfo { name, is_head, is_remote: false });
    }

    // Collect remote branches (e.g. origin/main, upstream/develop)
    let remote_iter = repo.branches(Some(git2::BranchType::Remote)).map_err(|e| e.message().to_string())?;
    for branch_result in remote_iter {
        let (branch, _) = branch_result.map_err(|e| e.message().to_string())?;
        let name = match branch.name() {
            Ok(Some(n)) => n.to_string(),
            _ => continue,
        };

        // Skip the remote HEAD reference (e.g. "origin/HEAD")
        if name.ends_with("/HEAD") {
            continue;
        }

        branches.push(BranchInfo { name, is_head: false, is_remote: true });
    }

    branches.sort_by(|a, b| {
        if a.is_head {
            std::cmp::Ordering::Less
        } else if b.is_head {
            std::cmp::Ordering::Greater
        } else if a.is_remote != b.is_remote {
            if a.is_remote { std::cmp::Ordering::Greater } else { std::cmp::Ordering::Less }
        } else {
            a.name.cmp(&b.name)
        }
    });

    Ok((branches, current_branch))
}

fn walk_commits(
    repo: &Repository,
    ref_name: &str,
    offset: usize,
    limit: usize,
    search: Option<&str>,
) -> Result<(Vec<CommitInfo>, bool), String> {
    let revspec = repo.revparse_single(ref_name).map_err(|e| e.message().to_string())?;
    let commit = revspec
        .into_commit()
        .map_err(|_| format!("'{}' is not a commit", ref_name))?;

    let mut revwalk = repo.revwalk().map_err(|e| e.message().to_string())?;
    revwalk
        .push(commit.id())
        .map_err(|e| e.message().to_string())?;
    revwalk
        .set_sorting(git2::Sort::TIME)
        .map_err(|e| e.message().to_string())?;

    let search_lower = search.map(|s| s.to_lowercase());
    let mut commits: Vec<CommitInfo> = Vec::new();
    let mut skipped: usize = 0;
    let need = offset + limit + 1; // +1 to determine has_more

    for oid_result in revwalk {
        let oid = oid_result.map_err(|e| e.message().to_string())?;
        if commits.len() >= need {
            break;
        }

        if let Ok(commit_obj) = repo.find_commit(oid) {
            let message = commit_obj.message().unwrap_or("").to_string();
            let author = commit_obj.author().name().unwrap_or("Unknown").to_string();
            let hash = oid.to_string();
            let short_hash = hash[..7.min(hash.len())].to_string();
            let timestamp = commit_obj.time().seconds();

            // Apply search filter if present
            if let Some(ref search_str) = search_lower {
                if !message.to_lowercase().contains(search_str) {
                    continue;
                }
            }

            if skipped < offset {
                skipped += 1;
                continue;
            }

            let first_line = message.lines().next().unwrap_or("").to_string();
            commits.push(CommitInfo {
                hash,
                short_hash,
                message: first_line,
                author,
                timestamp,
            });
        }
    }

    let has_more = commits.len() > limit;
    if has_more {
        commits.truncate(limit);
    }

    Ok((commits, has_more))
}

// ─── Route handlers ──────────────────────────────────────────────────────

/// POST /api/v1/git/clone
pub async fn clone_repo(
    State(state): State<AppState>,
    Json(req): Json<CloneRequest>,
) -> Response {
    if req.git_url.trim().is_empty() {
        return err_response(StatusCode::BAD_REQUEST, "Git URL is required".to_string());
    }

    let git_url = req.git_url.clone();
    let repo_id = uuid::Uuid::new_v4().to_string();
    let repo_id_for_path = repo_id.clone();
    let repo_dir = state.git_cache.repo_dir(&repo_id_for_path);

    let result = tokio::task::spawn_blocking(move || -> Result<(String, Vec<BranchInfo>, String), String> {
        let dir = repo_dir.clone();
        let dir_str = dir.to_str().ok_or("Invalid repo directory path")?.to_string();

        // Ensure parent directory exists
        if let Some(parent) = dir.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("Failed to create directory: {}", e))?;
        }

        Repository::clone(&git_url, &dir_str).map_err(|e| format!("Clone failed: {}", e))?;

        let repo = Repository::open(&dir_str).map_err(|e| format!("Failed to open cloned repo: {}", e))?;

        let (branches, current_branch) = extract_branches(&repo)?;

        Ok((dir_str, branches, current_branch))
    })
    .await;

    match result {
        Ok(Ok((path, branches, current_branch))) => {
            state.git_cache.insert(
                repo_id.clone(),
                crate::git_cache::GitRepo {
                    path,
                    git_url: req.git_url.clone(),
                    current_branch: current_branch.clone(),
                },
            );

            (
                StatusCode::OK,
                Json(CloneResponse {
                    repo_id,
                    branches,
                    current_branch,
                }),
            )
                .into_response()
        }
        Ok(Err(err)) => err_response(StatusCode::INTERNAL_SERVER_ERROR, err),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("Task failed: {}", e)),
    }
}

/// GET /api/v1/git/repos — list all cached repos
pub async fn list_repos(
    State(state): State<AppState>,
) -> Response {
    let repos = state.git_cache.list_all();
    let items: Vec<serde_json::Value> = repos
        .into_iter()
        .map(|(id, repo)| {
            serde_json::json!({
                "repo_id": id,
                "git_url": repo.git_url,
                "current_branch": repo.current_branch,
                "path": repo.path,
            })
        })
        .collect();
    (StatusCode::OK, Json(serde_json::json!({ "repos": items }))).into_response()
}

/// DELETE /api/v1/git/{repo_id} — remove a cached repo and its directory
pub async fn delete_repo(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
) -> Response {
    let repo_info = match state.git_cache.get(&repo_id) {
        Some(info) => info,
        None => return err_response(StatusCode::NOT_FOUND, "Repository not found".to_string()),
    };

    // Remove from cache first, then clean up on disk
    state.git_cache.remove(&repo_id);

    // Try to clean up the directory asynchronously
    let path = repo_info.path.clone();
    tokio::spawn(async move {
        let _ = tokio::fs::remove_dir_all(&path).await;
    });

    (StatusCode::OK, Json(serde_json::json!({ "message": "Repository removed" }))).into_response()
}

/// GET /api/v1/git/{repo_id}/branches
pub async fn list_branches(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
) -> Response {
    let repo_info = match state.git_cache.get(&repo_id) {
        Some(info) => info,
        None => return err_response(StatusCode::NOT_FOUND, "Repository not found".to_string()),
    };

    let path = repo_info.path.clone();
    let result = tokio::task::spawn_blocking(move || -> Result<(Vec<BranchInfo>, String), String> {
        let repo = Repository::open(&path).map_err(|e| e.message().to_string())?;
        extract_branches(&repo)
    })
    .await;

    match result {
        Ok(Ok((branches, current_branch))) => {
            // Update cached current_branch
            state.git_cache.insert(
                repo_id,
                crate::git_cache::GitRepo {
                    current_branch: current_branch.clone(),
                    ..repo_info
                },
            );

            (
                StatusCode::OK,
                Json(BranchesResponse {
                    branches,
                    current_branch,
                }),
            )
                .into_response()
        }
        Ok(Err(err)) => err_response(StatusCode::INTERNAL_SERVER_ERROR, err),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("Task failed: {}", e)),
    }
}

/// POST /api/v1/git/{repo_id}/checkout
pub async fn checkout_branch(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
    Json(req): Json<CheckoutRequest>,
) -> Response {
    let repo_info = match state.git_cache.get(&repo_id) {
        Some(info) => info,
        None => return err_response(StatusCode::NOT_FOUND, "Repository not found".to_string()),
    };

    let path = repo_info.path.clone();
    let branch_name = req.branch.clone();

    let result = tokio::task::spawn_blocking(move || -> Result<String, String> {
        let repo = Repository::open(&path).map_err(|e| e.message().to_string())?;
        let mut checkout_opts = git2::build::CheckoutBuilder::new();
        checkout_opts.force();

        // Check if this is a remote branch (contains a '/' before the name)
        let remote_ref = format!("refs/remotes/{}", branch_name);
        let local_ref = format!("refs/heads/{}", branch_name);

        if repo.find_reference(&local_ref).is_ok() {
            // Local branch exists — simple checkout
            repo.set_head(&local_ref).map_err(|e| format!("Failed to set HEAD: {}", e))?;
            repo.checkout_head(Some(&mut checkout_opts))
                .map_err(|e| format!("Checkout failed: {}", e))?;
            Ok(branch_name.clone())
        } else if repo.find_reference(&remote_ref).is_ok() {
            // Remote branch — create local tracking branch, then checkout
            let local_name = branch_name.split('/').skip(1).collect::<Vec<_>>().join("/");
            let short_local_ref = format!("refs/heads/{}", local_name);

            // Create local branch tracking the remote if it doesn't exist
            if repo.find_reference(&short_local_ref).is_err() {
                let remote_commit = repo.revparse_single(&branch_name)
                    .map_err(|_| format!("Cannot resolve '{}'", branch_name))?;
                let commit = remote_commit.peel_to_commit()
                    .map_err(|_| "Not a commit".to_string())?;
                repo.branch(&local_name, &commit, true)
                    .map_err(|e| format!("Failed to create branch '{}': {}", local_name, e))?;
            }

            repo.set_head(&short_local_ref).map_err(|e| format!("Failed to set HEAD: {}", e))?;
            repo.checkout_head(Some(&mut checkout_opts))
                .map_err(|e| format!("Checkout failed: {}", e))?;
            println!("Created and switched to local branch '{}' tracking '{}'", local_name, branch_name);
            Ok(local_name)
        } else {
            Err(format!("Branch '{}' not found (checked local and remote)", branch_name))
        }
    })
    .await;

    match result {
        Ok(Ok(branch)) => {
            state.git_cache.insert(
                repo_id,
                crate::git_cache::GitRepo {
                    current_branch: branch.clone(),
                    ..repo_info
                },
            );

            (
                StatusCode::OK,
                Json(CheckoutResponse {
                    branch,
                    message: format!("Switched to branch '{}'", req.branch),
                }),
            )
                .into_response()
        }
        Ok(Err(err)) => err_response(StatusCode::BAD_REQUEST, err),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("Task failed: {}", e)),
    }
}

/// GET /api/v1/git/{repo_id}/commits?ref=main&offset=0&limit=50&search=fix
pub async fn list_commits(
    State(state): State<AppState>,
    Path(repo_id): Path<String>,
    Query(params): Query<CommitsParams>,
) -> Response {
    let repo_info = match state.git_cache.get(&repo_id) {
        Some(info) => info,
        None => return err_response(StatusCode::NOT_FOUND, "Repository not found".to_string()),
    };

    let path = repo_info.path.clone();
    let ref_name = params.ref_.unwrap_or_else(|| "HEAD".to_string());
    let offset = params.offset.unwrap_or(0);
    let limit = params.limit.unwrap_or(50).min(200);
    let search = params.search.clone();

    let result = tokio::task::spawn_blocking(move || -> Result<(Vec<CommitInfo>, bool), String> {
        let repo = Repository::open(&path).map_err(|e| e.message().to_string())?;
        walk_commits(&repo, &ref_name, offset, limit, search.as_deref())
    })
    .await;

    match result {
        Ok(Ok((commits, has_more))) => {
            (StatusCode::OK, Json(CommitsResponse { commits, has_more })).into_response()
        }
        Ok(Err(err)) => err_response(StatusCode::BAD_REQUEST, err),
        Err(e) => err_response(StatusCode::INTERNAL_SERVER_ERROR, format!("Task failed: {}", e)),
    }
}
