use crate::aggregation::{AggregationResult, TopNAggregator, ViewFilters};
use crate::config::{AppConfig, ViewConfig, ViewKind};
pub use crate::state::AppState;
use axum::{
    extract::{Json as AxumJson, Path, Query, State},
    http::StatusCode,
    response::{IntoResponse, Json, Response},
};
use codeprism_core::MatchDetail;
use serde::{Deserialize, Serialize};
use serde_json;

use crate::scan_routes::{ScanJobHandle, ScanStartedResponse};
use codeprism_scanner::Scanner;

// Route macro ViewFilters usage might need IntoParams available?
// Actually ViewFilters derives IntoParams.
// "params(..., ViewFilters)" usage needs ToSchema? ToParams?
// Utoipa: params(..., ViewFilters) works if ViewFilters implements IntoParams.

#[derive(Serialize, utoipa::ToSchema)]
pub struct ProjectInfo {
    pub id: i64,
    pub name: String,
    pub repo_path: String,
    pub created_at: String,
    pub scan_modes: Vec<String>,
    pub total_scans: i64,
    pub last_scan_time: Option<String>,
}

// ── Unified Project Info ─────────────────────────────────────────────

#[derive(Serialize)]
pub struct UnifiedProjectInfo {
    pub name: String,
    pub has_config: bool,
    pub config_repo_path: Option<String>,
    pub has_cached_repo: bool,
    pub cached_repo_id: Option<String>,
    pub cached_repo_branch: Option<String>,
    pub total_scans: i64,
    pub last_scan_time: Option<String>,
    pub scan_modes: Vec<String>,
}

/// GET /api/v1/projects/unified — unified project list from config + DB + git cache
pub async fn list_unified_projects(State(state): State<AppState>) -> impl IntoResponse {
    use std::collections::HashMap;

    // 1. Query DB projects with scan stats
    let db_query = r#"
        SELECT p.name,
               COUNT(s.id) as total_scans,
               MAX(s.scan_time) as last_scan_time,
               GROUP_CONCAT(DISTINCT s.scan_mode) as scan_modes
        FROM projects p
        LEFT JOIN scans s ON s.project_id = p.id
        GROUP BY p.name
        ORDER BY p.name
    "#;

    let db_rows = match sqlx::query_as::<_, (String, i64, Option<String>, Option<String>)>(db_query)
        .fetch_all(state.db.pool())
        .await
    {
        Ok(rows) => rows,
        Err(e) => {
            eprintln!("Database Error listing unified projects: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Database Error").into_response();
        }
    };

    // 2. Index git cache repos by project name
    let mut git_repos: HashMap<String, (String, String)> = HashMap::new();
    for (repo_id, repo) in state.git_cache.list_all() {
        if let Some(ref pn) = repo.project_name {
            git_repos.insert(pn.clone(), (repo_id, repo.current_branch));
        }
    }

    // 3. Read config projects
    let core_config = state.core_config.read().unwrap().clone();

    // 4. Build unified map keyed by project name
    let mut project_map: HashMap<String, UnifiedProjectInfo> = HashMap::new();

    // Start with config projects
    for p in &core_config.projects {
        project_map.insert(
            p.name.clone(),
            UnifiedProjectInfo {
                name: p.name.clone(),
                has_config: true,
                config_repo_path: p.repo_path.clone(),
                has_cached_repo: false,
                cached_repo_id: None,
                cached_repo_branch: None,
                total_scans: 0,
                last_scan_time: None,
                scan_modes: vec![],
            },
        );
    }

    // Merge DB scan data
    for (name, total_scans, last_scan_time, scan_modes) in &db_rows {
        let entry = project_map
            .entry(name.clone())
            .or_insert(UnifiedProjectInfo {
                name: name.clone(),
                has_config: false,
                config_repo_path: None,
                has_cached_repo: false,
                cached_repo_id: None,
                cached_repo_branch: None,
                total_scans: 0,
                last_scan_time: None,
                scan_modes: vec![],
            });
        entry.total_scans = *total_scans;
        entry.last_scan_time = last_scan_time.clone();
        entry.scan_modes = scan_modes
            .as_ref()
            .map(|s| s.split(',').map(|m| m.to_string()).collect())
            .unwrap_or_default();
    }

    // Merge git cache
    for (name, (repo_id, branch)) in &git_repos {
        let entry = project_map
            .entry(name.clone())
            .or_insert(UnifiedProjectInfo {
                name: name.clone(),
                has_config: false,
                config_repo_path: None,
                has_cached_repo: false,
                cached_repo_id: Some(repo_id.clone()),
                cached_repo_branch: Some(branch.clone()),
                total_scans: 0,
                last_scan_time: None,
                scan_modes: vec![],
            });
        entry.has_cached_repo = true;
        entry.cached_repo_id = Some(repo_id.clone());
        entry.cached_repo_branch = Some(branch.clone());
    }

    // 5. Return sorted by name
    let mut result: Vec<UnifiedProjectInfo> = project_map.into_values().collect();
    result.sort_by(|a, b| a.name.cmp(&b.name));

    Json(result).into_response()
}

#[derive(Deserialize)]
pub struct CreateProjectRequest {
    pub name: String,
}

/// POST /api/v1/projects — create a bare project config entry
pub async fn create_project(
    State(state): State<AppState>,
    AxumJson(req): AxumJson<CreateProjectRequest>,
) -> impl IntoResponse {
    let name = req.name.trim().to_string();
    if name.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Project name is required"})),
        )
            .into_response();
    }

    let yaml_content = match std::fs::read_to_string(&state.config_path) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to read config: {}", e)})),
            )
                .into_response();
        }
    };

    let mut core_config: codeprism_core::CodePrismConfig = match serde_yaml::from_str(&yaml_content)
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to parse config: {}", e)})),
            )
                .into_response();
        }
    };

    // Idempotent: if project already exists in config, just return OK
    if core_config.projects.iter().any(|p| p.name == name) {
        return Json(serde_json::json!({"status": "ok", "message": "Project already exists"}))
            .into_response();
    }

    core_config.projects.push(codeprism_core::ProjectConfig {
        name: name.clone(),
        ..Default::default()
    });

    // Atomic write: tmp + rename
    let yaml_str = match serde_yaml::to_string(&core_config) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to serialize config: {}", e)})),
            )
                .into_response();
        }
    };
    let tmp_path = format!("{}.tmp", state.config_path);
    if let Err(e) = std::fs::write(&tmp_path, &yaml_str) {
        let _ = std::fs::remove_file(&tmp_path);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to write config: {}", e)})),
        )
            .into_response();
    }
    if let Err(e) = std::fs::rename(&tmp_path, &state.config_path) {
        let _ = std::fs::remove_file(&tmp_path);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to save config: {}", e)})),
        )
            .into_response();
    }

    // Rebuild in-memory state (same pattern as update_project_config)
    *state.core_config.write().unwrap() = core_config;
    let projects_config = state.core_config.read().unwrap().get_projects();
    let mut project_app_configs = Vec::new();
    for project in &projects_config {
        let views = crate::convert_project_views(project);
        let mut tech_stacks: Vec<crate::config::TechStackInfo> = project
            .tech_stacks
            .iter()
            .map(|ts| crate::config::TechStackInfo {
                name: ts.name.clone(),
                category: ts.category.clone(),
            })
            .collect();
        tech_stacks.sort_by(|a, b| a.name.cmp(&b.name));
        project_app_configs.push(crate::config::ProjectAppConfig {
            name: project.name.clone(),
            views,
            tech_stacks,
            columns: project.columns,
        });
    }
    *state.config.write().unwrap() = crate::config::AppConfig {
        projects: project_app_configs,
    };

    Json(serde_json::json!({"status": "ok", "message": format!("Project '{}' created", name)}))
        .into_response()
}

