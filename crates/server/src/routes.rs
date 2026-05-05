use crate::aggregation::{AggregationResult, TopNAggregator, ViewFilters};
use crate::config::{AppConfig, ViewConfig, ViewKind};
use crate::git_cache::GitCache;
use axum::{
    extract::{Path, Query, State, Json as AxumJson},
    response::{IntoResponse, Json},
    http::StatusCode,
};
use codeprism_database::Db;
use serde::{Serialize, Deserialize};
use serde_json;
use std::sync::{Arc, RwLock};
use codeprism_core::CodePrismConfig;
use codeprism_scanner::Scanner;
// Route macro ViewFilters usage might need IntoParams available?
// Actually ViewFilters derives IntoParams.
// "params(..., ViewFilters)" usage needs ToSchema? ToParams?
// Utoipa: params(..., ViewFilters) works if ViewFilters implements IntoParams.

#[derive(Clone)]
pub struct AppState {
    pub(crate) config: Arc<RwLock<AppConfig>>,
    pub(crate) db: Db,
    pub(crate) core_config: Arc<RwLock<CodePrismConfig>>,
    pub(crate) git_cache: GitCache,
    pub config_path: String,
}

impl AppState {
    /// Create a new AppState for use in tests or external builders
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

    match sqlx::query_as::<_, (i64, String, String, String, Option<String>, i64, Option<String>)>(query)
        .fetch_all(state.db.pool())
        .await
    {
        Ok(rows) => {
            let projects: Vec<ProjectInfo> = rows
                .into_iter()
                .map(|(id, name, repo_path, created_at, scan_modes, total_scans, last_scan_time)| {
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
                })
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

#[derive(Serialize, utoipa::ToSchema)]
pub struct ScanResponseData {
    pub scan_id: i64,
    pub project_name: String,
    pub status: String,
    pub message: String,
}

// ── Scan Job Tracking ────────────────────────────────────────────────

#[derive(Serialize, utoipa::ToSchema)]
pub struct ScanStartedResponse {
    pub job_id: i64,
    pub project_name: String,
    pub status: String,
    pub message: String,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ScanJobResponse {
    pub job_id: i64,
    pub project_name: String,
    pub scan_mode: String,
    pub status: String,
    pub progress: u8,
    pub error_message: Option<String>,
    pub scan_id: Option<i64>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Serialize, Deserialize)]
pub struct AnalyzerStatItem {
    pub analyzer_id: String,
    pub files_analyzed: i64,
    pub execution_errors: i64,
    pub error_details: Vec<String>,
}

#[derive(Serialize)]
pub struct ScanSummaryResponse {
    pub scan_id: i64,
    pub total_files_scanned: i64,
    pub total_analyzers_loaded: i64,
    pub total_analyzers_executed: i64,
    pub total_analyzer_executions: i64,
    pub total_errors: i64,
    pub load_errors: Vec<String>,
    pub analyzer_stats: Vec<AnalyzerStatItem>,
}

#[derive(Clone)]
pub struct ScanJobHandle {
    db: Db,
    job_id: i64,
}

impl ScanJobHandle {
    pub fn new(db: Db, job_id: i64) -> Self {
        Self { db, job_id }
    }

    pub async fn set_running(&self) {
        self.set_status("running", 10).await;
    }

    pub async fn set_completed(&self, scan_id: i64) {
        sqlx::query(
            "UPDATE scan_jobs SET status = 'completed', progress = 100, scan_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(scan_id)
        .bind(self.job_id)
        .execute(self.db.pool())
        .await
        .ok();
    }

    pub async fn set_failed(&self, error: &str) {
        sqlx::query(
            "UPDATE scan_jobs SET status = 'failed', progress = 100, error_message = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(error)
        .bind(self.job_id)
        .execute(self.db.pool())
        .await
        .ok();
    }

    async fn set_status(&self, status: &str, progress: u8) {
        sqlx::query(
            "UPDATE scan_jobs SET status = ?, progress = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
        )
        .bind(status)
        .bind(progress as i32)
        .bind(self.job_id)
        .execute(self.db.pool())
        .await
        .ok();
    }
}

// ── Scan Job API ─────────────────────────────────────────────────────

#[utoipa::path(
    get,
    path = "/api/v1/scan-jobs/{job_id}",
    params(
        ("job_id" = i64, Path, description = "Scan Job ID"),
    ),
    responses(
        (status = 200, description = "Scan job status", body = ScanJobResponse),
        (status = 404, description = "Job not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_scan_job(
    State(state): State<AppState>,
    Path(job_id): Path<i64>,
) -> impl IntoResponse {
    match sqlx::query_as::<_, (i64, String, String, String, i32, Option<String>, Option<i64>, String, String)>(
        "SELECT id, project_name, scan_mode, status, progress, error_message, scan_id, created_at, updated_at FROM scan_jobs WHERE id = ?"
    )
    .bind(job_id)
    .fetch_optional(state.db.pool())
    .await
    {
        Ok(Some((id, pn, sm, st, pr, em, si, ca, ua))) => {
            Json(ScanJobResponse {
                job_id: id,
                project_name: pn,
                scan_mode: sm,
                status: st,
                progress: pr as u8,
                error_message: em,
                scan_id: si,
                created_at: ca,
                updated_at: ua,
            })
            .into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Job not found").into_response(),
        Err(e) => {
            eprintln!("DB error fetching scan job: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
}

/// GET /api/v1/projects/:project_name/scans/:scan_id/summary
pub async fn get_scan_summary(
    State(state): State<AppState>,
    Path((_project_name, scan_id)): Path<(String, i64)>,
) -> impl IntoResponse {
    let row = sqlx::query_as::<_, (String, i64, i64, i64, i64, i64, String)>(
        "SELECT load_errors, total_files_scanned, total_analyzers_loaded, \
                total_analyzers_executed, total_analyzer_executions, total_errors, \
                analyzer_stats
         FROM scan_summaries WHERE scan_id = ?",
    )
    .bind(scan_id)
    .fetch_optional(state.db.pool())
    .await;

    match row {
        Ok(Some((load_errors_json, files, loaded, executed, executions, errors, stats_json))) => {
            let load_errors: Vec<String> =
                serde_json::from_str(&load_errors_json).unwrap_or_default();
            let analyzer_stats: Vec<AnalyzerStatItem> =
                serde_json::from_str(&stats_json).unwrap_or_default();

            Json(ScanSummaryResponse {
                scan_id,
                total_files_scanned: files,
                total_analyzers_loaded: loaded,
                total_analyzers_executed: executed,
                total_analyzer_executions: executions,
                total_errors: errors,
                load_errors,
                analyzer_stats,
            })
            .into_response()
        }
        Ok(None) => (StatusCode::NOT_FOUND, "Scan summary not found").into_response(),
        Err(e) => {
            eprintln!("DB error fetching scan summary: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Database error").into_response()
        }
    }
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
                match TopNAggregator::execute(&state.db.pool(), scan_id, &config, &filters).await {
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
                    &state.db.pool(),
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
                    &state.db.pool(),
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
                    &state.db.pool(),
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
                    &state.db.pool(),
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
                    &state.db.pool(),
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

#[derive(serde::Deserialize, utoipa::IntoParams)]
pub struct ScanFilters {
    pub mode: Option<String>,
}

#[derive(Serialize, utoipa::ToSchema)]
pub struct ScanResponse {
    id: i64,
    commit_hash: String,
    scan_time: String,
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
    let query = "SELECT id, commit_hash, scan_time FROM scans WHERE project_id = ? AND scan_mode = ? ORDER BY scan_time DESC";

    match sqlx::query_as::<_, (i64, String, String)>(query)
        .bind(project_id)
        .bind(mode)
        .fetch_all(state.db.pool())
        .await
    {
        Ok(rows) => {
            let scans: Vec<ScanResponse> = rows
                .into_iter()
                .map(|(id, hash, time)| ScanResponse {
                    id,
                    commit_hash: hash,
                    scan_time: time,
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
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Project not found"}))).into_response(),
    }
}

/// PUT /api/v1/config/projects/{project_name} — update project config and write to YAML
pub async fn update_project_config(
    State(state): State<AppState>,
    Path(project_name): Path<String>,
    AxumJson(updated_config): AxumJson<codeprism_core::ProjectConfig>,
) -> impl IntoResponse {
    if updated_config.name != project_name {
        return (StatusCode::BAD_REQUEST, Json(serde_json::json!({"error": "Project name in path must match body"}))).into_response();
    }

    // Read current YAML file
    let yaml_content = match std::fs::read_to_string(&state.config_path) {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("Failed to read config file: {}", e)}))).into_response(),
    };

    let mut core_config: codeprism_core::CodePrismConfig = match serde_yaml::from_str(&yaml_content) {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("Failed to parse config file: {}", e)}))).into_response(),
    };

    // Upsert: replace existing project or append new one
    let pos = core_config.projects.iter().position(|p| p.name == project_name);
    if let Some(pos) = pos {
        core_config.projects[pos] = updated_config;
    } else {
        core_config.projects.push(updated_config);
    }

    // Write YAML atomically: tmp file + rename
    let yaml_str = match serde_yaml::to_string(&core_config) {
        Ok(s) => s,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("Failed to serialize config: {}", e)}))).into_response(),
    };

    let tmp_path = format!("{}.tmp", state.config_path);
    if let Err(e) = std::fs::write(&tmp_path, &yaml_str) {
        let _ = std::fs::remove_file(&tmp_path);
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("Failed to write config: {}", e)}))).into_response();
    }
    if let Err(e) = std::fs::rename(&tmp_path, &state.config_path) {
        let _ = std::fs::remove_file(&tmp_path);
        return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("Failed to save config: {}", e)}))).into_response();
    }

    // Rebuild in-memory AppConfig (UI subset)
    let projects_config = core_config.get_projects();
    let mut project_app_configs = Vec::new();
    for project in &projects_config {
        let views = crate::convert_project_views(project);
        let mut tech_stacks: Vec<String> = project.tech_stacks.iter().map(|ts| ts.name.clone()).collect();
        tech_stacks.sort();
        project_app_configs.push(crate::config::ProjectAppConfig {
            name: project.name.clone(),
            views,
            tech_stacks,
        });
    }
    let new_app_config = crate::config::AppConfig { projects: project_app_configs };

    // Update in-memory state
    *state.config.write().unwrap() = new_app_config;
    *state.core_config.write().unwrap() = core_config;

    Json(serde_json::json!({"status": "ok", "message": "Configuration saved successfully"})).into_response()
}

