use crate::{api_error::ApiError, state::AppState};
use axum::{
    Json,
    extract::{Path, State},
};
use codeprism_database::Db;
use serde::{Deserialize, Serialize};

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
    pub progress_message: Option<String>,
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
    pub fn job_id(&self) -> i64 {
        self.job_id
    }
    pub async fn set_running(&self) {
        self.set_status("running", 10).await;
    }
    pub async fn set_completed(&self, scan_id: i64) {
        sqlx::query("UPDATE scan_jobs SET status = 'completed', progress = 100, scan_id = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(scan_id).bind(self.job_id).execute(self.db.pool()).await.ok();
    }
    pub async fn set_completed_with_errors(&self, scan_id: i64) {
        sqlx::query("UPDATE scan_jobs SET status = 'completed_with_errors', progress = 100, scan_id = ?, progress_message = 'Scan completed; one or more cross-file analyzers failed', updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(scan_id).bind(self.job_id).execute(self.db.pool()).await.ok();
    }
    pub async fn set_failed(&self, error: &str) {
        sqlx::query("UPDATE scan_jobs SET status = 'failed', progress = 100, error_message = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(error).bind(self.job_id).execute(self.db.pool()).await.ok();
    }
    async fn set_status(&self, status: &str, progress: u8) {
        sqlx::query("UPDATE scan_jobs SET status = ?, progress = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(status).bind(progress as i32).bind(self.job_id).execute(self.db.pool()).await.ok();
    }
}

#[utoipa::path(
    get,
    path = "/api/v1/scan-jobs/{job_id}",
    params(("job_id" = i64, Path, description = "Scan Job ID")),
    responses(
        (status = 200, description = "Scan job status", body = ScanJobResponse),
        (status = 404, description = "Job not found"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_scan_job(
    State(state): State<AppState>,
    Path(job_id): Path<i64>,
) -> Result<Json<ScanJobResponse>, ApiError> {
    let row = sqlx::query_as::<_, (i64, String, String, String, i32, Option<String>, Option<String>, Option<i64>, String, String)>(
        "SELECT id, project_name, scan_mode, status, progress, progress_message, error_message, scan_id, created_at, updated_at FROM scan_jobs WHERE id = ?"
    ).bind(job_id).fetch_optional(state.db.pool()).await.map_err(ApiError::database)?;
    let (
        id,
        project_name,
        scan_mode,
        status,
        progress,
        progress_message,
        error_message,
        scan_id,
        created_at,
        updated_at,
    ) = row.ok_or_else(|| ApiError::not_found("Job not found"))?;
    Ok(Json(ScanJobResponse {
        job_id: id,
        project_name,
        scan_mode,
        status,
        progress: progress as u8,
        progress_message,
        error_message,
        scan_id,
        created_at,
        updated_at,
    }))
}

pub async fn get_scan_summary(
    State(state): State<AppState>,
    Path((_project_name, scan_id)): Path<(String, i64)>,
) -> Result<Json<ScanSummaryResponse>, ApiError> {
    let row = sqlx::query_as::<_, (String, i64, i64, i64, i64, i64, String)>(
        "SELECT load_errors, total_files_scanned, total_analyzers_loaded, total_analyzers_executed, total_analyzer_executions, total_errors, analyzer_stats FROM scan_summaries WHERE scan_id = ?",
    ).bind(scan_id).fetch_optional(state.db.pool()).await.map_err(ApiError::database)?;
    let (load_errors_json, files, loaded, executed, executions, errors, stats_json) =
        row.ok_or_else(|| ApiError::not_found("Scan summary not found"))?;
    Ok(Json(ScanSummaryResponse {
        scan_id,
        total_files_scanned: files,
        total_analyzers_loaded: loaded,
        total_analyzers_executed: executed,
        total_analyzer_executions: executions,
        total_errors: errors,
        load_errors: serde_json::from_str(&load_errors_json).unwrap_or_default(),
        analyzer_stats: serde_json::from_str(&stats_json).unwrap_or_default(),
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn scan_job_handle_preserves_public_status_contract() {
        let db = Db::new("sqlite::memory:").await.unwrap();
        db.migrate().await.unwrap();
        sqlx::query("INSERT INTO scan_jobs (project_name, scan_mode) VALUES ('demo', 'snapshot')")
            .execute(db.pool())
            .await
            .unwrap();
        let job = ScanJobHandle::new(db.clone(), 1);

        job.set_running().await;
        let running: (String, i64) =
            sqlx::query_as("SELECT status, progress FROM scan_jobs WHERE id = 1")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(running, ("running".into(), 10));

        job.set_completed_with_errors(42).await;
        let completed: (String, i64, i64) =
            sqlx::query_as("SELECT status, progress, scan_id FROM scan_jobs WHERE id = 1")
                .fetch_one(db.pool())
                .await
                .unwrap();
        assert_eq!(completed, ("completed_with_errors".into(), 100, 42));
    }
}