/// DELETE /api/v1/projects/{project_name} — full project deletion (config + DB + repo files)
pub async fn delete_project(
    State(state): State<AppState>,
    Path(project_name): Path<String>,
) -> impl IntoResponse {
    let pool = state.db.pool();

    // 1. Delete all DB records for this project
    let project_ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM projects WHERE name = ?")
        .bind(&project_name)
        .fetch_all(pool)
        .await
        .unwrap_or_default();

    for pid in &project_ids {
        sqlx::query("DELETE FROM scan_summaries WHERE scan_id IN (SELECT id FROM scans WHERE project_id = ?)")
            .bind(pid).execute(pool).await.ok();
        sqlx::query(
            "DELETE FROM matches WHERE scan_id IN (SELECT id FROM scans WHERE project_id = ?)",
        )
        .bind(pid)
        .execute(pool)
        .await
        .ok();
        sqlx::query(
            "DELETE FROM metrics WHERE scan_id IN (SELECT id FROM scans WHERE project_id = ?)",
        )
        .bind(pid)
        .execute(pool)
        .await
        .ok();
        sqlx::query(
            "DELETE FROM file_changes WHERE scan_id IN (SELECT id FROM scans WHERE project_id = ?)",
        )
        .bind(pid)
        .execute(pool)
        .await
        .ok();
        sqlx::query("DELETE FROM scans WHERE project_id = ?")
            .bind(pid)
            .execute(pool)
            .await
            .ok();
    }
    sqlx::query("DELETE FROM scan_jobs WHERE project_name = ?")
        .bind(&project_name)
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM projects WHERE name = ?")
        .bind(&project_name)
        .execute(pool)
        .await
        .ok();
    sqlx::query(
        "DELETE FROM match_contents \
         WHERE NOT EXISTS (SELECT 1 FROM matches WHERE matches.content_id = match_contents.id) \
           AND NOT EXISTS (SELECT 1 FROM metrics WHERE metrics.content_id = match_contents.id)",
    )
    .execute(pool)
    .await
    .ok();

    // 2. Remove cached repo entries + on-disk directories
    let repos = state.git_cache.list_all();
    for (repo_id, repo) in &repos {
        if repo.project_name.as_deref() == Some(&project_name) {
            let path = repo.path.clone();
            tokio::spawn(async move {
                let _ = tokio::fs::remove_dir_all(&path).await;
            });
            state.git_cache.remove(repo_id);
        }
    }

    // 3. Remove project config from YAML
    let yaml_content = match std::fs::read_to_string(&state.config_path) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to read config: {}", e)})),
            )
                .into_response();
        }
    };
    let mut core_config: codeprism_core::CodePrismConfig = match serde_yaml::from_str(&yaml_content)
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to parse config: {}", e)})),
            )
                .into_response();
        }
    };
    core_config.projects.retain(|p| p.name != project_name);

    // Atomic write
    let yaml_str = match serde_yaml::to_string(&core_config) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to serialize config: {}", e)})),
            )
                .into_response();
        }
    };
    let tmp_path = format!("{}.tmp", state.config_path);
    if let Err(e) = std::fs::write(&tmp_path, &yaml_str) {
        let _ = std::fs::remove_file(&tmp_path);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to write config: {}", e)})),
        )
            .into_response();
    }
    if let Err(e) = std::fs::rename(&tmp_path, &state.config_path) {
        let _ = std::fs::remove_file(&tmp_path);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to save config: {}", e)})),
        )
            .into_response();
    }

    // Rebuild in-memory state
    *state.core_config.write().unwrap() = core_config;
    let projects_config = state.core_config.read().unwrap().get_projects();
    let mut project_app_configs = Vec::new();
    for project in &projects_config {
        let views = crate::convert_project_views(project);
        let mut tech_stacks: Vec<crate::config::TechStackInfo> = project
            .tech_stacks
            .iter()
            .map(|ts| crate::config::TechStackInfo {
                name: ts.name.clone(),
                category: ts.category.clone(),
            })
            .collect();
        tech_stacks.sort_by(|a, b| a.name.cmp(&b.name));
        project_app_configs.push(crate::config::ProjectAppConfig {
            name: project.name.clone(),
            views,
            tech_stacks,
            columns: project.columns,
        });
    }
    *state.config.write().unwrap() = crate::config::AppConfig {
        projects: project_app_configs,
    };

    Json(serde_json::json!({"status": "ok", "message": format!("Project '{}' deleted", project_name)})).into_response()
}

/// GET /api/v1/projects — list all projects that have scan data in the DB
pub async fn list_projects(State(state): State<AppState>) -> impl IntoResponse {
    let query = r#"
        SELECT p.id, p.name, p.repo_path, p.created_at,
               GROUP_CONCAT(DISTINCT s.scan_mode) as scan_modes,
               COUNT(s.id) as total_scans,
               MAX(s.scan_time) as last_scan_time
        FROM projects p
        LEFT JOIN scans s ON s.project_id = p.id
        GROUP BY p.id
        ORDER BY last_scan_time DESC
    "#;

    match sqlx::query_as::<
        _,
        (
            i64,
            String,
            String,
            String,
            Option<String>,
            i64,
            Option<String>,
        ),
    >(query)
    .fetch_all(state.db.pool())
    .await
    {
        Ok(rows) => {
            let projects: Vec<ProjectInfo> = rows
                .into_iter()
                .map(
                    |(id, name, repo_path, created_at, scan_modes, total_scans, last_scan_time)| {
                        ProjectInfo {
                            id,
                            name,
                            repo_path,
                            created_at,
                            scan_modes: scan_modes
                                .map(|s| s.split(',').map(|m| m.to_string()).collect())
                                .unwrap_or_default(),
                            total_scans,
                            last_scan_time,
                        }
                    },
                )
                .collect();
            Json(projects).into_response()
        }
        Err(e) => {
            eprintln!("Database Error listing projects: {}", e);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Database Error",
            )
                .into_response()
        }
    }
}

#[derive(Serialize, utoipa::ToSchema)]
struct ViewResponse {
    view_id: String,
    items: Vec<AggregationResult>,
}

#[derive(Deserialize, utoipa::ToSchema)]
pub struct ScanRequest {
    pub git_url: String,
    pub branch: Option<String>,
    pub commit: Option<String>,
    pub base_commit: Option<String>,
    pub scan_mode: String, // "snapshot" or "diff"
    pub project_name: Option<String>,
    // New fields for multi-step workflow
    pub repo_id: Option<String>,
    pub ref_1: Option<String>,
    pub ref_2: Option<String>,
}

#[derive(Deserialize)]
pub struct AddLocalProjectRequest {
    pub name: String,
    pub path: String,
}