/// POST /api/v1/config/reload — reload config from YAML file on disk
pub async fn reload_config(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let yaml_content = match std::fs::read_to_string(&state.config_path) {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("Failed to read config: {}", e)}))).into_response(),
    };

    let core_config: codeprism_core::CodePrismConfig = match serde_yaml::from_str(&yaml_content) {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("Failed to parse config: {}", e)}))).into_response(),
    };

    // Rebuild AppConfig (UI subset)
    let projects_config = core_config.get_projects();
    let mut project_app_configs = Vec::new();
    for project in &projects_config {
        let views = crate::convert_project_views(project);
        let mut tech_stacks: Vec<String> = project.tech_stacks.iter().map(|ts| ts.name.clone()).collect();
        tech_stacks.sort();
        project_app_configs.push(crate::config::ProjectAppConfig {
            name: project.name.clone(),
            views,
            tech_stacks,
        });
    }
    let new_app_config = crate::config::AppConfig { projects: project_app_configs };

    // Update in-memory state
    *state.config.write().unwrap() = new_app_config;
    *state.core_config.write().unwrap() = core_config;

    Json(serde_json::json!({"status": "ok", "message": "Configuration reloaded successfully"})).into_response()
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
                        message: "Repository not found in cache. Please clone it first.".to_string(),
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
                    job.set_completed(scan_id).await;
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
        let temp_dir = std::env::temp_dir().join(format!("codeprism-{}", uuid::Uuid::new_v4().to_string()));
        let temp_dir_str = match temp_dir.to_str() {
            Some(path) => path.to_string(),
            None => return Err("Failed to create temp directory".to_string()),
        };

        match git2::Repository::clone(&git_url, &temp_dir_str) {
            Ok(repo) => {
                if let Some(br) = &branch {
                    if let Err(e) = repo.set_head(&format!("refs/heads/{}", br)) {
                        let _ = std::fs::remove_dir_all(&temp_dir_str);
                        return Err(format!("Failed to checkout branch {}: {}", br, e));
                    }
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
                        job.set_completed(scan_id).await;
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