#[derive(Serialize)]
pub struct AddLocalProjectResponse {
    pub repo_id: String,
    pub branches: Vec<crate::git_routes::BranchInfo>,
    pub current_branch: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ScanResponseData {
    pub scan_id: i64,
    pub project_name: String,
    pub status: String,
    pub message: String,
}

// ── Scan Job Tracking ────────────────────────────────────────────────

// ── Scan Job API ─────────────────────────────────────────────────────

#[derive(Serialize)]
pub struct MatchesResponse {
    pub scan_id: i64,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
    pub matches: Vec<MatchDetail>,
}

#[derive(Deserialize)]
pub struct MatchesQuery {
    pub file_path: Option<String>,
    pub analyzer_id: Option<String>,
    pub finding_key: Option<String>,
    /// Deprecated product-specific alias for finding_key.
    pub content_hash: Option<String>,
    pub side: Option<i32>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

/// GET /api/v1/projects/:project_name/scans/:scan_id/matches
pub async fn get_matches(
    State(state): State<AppState>,
    Path((_project_name, scan_id)): Path<(String, i64)>,
    Query(params): Query<MatchesQuery>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(100).min(500);
    let offset = (page - 1) * page_size;

    let finding_key = params.finding_key.as_ref().or(params.content_hash.as_ref());

    if let Some(finding_key) = finding_key {
        // Cross-file finding mode: return every real occurrence for this finding.

        let mut count_sql =
            String::from("SELECT COUNT(*) FROM matches WHERE scan_id = ? AND finding_key = ?");
        let mut rows_sql = String::from(
            "SELECT m.file_path, m.line_start, m.line_end, m.column_start, m.column_end, c.content, \
                    m.side, m.context_before, m.context_after, m.analyzer_id \
             FROM matches m JOIN match_contents c ON c.id = m.content_id \
             WHERE m.scan_id = ? AND m.finding_key = ?",
        );

        if params.analyzer_id.is_some() {
            count_sql.push_str(" AND analyzer_id = ?");
            rows_sql.push_str(" AND analyzer_id = ?");
        }
        rows_sql.push_str(" ORDER BY m.file_path, m.side, m.line_start, m.id LIMIT ? OFFSET ?");

        let total: i64 = {
            let mut q = sqlx::query_scalar(&count_sql)
                .bind(scan_id)
                .bind(finding_key);
            if let Some(ref aid) = params.analyzer_id {
                q = q.bind(aid);
            }
            match q.fetch_optional(state.db.pool()).await {
                Ok(Some(c)) => c,
                _ => 0,
            }
        };

        let mut query = sqlx::query_as::<
            _,
            (
                String,
                i32,
                Option<i32>,
                Option<i32>,
                Option<i32>,
                String,
                Option<i32>,
                Option<String>,
                Option<String>,
                String,
            ),
        >(&rows_sql)
        .bind(scan_id)
        .bind(finding_key);
        if let Some(ref aid) = params.analyzer_id {
            query = query.bind(aid);
        }
        query = query.bind(page_size as i64).bind(offset as i64);

        let matches: Vec<MatchDetail> = match query.fetch_all(state.db.pool()).await {
            Ok(rows) => rows
                .into_iter()
                .map(
                    |(fp, ls, le, cs, ce, content, sd, cb, ca, aid)| MatchDetail {
                        file_path: fp,
                        line_number: ls as u32,
                        line_end: le.map(|v| v as u32),
                        column_start: cs.map(|v| v as u32),
                        column_end: ce.map(|v| v as u32),
                        matched_text: content,
                        side: sd.map(|v| v != 0),
                        context_before: cb,
                        context_after: ca,
                        analyzer_id: aid,
                    },
                )
                .collect(),
            Err(e) => {
                eprintln!("DB error fetching matches: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(MatchesResponse {
                        scan_id,
                        total: 0,
                        page,
                        page_size,
                        matches: vec![],
                    }),
                )
                    .into_response();
            }
        };

        return Json(MatchesResponse {
            scan_id,
            total,
            page,
            page_size,
            matches,
        })
        .into_response();
    }

    // File-path mode (original behavior)
    let file_path = match &params.file_path {
        Some(fp) => fp.clone(),
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(MatchesResponse {
                    scan_id,
                    total: 0,
                    page,
                    page_size,
                    matches: vec![],
                }),
            )
                .into_response();
        }
    };

    let mut count_sql =
        "SELECT COUNT(*) FROM matches WHERE scan_id = ? AND file_path = ?".to_string();
    let mut rows_sql =
        "SELECT m.file_path, m.line_start, m.line_end, m.column_start, m.column_end, c.content, \
                               m.side, m.context_before, m.context_after, m.analyzer_id \
                        FROM matches m JOIN match_contents c ON c.id = m.content_id \
                        WHERE m.scan_id = ? AND m.file_path = ?"
            .to_string();

    if params.analyzer_id.is_some() {
        count_sql.push_str(" AND analyzer_id = ?");
        rows_sql.push_str(" AND analyzer_id = ?");
    }
    if params.side.is_some() {
        count_sql.push_str(" AND side = ?");
        rows_sql.push_str(" AND side = ?");
    }
    rows_sql.push_str(" ORDER BY m.line_start ASC LIMIT ? OFFSET ?");

    let total: i64 = {
        let mut q = sqlx::query_scalar(&count_sql)
            .bind(scan_id)
            .bind(&file_path);
        if let Some(ref aid) = params.analyzer_id {
            q = q.bind(aid);
        }
        if let Some(s) = params.side {
            q = q.bind(s as i64);
        }
        match q.fetch_optional(state.db.pool()).await {
            Ok(Some(c)) => c,
            _ => 0,
        }
    };

    let mut query = sqlx::query_as::<
        _,
        (
            String,
            i32,
            Option<i32>,
            Option<i32>,
            Option<i32>,
            String,
            Option<i32>,
            Option<String>,
            Option<String>,
            String,
        ),
    >(&rows_sql)
    .bind(scan_id)
    .bind(&file_path);
    if let Some(ref aid) = params.analyzer_id {
        query = query.bind(aid);
    }
    if let Some(s) = params.side {
        query = query.bind(s as i64);
    }
    query = query.bind(page_size as i64).bind(offset as i64);

    let matches: Vec<MatchDetail> = match query.fetch_all(state.db.pool()).await {
        Ok(rows) => rows
            .into_iter()
            .map(
                |(fp, ls, le, cs, ce, content, sd, cb, ca, aid)| MatchDetail {
                    file_path: fp,
                    line_number: ls as u32,
                    line_end: le.map(|v| v as u32),
                    column_start: cs.map(|v| v as u32),
                    column_end: ce.map(|v| v as u32),
                    matched_text: content,
                    side: sd.map(|v| v != 0),
                    context_before: cb,
                    context_after: ca,
                    analyzer_id: aid,
                },
            )
            .collect(),
        Err(e) => {
            eprintln!("DB error fetching matches: {}", e);
            vec![]
        }
    };

    Json(MatchesResponse {
        scan_id,
        total,
        page,
        page_size,
        matches,
    })
    .into_response()
}

// ─── Cross-file findings API ────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct FindingFileInfo {
    pub path: String,
    pub scope: Option<String>,
    pub value_before: f64,
    pub value_after: f64,
}

#[derive(Serialize, Clone)]
pub struct FindingInfo {
    pub analyzer_id: String,
    pub finding_key: String,
    pub content: String,
    pub affected_line_count: usize,
    /// Deprecated product-specific alias for finding_key.
    pub content_hash: String,
    /// Deprecated product-specific alias for content.
    pub block_content: String,
    /// Deprecated product-specific alias for affected_line_count.
    pub block_size: i32,
    pub occurrence_count: usize,
    pub affected_file_count: usize,
    pub files: Vec<FindingFileInfo>,
}

#[derive(Serialize)]
pub struct FindingsResponse {
    pub findings: Vec<FindingInfo>,
    /// Deprecated product-specific response alias.
    pub duplications: Vec<FindingInfo>,
    pub total: i64,
    pub page: u32,
    pub page_size: u32,
}

#[derive(Deserialize)]
pub struct FindingsQuery {
    pub analyzer_id: Option<String>,
    pub min_occurrences: Option<u32>,
    pub page: Option<u32>,
    pub page_size: Option<u32>,
}

/// GET /api/v1/projects/:project_name/scans/:scan_id/findings
///
/// Returns paginated cross-file findings. The legacy route and response names
/// remain product aliases; storage and grouping use the generic finding_key.
pub async fn get_findings(
    State(state): State<AppState>,
    Path((_project_name, scan_id)): Path<(String, i64)>,
    Query(params): Query<FindingsQuery>,
) -> impl IntoResponse {
    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).clamp(1, 100);
    let offset = ((page - 1) * page_size) as i64;
    let min_occurrences = params.min_occurrences.unwrap_or(2) as i64;

    let analyzer_filter = params.analyzer_id.as_ref().map(|value| {
        if value.ends_with("_aggregated") {
            value.clone()
        } else {
            format!("{}_aggregated", value)
        }
    });

    let total: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM ( \
         SELECT analyzer_id, finding_key FROM matches \
         WHERE scan_id = ? AND finding_key IS NOT NULL \
           AND (? IS NULL OR analyzer_id = ?) \
         GROUP BY analyzer_id, finding_key \
         HAVING COUNT(DISTINCT file_path) >= ?)",
    )
    .bind(scan_id)
    .bind(analyzer_filter.as_deref())
    .bind(analyzer_filter.as_deref())
    .bind(min_occurrences)
    .fetch_one(state.db.pool())
    .await
    .unwrap_or(0);

    let groups_sql = "SELECT m.finding_key, m.analyzer_id, MIN(m.content_id), COUNT(*), \
                COUNT(DISTINCT m.file_path), MAX(c.line_count) \
         FROM matches m JOIN match_contents c ON c.id = m.content_id \
         WHERE m.scan_id = ? AND m.finding_key IS NOT NULL \
           AND (? IS NULL OR m.analyzer_id = ?) \
         GROUP BY m.analyzer_id, m.finding_key \
         HAVING COUNT(DISTINCT m.file_path) >= ? \
         ORDER BY COUNT(*) DESC \
         LIMIT ? OFFSET ?";

    let groups_query = sqlx::query_as::<_, (String, String, i64, i64, i64, i64)>(groups_sql)
        .bind(scan_id)
        .bind(analyzer_filter.as_deref())
        .bind(analyzer_filter.as_deref())
        .bind(min_occurrences)
        .bind(page_size as i64)
        .bind(offset);

    let groups = match groups_query.fetch_all(state.db.pool()).await {
        Ok(rows) => rows,
        Err(e) => {
            eprintln!("DB error fetching cross-file findings: {}", e);
            return Json(FindingsResponse {
                findings: vec![],
                duplications: vec![],
                total: 0,
                page,
                page_size,
            })
            .into_response();
        }
    };

    if groups.is_empty() {
        return Json(FindingsResponse {
            findings: vec![],
            duplications: vec![],
            total,
            page,
            page_size,
        })
        .into_response();
    }

    let content_ids: Vec<i64> = groups.iter().map(|(_, _, id, _, _, _)| *id).collect();
    let content_placeholders: String = (0..content_ids.len())
        .map(|i| {
            if i == 0 {
                "?".to_string()
            } else {
                ", ?".to_string()
            }
        })
        .collect();
    let content_sql = format!(
        "SELECT id, content FROM match_contents WHERE id IN ({})",
        content_placeholders
    );
    let mut content_query = sqlx::query_as::<_, (i64, String)>(&content_sql);
    for content_id in &content_ids {
        content_query = content_query.bind(content_id);
    }
    let content_by_id: std::collections::HashMap<i64, String> = content_query
        .fetch_all(state.db.pool())
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();

    let finding_keys: Vec<String> = groups
        .iter()
        .map(|(key, _, _, _, _, _)| key.clone())
        .collect();
    let key_placeholders: String = (0..finding_keys.len())
        .map(|i| {
            if i == 0 {
                "?".to_string()
            } else {
                ", ?".to_string()
            }
        })
        .collect();
    let file_sql = format!(
        "SELECT analyzer_id, finding_key, file_path, MIN(line_start), MAX(line_end), \
                MAX(CASE WHEN side = 0 THEN 1 ELSE 0 END), \
                MAX(CASE WHEN side = 1 OR side IS NULL THEN 1 ELSE 0 END) \
         FROM matches WHERE scan_id = ? AND finding_key IN ({}) \
         GROUP BY analyzer_id, finding_key, file_path",
        key_placeholders
    );
    let mut file_query = sqlx::query_as::<
        _,
        (String, String, String, Option<i64>, Option<i64>, i64, i64),
    >(&file_sql)
    .bind(scan_id);
    for key in &finding_keys {
        file_query = file_query.bind(key);
    }
    let file_records = match file_query.fetch_all(state.db.pool()).await {
        Ok(rows) => rows,
        Err(e) => {
            eprintln!("DB error fetching finding occurrences: {}", e);
            return Json(FindingsResponse {
                findings: vec![],
                duplications: vec![],
                total,
                page,
                page_size,
            })
            .into_response();
        }
    };

    let mut files_by_finding: std::collections::HashMap<(String, String), Vec<FindingFileInfo>> =
        std::collections::HashMap::new();
    for (analyzer_id, finding_key, path, line_start, line_end, before, after) in file_records {
        let scope = line_start
            .map(|start| format!("{}:{}-{}", analyzer_id, start, line_end.unwrap_or(start)));
        files_by_finding
            .entry((analyzer_id, finding_key))
            .or_default()
            .push(FindingFileInfo {
                path,
                scope,
                value_before: before as f64,
                value_after: after as f64,
            });
    }

    let findings: Vec<FindingInfo> = groups
        .into_iter()
        .map(
            |(finding_key, analyzer_id, content_id, occurrence_count, file_count, line_count)| {
                let block_content = content_by_id.get(&content_id).cloned().unwrap_or_default();
                let file_infos = files_by_finding
                    .remove(&(analyzer_id.clone(), finding_key.clone()))
                    .unwrap_or_default();

                FindingInfo {
                    analyzer_id,
                    finding_key: finding_key.clone(),
                    content: block_content.clone(),
                    affected_line_count: line_count as usize,
                    content_hash: finding_key,
                    block_content,
                    block_size: line_count as i32,
                    occurrence_count: occurrence_count as usize,
                    affected_file_count: file_count as usize,
                    files: file_infos,
                }
            },
        )
        .collect();

    Json(FindingsResponse {
        duplications: findings.clone(),
        findings,
        total,
        page,
        page_size,
    })
    .into_response()
}

/// Helper to find a view config by ID across all projects
fn find_view_config(config: &AppConfig, view_id: &str) -> Option<ViewConfig> {
    for project in &config.projects {
        if let Some(view) = project.views.iter().find(|v| v.id == view_id) {
            return Some(view.clone());
        }
    }
    None
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_name}/scans/{scan_id}/views/{view_id}",
    params(
        ("project_name" = String, Path, description = "Project Name"),
        ("scan_id" = i64, Path, description = "Scan ID"),
        ("view_id" = String, Path, description = "View ID"),
        ViewFilters
    ),
    responses(
        (status = 200, description = "View result", body = inline(ViewResponse)),
        (status = 404, description = "View not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_view(
    State(state): State<AppState>,
    Path((project_name, scan_id, view_id)): Path<(String, i64, String)>,
    Query(filters): Query<ViewFilters>,
) -> impl IntoResponse {
    // 1. Find the project and its view config
    let app_config = state.config.read().unwrap().clone();
    let view_config = app_config
        .projects
        .iter()
        .find(|p| p.name == project_name)
        .and_then(|p| p.views.iter().find(|v| v.id == view_id))
        .cloned();

    // Fallback: If not found in current project, search all (backward compatibility/graceful)
    let view_config = view_config.or_else(|| find_view_config(&app_config, &view_id));

    if let Some(config) = view_config {
        // 2. Execute Aggregation
        // We currently only support TopN
        match &config.kind {
            ViewKind::TopN { .. } => {
                match TopNAggregator::execute(state.db.pool(), scan_id, &config, &filters).await {
                    Ok(items) => Json(ViewResponse {
                        view_id: view_id.clone(),
                        items,
                    })
                    .into_response(),
                    Err(e) => {
                        eprintln!("Aggregation Error: {}", e);
                        (
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal Server Error",
                        )
                            .into_response()
                    }
                }
            }
            ViewKind::Sum { .. } => {
                match crate::aggregation::SumAggregator::execute(
                    state.db.pool(),
                    scan_id,
                    &config,
                    &filters,
                )
                .await
                {
                    Ok(items) => Json(ViewResponse {
                        view_id: view_id.clone(),
                        items,
                    })
                    .into_response(),
                    Err(e) => {
                        eprintln!("Aggregation Error: {}", e);
                        (
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal Server Error",
                        )
                            .into_response()
                    }
                }
            }
            ViewKind::Avg { .. } => {
                match crate::aggregation::StatAggregator::execute(
                    state.db.pool(),
                    scan_id,
                    &config,
                    &filters,
                    crate::aggregation::StatType::Avg,
                )
                .await
                {
                    Ok(items) => Json(ViewResponse {
                        view_id: view_id.clone(),
                        items,
                    })
                    .into_response(),
                    Err(e) => {
                        eprintln!("Aggregation Error: {}", e);
                        (
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal Server Error",
                        )
                            .into_response()
                    }
                }
            }
            ViewKind::Min { .. } => {
                match crate::aggregation::StatAggregator::execute(
                    state.db.pool(),
                    scan_id,
                    &config,
                    &filters,
                    crate::aggregation::StatType::Min,
                )
                .await
                {
                    Ok(items) => Json(ViewResponse {
                        view_id: view_id.clone(),
                        items,
                    })
                    .into_response(),
                    Err(e) => {
                        eprintln!("Aggregation Error: {}", e);
                        (
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal Server Error",
                        )
                            .into_response()
                    }
                }
            }
            ViewKind::Max { .. } => {
                match crate::aggregation::StatAggregator::execute(
                    state.db.pool(),
                    scan_id,
                    &config,
                    &filters,
                    crate::aggregation::StatType::Max,
                )
                .await
                {
                    Ok(items) => Json(ViewResponse {
                        view_id: view_id.clone(),
                        items,
                    })
                    .into_response(),
                    Err(e) => {
                        eprintln!("Aggregation Error: {}", e);
                        (
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal Server Error",
                        )
                            .into_response()
                    }
                }
            }
            ViewKind::Distribution { .. } => {
                match crate::aggregation::DistributionAggregator::execute(
                    state.db.pool(),
                    scan_id,
                    &config,
                    &filters,
                )
                .await
                {
                    Ok(items) => Json(ViewResponse {
                        view_id: view_id.clone(),
                        items,
                    })
                    .into_response(),
                    Err(e) => {
                        eprintln!("Aggregation Error: {}", e);
                        (
                            axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                            "Internal Server Error",
                        )
                            .into_response()
                    }
                }
            }
        }
    } else {
        (axum::http::StatusCode::NOT_FOUND, "View not found").into_response()
    }
}

// ── Trend / Timeseries endpoint ──

#[derive(serde::Deserialize, utoipa::IntoParams)]
pub struct TrendQuery {
    #[serde(default = "default_trend_mode")]
    pub mode: String,
    #[serde(default = "default_trend_limit")]
    pub limit: u32,
    pub base_commit: Option<String>,
    pub scan_ids: Option<String>,
    pub from: Option<i64>,
    pub to: Option<i64>,
    pub tech_stack: Option<String>,
    pub category: Option<String>,
    pub metric_key: Option<String>,
    pub change_type: Option<String>,
    pub group_by: Option<String>,
}

fn default_trend_mode() -> String {
    "snapshot".to_string()
}

fn default_trend_limit() -> u32 {
    20
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_name}/trends/{view_id}",
    params(
        ("project_name" = String, Path, description = "Project Name"),
        ("view_id" = String, Path, description = "View ID"),
        TrendQuery
    ),
    responses(
        (status = 200, description = "Trend data", body = inline(crate::aggregation::TrendResponse)),
        (status = 404, description = "View not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_trend(
    State(state): State<AppState>,
    Path((project_name, view_id)): Path<(String, String)>,
    Query(query): Query<TrendQuery>,
) -> impl IntoResponse {
    let app_config = state.config.read().unwrap().clone();

    // Find the view config (trend is a field on AggregationView)
    let view_config = app_config
        .projects
        .iter()
        .find(|p| p.name == project_name)
        .and_then(|p| p.views.iter().find(|v| v.id == view_id))
        .cloned();

    let view_config = match view_config {
        Some(v) => v,
        None => return (StatusCode::NOT_FOUND, "View not found").into_response(),
    };

    // When scan_ids is explicitly provided, skip the trend flag check (ad-hoc mode)
    let has_scan_ids = query.scan_ids.is_some();
    if !has_scan_ids && !view_config.trend {
        return (StatusCode::BAD_REQUEST, "View does not have trend enabled").into_response();
    }

    // Parse optional scan_ids (comma-separated)
    let parsed_scan_ids: Option<Vec<i64>> = query.scan_ids.as_ref().map(|s| {
        s.split(',')
            .filter_map(|id| id.trim().parse::<i64>().ok())
            .collect()
    });

    let mode = query.mode.as_str();
    let limit = query.limit.clamp(1, 100);

    let view_filters = ViewFilters {
        tech_stack: query.tech_stack.as_deref().map(String::from),
        category: query.category.as_deref().map(String::from),
        metric_key: query.metric_key.as_deref().map(String::from),
        change_type: query.change_type.as_deref().map(String::from),
        group_by: query.group_by.as_deref().map(String::from),
    };

    match crate::aggregation::TrendAggregator::execute(
        state.db.pool(),
        crate::aggregation::TrendQuery {
            project_name: &project_name,
            view_config: &view_config,
            mode,
            limit,
            base_commit: query.base_commit.as_deref(),
            scan_ids: parsed_scan_ids.as_deref(),
            from: query.from,
            to: query.to,
            view_filters: &view_filters,
        },
    )
    .await
    {
        Ok(response) => Json(response).into_response(),
        Err(e) => {
            eprintln!("Trend Aggregation Error: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Internal Server Error").into_response()
        }
    }
}

#[derive(serde::Deserialize, utoipa::IntoParams)]
pub struct ScanFilters {
    pub mode: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ScanResponse {
    id: i64,
    commit_hash: String,
    scan_time: String,
    commit_timestamp: Option<i64>,
    base_commit_hash: Option<String>,
    scan_mode: String,
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_name}/scans",
    params(
        ("project_name" = String, Path, description = "Project Name"),
        ScanFilters
    ),
    responses(
        (status = 200, description = "List of scans", body = inline(Vec<ScanResponse>)),
        (status = 404, description = "Project not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_scans(
    State(state): State<AppState>,
    Path(project_name): Path<String>,
    Query(filters): Query<ScanFilters>,
) -> impl IntoResponse {
    let mode = filters.mode.unwrap_or_else(|| "SNAPSHOT".to_string());

    // First, lookup project by name to get its ID
    let project_query = "SELECT id FROM projects WHERE name = ?";
    let project_id: Option<i64> = match sqlx::query_scalar(project_query)
        .bind(&project_name)
        .fetch_optional(state.db.pool())
        .await
    {
        Ok(id) => id,
        Err(e) => {
            eprintln!("Database Error looking up project: {}", e);
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Database Error",
            )
                .into_response();
        }
    };

    let project_id = match project_id {
        Some(id) => id,
        None => {
            return (axum::http::StatusCode::NOT_FOUND, "Project not found").into_response();
        }
    };

    // Query scans from database
    let query = "SELECT id, commit_hash, scan_time, commit_timestamp, base_commit_hash, scan_mode FROM scans WHERE project_id = ? AND scan_mode = ? ORDER BY scan_time DESC";

    match sqlx::query_as::<_, (i64, String, String, Option<i64>, Option<String>, String)>(query)
        .bind(project_id)
        .bind(mode)
        .fetch_all(state.db.pool())
        .await
    {
        Ok(rows) => {
            let scans: Vec<ScanResponse> = rows
                .into_iter()
                .map(|(id, hash, time, ts, base, sm)| ScanResponse {
                    id,
                    commit_hash: hash,
                    scan_time: time,
                    commit_timestamp: ts,
                    base_commit_hash: base,
                    scan_mode: sm,
                })
                .collect();
            Json(scans).into_response()
        }
        Err(e) => {
            eprintln!("Database Error: {}", e);
            (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                "Database Error",
            )
                .into_response()
        }
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/config",
    responses(
        (status = 200, description = "Application Config", body = inline(AppConfig)),
    )
)]
pub async fn get_config(State(state): State<AppState>) -> impl IntoResponse {
    let config = state.config.read().unwrap();
    Json(config.clone()).into_response()
}

/// GET /api/v1/config/projects/{project_name} — returns full project config from core
pub async fn get_full_project_config(
    State(state): State<AppState>,
    Path(project_name): Path<String>,
) -> impl IntoResponse {
    let core_config = state.core_config.read().unwrap();
    match core_config.projects.iter().find(|p| p.name == project_name) {
        Some(config) => Json(config).into_response(),
        None => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": "Project not found"})),
        )
            .into_response(),
    }
}

/// PUT /api/v1/config/projects/{project_name} — update project config and write to YAML
pub async fn update_project_config(
    State(state): State<AppState>,
    Path(project_name): Path<String>,
    AxumJson(mut updated_config): AxumJson<codeprism_core::ProjectConfig>,
) -> impl IntoResponse {
    if updated_config.name != project_name {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Project name in path must match body"})),
        )
            .into_response();
    }

    // Validate no duplicate analyzer names across all categories
    {
        let mut seen = std::collections::HashSet::new();
        let mut duplicates = Vec::new();
        for key in updated_config.custom_regex_analyzers.keys() {
            if !seen.insert(key.clone()) {
                duplicates.push(key.clone());
            }
        }
        for key in updated_config.custom_impl_analyzers.keys() {
            if !seen.insert(key.clone()) {
                duplicates.push(key.clone());
            }
        }
        for key in updated_config.external_analyzers.keys() {
            if !seen.insert(key.clone()) {
                duplicates.push(key.clone());
            }
        }
        if !duplicates.is_empty() {
            return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": format!("Duplicate analyzer names: {}", duplicates.join(", "))}))).into_response();
        }
    }

    // Read current YAML file
    let yaml_content = match std::fs::read_to_string(&state.config_path) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to read config file: {}", e)})),
            )
                .into_response();
        }
    };

    let mut core_config: codeprism_core::CodePrismConfig = match serde_yaml::from_str(&yaml_content)
    {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to parse config file: {}", e)})),
            )
                .into_response();
        }
    };

    // Upsert: replace existing project or append new one
    let pos = core_config
        .projects
        .iter()
        .position(|p| p.name == project_name);

    // Preserve existing repo_path if update doesn't provide one
    if updated_config.repo_path.is_none()
        && let Some(pos) = pos
    {
        updated_config.repo_path = core_config.projects[pos].repo_path.clone();
    }

    if let Some(pos) = pos {
        core_config.projects[pos] = updated_config;
    } else {
        core_config.projects.push(updated_config);
    }

    // Write YAML atomically: tmp file + rename
    let yaml_str = match serde_yaml::to_string(&core_config) {
        Ok(s) => s,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to serialize config: {}", e)})),
            )
                .into_response();
        }
    };

    let tmp_path = format!("{}.tmp", state.config_path);
    if let Err(e) = std::fs::write(&tmp_path, &yaml_str) {
        let _ = std::fs::remove_file(&tmp_path);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to write config: {}", e)})),
        )
            .into_response();
    }
    if let Err(e) = std::fs::rename(&tmp_path, &state.config_path) {
        let _ = std::fs::remove_file(&tmp_path);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to save config: {}", e)})),
        )
            .into_response();
    }

    // Rebuild in-memory AppConfig (UI subset)
    let projects_config = core_config.get_projects();
    let mut project_app_configs = Vec::new();
    for project in &projects_config {
        let views = crate::convert_project_views(project);
        let mut tech_stacks: Vec<crate::config::TechStackInfo> = project
            .tech_stacks
            .iter()
            .map(|ts| crate::config::TechStackInfo {
                name: ts.name.clone(),
                category: ts.category.clone(),
            })
            .collect();
        tech_stacks.sort_by(|a, b| a.name.cmp(&b.name));
        project_app_configs.push(crate::config::ProjectAppConfig {
            name: project.name.clone(),
            views,
            tech_stacks,
            columns: project.columns,
        });
    }
    let new_app_config = crate::config::AppConfig {
        projects: project_app_configs,
    };

    // Update in-memory state
    *state.config.write().unwrap() = new_app_config;
    *state.core_config.write().unwrap() = core_config;

    Json(serde_json::json!({"status": "ok", "message": "Configuration saved successfully"}))
        .into_response()
}

/// POST /api/v1/projects/add-local — register a local git repository as a project
pub async fn add_local_project(
    State(state): State<AppState>,
    AxumJson(req): AxumJson<AddLocalProjectRequest>,
) -> Response {
    if req.name.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Project name is required"})),
        )
            .into_response();
    }
    if req.path.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": "Path is required"})),
        )
            .into_response();
    }

    let repo_path = std::path::Path::new(&req.path);

    // Validate path exists
    if !repo_path.exists() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("Path does not exist: {}", req.path)})),
        )
            .into_response();
    }
    if !repo_path.is_dir() {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("Path is not a directory: {}", req.path)})),
        )
            .into_response();
    }

    // Resolve to canonical/absolute path
    let canonical_path = std::fs::canonicalize(repo_path)
        .unwrap_or_else(|_| repo_path.to_path_buf())
        .to_string_lossy()
        .to_string();

    // Verify it's a valid git repository
    if git2::Repository::open(&canonical_path).is_err() {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": format!("Path is not a valid git repository: {}", req.path)}))).into_response();
    }

    // Open repo and extract branches
    let repo = match git2::Repository::open(&canonical_path) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to open repository: {}", e)})),
            )
                .into_response();
        }
    };
    let (branches, current_branch) = match crate::git_routes::extract_branches(&repo) {
        Ok(v) => v,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to read branches: {}", e)})),
            )
                .into_response();
        }
    };

    // Write project config with repo_path to YAML
    let yaml_content = match std::fs::read_to_string(&state.config_path) {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to read config file"})),
            )
                .into_response();
        }
    };

    let mut core_config: codeprism_core::CodePrismConfig = match serde_yaml::from_str(&yaml_content)
    {
        Ok(c) => c,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to parse config file"})),
            )
                .into_response();
        }
    };

    // Upsert project config with repo_path
    let pos = core_config.projects.iter().position(|p| p.name == req.name);
    if let Some(pos) = pos {
        core_config.projects[pos].repo_path = Some(canonical_path.clone());
    } else {
        core_config.projects.push(codeprism_core::ProjectConfig {
            name: req.name.clone(),
            repo_path: Some(canonical_path.clone()),
            ..Default::default()
        });
    }

    // Atomic write: tmp + rename
    let yaml_str = match serde_yaml::to_string(&core_config) {
        Ok(s) => s,
        Err(_) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "Failed to serialize config"})),
            )
                .into_response();
        }
    };
    let tmp_path = format!("{}.tmp", state.config_path);
    if let Err(e) = std::fs::write(&tmp_path, &yaml_str) {
        let _ = std::fs::remove_file(&tmp_path);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to write config: {}", e)})),
        )
            .into_response();
    }
    if let Err(e) = std::fs::rename(&tmp_path, &state.config_path) {
        let _ = std::fs::remove_file(&tmp_path);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(serde_json::json!({"error": format!("Failed to save config: {}", e)})),
        )
            .into_response();
    }

    // Generate repo_id and add to GitCache
    let repo_id = uuid::Uuid::new_v4().to_string();
    state.git_cache.insert(
        repo_id.clone(),
        crate::git_cache::GitRepo {
            path: canonical_path.clone(),
            git_url: String::new(),
            current_branch: current_branch.clone(),
            project_name: Some(req.name.clone()),
        },
    );

    // Rebuild in-memory state
    *state.core_config.write().unwrap() = core_config;
    let projects_config = state.core_config.read().unwrap().get_projects();
    let mut project_app_configs = Vec::new();
    for project in &projects_config {
        let views = crate::convert_project_views(project);
        let mut tech_stacks: Vec<crate::config::TechStackInfo> = project
            .tech_stacks
            .iter()
            .map(|ts| crate::config::TechStackInfo {
                name: ts.name.clone(),
                category: ts.category.clone(),
            })
            .collect();
        tech_stacks.sort_by(|a, b| a.name.cmp(&b.name));
        project_app_configs.push(crate::config::ProjectAppConfig {
            name: project.name.clone(),
            views,
            tech_stacks,
            columns: project.columns,
        });
    }
    *state.config.write().unwrap() = crate::config::AppConfig {
        projects: project_app_configs,
    };

    (
        StatusCode::OK,
        Json(AddLocalProjectResponse {
            repo_id,
            branches,
            current_branch,
        }),
    )
        .into_response()
}

/// POST /api/v1/config/reload — reload config from YAML file on disk
pub async fn reload_config(State(state): State<AppState>) -> impl IntoResponse {
    let yaml_content = match std::fs::read_to_string(&state.config_path) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to read config: {}", e)})),
            )
                .into_response();
        }
    };

    let core_config: codeprism_core::CodePrismConfig = match serde_yaml::from_str(&yaml_content) {
        Ok(c) => c,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": format!("Failed to parse config: {}", e)})),
            )
                .into_response();
        }
    };

    // Rebuild AppConfig (UI subset)
    let projects_config = core_config.get_projects();
    let mut project_app_configs = Vec::new();
    for project in &projects_config {
        let views = crate::convert_project_views(project);
        let mut tech_stacks: Vec<crate::config::TechStackInfo> = project
            .tech_stacks
            .iter()
            .map(|ts| crate::config::TechStackInfo {
                name: ts.name.clone(),
                category: ts.category.clone(),
            })
            .collect();
        tech_stacks.sort_by(|a, b| a.name.cmp(&b.name));
        project_app_configs.push(crate::config::ProjectAppConfig {
            name: project.name.clone(),
            views,
            tech_stacks,
            columns: project.columns,
        });
    }
    let new_app_config = crate::config::AppConfig {
        projects: project_app_configs,
    };

    // Update in-memory state
    *state.config.write().unwrap() = new_app_config;
    *state.core_config.write().unwrap() = core_config;

    Json(serde_json::json!({"status": "ok", "message": "Configuration reloaded successfully"}))
        .into_response()
}

#[utoipa::path(
    post,
    path = "/api/v1/scan",
    request_body = ScanRequest,
    responses(
        (status = 200, description = "Scan started successfully", body = ScanResponseData),
        (status = 400, description = "Bad request"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn execute_scan(
    State(state): State<AppState>,
    AxumJson(request): AxumJson<ScanRequest>,
) -> impl IntoResponse {
    // Validate request
    if request.scan_mode != "snapshot" && request.scan_mode != "diff" {
        return (
            StatusCode::BAD_REQUEST,
            Json(ScanResponseData {
                scan_id: 0,
                project_name: String::new(),
                status: "error".to_string(),
                message: "scan_mode must be 'snapshot' or 'diff'".to_string(),
            }),
        )
            .into_response();
    }

    let scan_mode = request.scan_mode.clone();
    let project_name = request
        .project_name
        .clone()
        .unwrap_or_else(|| "scanned_project".to_string());

    // Create scan_job record before branching into two flows
    let job_id = match sqlx::query_scalar::<_, i64>(
        "INSERT INTO scan_jobs (project_name, scan_mode) VALUES (?, ?) RETURNING id",
    )
    .bind(&project_name)
    .bind(&scan_mode)
    .fetch_one(state.db.pool())
    .await
    {
        Ok(id) => id,
        Err(e) => {
            eprintln!("Failed to create scan job: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ScanResponseData {
                    scan_id: 0,
                    project_name: String::new(),
                    status: "error".to_string(),
                    message: "Failed to initialize scan job".to_string(),
                }),
            )
                .into_response();
        }
    };

    let job_handle = ScanJobHandle::new(state.db.clone(), job_id);

    // ── Flow 1: repo_id provided — use cached cloned repo ──────────────
    if let Some(ref repo_id) = request.repo_id {
        let repo_info = match state.git_cache.get(repo_id) {
            Some(info) => info,
            None => {
                return (
                    StatusCode::NOT_FOUND,
                    Json(ScanResponseData {
                        scan_id: 0,
                        project_name: String::new(),
                        status: "error".to_string(),
                        message: "Repository not found in cache. Please clone it first."
                            .to_string(),
                    }),
                )
                    .into_response();
            }
        };

        let temp_dir = repo_info.path.clone();
        let ref_1 = request.ref_1.clone().unwrap_or_else(|| "HEAD".to_string());
        let ref_2 = request.ref_2.clone();
        let proj_name = project_name.clone();

        let db = state.db.clone();
        let core_config = state.core_config.read().unwrap().clone();
        let job = job_handle;

        tokio::spawn(async move {
            job.set_running().await;
            let mut scanner = Scanner::with_config(db, core_config);
            scanner.set_scan_job_id(job.job_id());

            let result = if scan_mode == "snapshot" {
                scanner
                    .scan_snapshot(&temp_dir, &proj_name, Some(&ref_1))
                    .await
            } else {
                if let Some(base) = ref_2 {
                    scanner
                        .scan_diff(&temp_dir, &proj_name, &base, &ref_1)
                        .await
                } else {
                    Err(anyhow::anyhow!(
                        "ref_2 is required for diff mode when using cached repo"
                    ))
                }
            };

            match result {
                Ok(scan_id) => {
                    if scanner.completed_with_errors() {
                        job.set_completed_with_errors(scan_id).await;
                    } else {
                        job.set_completed(scan_id).await;
                    }
                    println!("Scan completed. job={}, scan={}", job_id, scan_id);
                }
                Err(e) => {
                    job.set_failed(&e.to_string()).await;
                    eprintln!("Scan error for job {}: {}", job_id, e);
                }
            }
        });

        return (
            StatusCode::OK,
            Json(ScanStartedResponse {
                job_id,
                project_name,
                status: "started".to_string(),
                message: "Scan has been queued and will start shortly".to_string(),
            }),
        )
            .into_response();
    }

    // ── Flow 2: No repo_id — clone fresh (original behavior) ──────────
    if request.git_url.trim().is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ScanResponseData {
                scan_id: 0,
                project_name: String::new(),
                status: "error".to_string(),
                message: "git_url is required when no repo_id is provided".to_string(),
            }),
        )
            .into_response();
    }

    let git_url = request.git_url.clone();
    let branch = request.branch.clone();
    let commit = request.commit.clone();
    let base_commit = request.base_commit.clone();
    let project_name_clone = project_name.clone();

    let result = tokio::task::spawn_blocking(move || {
        let temp_dir = std::env::temp_dir().join(format!("codeprism-{}", uuid::Uuid::new_v4()));
        let temp_dir_str = match temp_dir.to_str() {
            Some(path) => path.to_string(),
            None => return Err("Failed to create temp directory".to_string()),
        };

        match git2::Repository::clone(&git_url, &temp_dir_str) {
            Ok(repo) => {
                if let Some(br) = &branch
                    && let Err(e) = repo.set_head(&format!("refs/heads/{}", br))
                {
                    let _ = std::fs::remove_dir_all(&temp_dir_str);
                    return Err(format!("Failed to checkout branch {}: {}", br, e));
                }
                Ok((temp_dir_str, project_name_clone))
            }
            Err(e) => {
                let _ = std::fs::remove_dir_all(&temp_dir_str);
                Err(format!("Failed to clone repository: {}", e))
            }
        }
    })
    .await;

    match result {
        Ok(Ok((temp_dir, proj_name))) => {
            let db = state.db.clone();
            let core_config = state.core_config.read().unwrap().clone();
            let job = job_handle;

            tokio::spawn(async move {
                job.set_running().await;
                let mut scanner = Scanner::with_config(db, core_config);
                scanner.set_scan_job_id(job.job_id());

                let result = if scan_mode == "snapshot" {
                    let commit_ref = commit.as_deref();
                    scanner
                        .scan_snapshot(&temp_dir, &proj_name, commit_ref)
                        .await
                } else {
                    if let Some(base) = base_commit {
                        let target = commit.as_deref().unwrap_or("HEAD");
                        scanner
                            .scan_diff(&temp_dir, &proj_name, &base, target)
                            .await
                    } else {
                        Err(anyhow::anyhow!("base_commit is required for diff mode"))
                    }
                };

                let _ = std::fs::remove_dir_all(&temp_dir);

                match result {
                    Ok(scan_id) => {
                        if scanner.completed_with_errors() {
                            job.set_completed_with_errors(scan_id).await;
                        } else {
                            job.set_completed(scan_id).await;
                        }
                        println!("Scan completed. job={}, scan={}", job_id, scan_id);
                    }
                    Err(e) => {
                        job.set_failed(&e.to_string()).await;
                        eprintln!("Scan error for job {}: {}", job_id, e);
                    }
                }
            });

            (
                StatusCode::OK,
                Json(ScanStartedResponse {
                    job_id,
                    project_name,
                    status: "started".to_string(),
                    message: "Scan has been queued and will start shortly".to_string(),
                }),
            )
                .into_response()
        }
        _ => (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ScanResponseData {
                scan_id: 0,
                project_name: String::new(),
                status: "error".to_string(),
                message: "Failed to initialize scan".to_string(),
            }),
        )
            .into_response(),
    }
}

pub async fn static_handler(uri: axum::http::Uri) -> impl IntoResponse {
    let path = uri.path().trim_start_matches('/').to_string();

    let path = if path.is_empty() {
        "index.html".to_string()
    } else {
        path
    };

    match crate::assets::FrontendAssets::get(&path) {
        Some(content) => {
            let mime = mime_guess::from_path(&path).first_or_octet_stream();
            axum::response::Response::builder()
                .header(axum::http::header::CONTENT_TYPE, mime.as_ref())
                .body(axum::body::Body::from(content.data))
                .unwrap()
                .into_response()
        }
        None => {
            if path.contains('.') {
                return (axum::http::StatusCode::NOT_FOUND, "404 Not Found").into_response();
            }
            // Fallback to index.html for SPA
            match crate::assets::FrontendAssets::get("index.html") {
                Some(content) => axum::response::Response::builder()
                    .header(axum::http::header::CONTENT_TYPE, "text/html")
                    .body(axum::body::Body::from(content.data))
                    .unwrap()
                    .into_response(),
                None => (axum::http::StatusCode::NOT_FOUND, "404 Not Found").into_response(),
            }
        }
    }
}
