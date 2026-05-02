use axum::{
    extract::{Path, State},
    http::StatusCode,
    response::{IntoResponse, Json},
};
use std::collections::HashMap;
use codeprism_core::ProjectConfig;

use crate::routes::AppState;

/// GET /api/v1/config/templates — list all project templates
pub async fn list_templates(
    State(state): State<AppState>,
) -> impl IntoResponse {
    let core_config = state.core_config.read().unwrap();
    let templates: HashMap<String, ProjectConfig> = core_config.project_templates.clone();
    Json(templates).into_response()
}

/// GET /api/v1/config/templates/:name — get a single template by name
pub async fn get_template(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    let core_config = state.core_config.read().unwrap();
    match core_config.project_templates.get(&name) {
        Some(template) => Json(template).into_response(),
        None => (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Template not found"}))).into_response(),
    }
}

/// PUT /api/v1/config/templates/:name — create or update a project template
pub async fn upsert_template(
    State(state): State<AppState>,
    Path(name): Path<String>,
    Json(template_config): Json<ProjectConfig>,
) -> impl IntoResponse {
    // Read current YAML file
    let yaml_content = match std::fs::read_to_string(&state.config_path) {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("Failed to read config file: {}", e)}))).into_response(),
    };

    let mut core_config: codeprism_core::CodePrismConfig = match serde_yaml::from_str(&yaml_content) {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("Failed to parse config file: {}", e)}))).into_response(),
    };

    // Insert/update template
    core_config.project_templates.insert(name.clone(), template_config);

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

    // Update in-memory core_config
    *state.core_config.write().unwrap() = core_config;

    Json(serde_json::json!({"status": "ok", "message": format!("Template '{}' saved", name)})).into_response()
}

/// DELETE /api/v1/config/templates/:name — delete a project template
pub async fn delete_template(
    State(state): State<AppState>,
    Path(name): Path<String>,
) -> impl IntoResponse {
    // Read current YAML file
    let yaml_content = match std::fs::read_to_string(&state.config_path) {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("Failed to read config file: {}", e)}))).into_response(),
    };

    let mut core_config: codeprism_core::CodePrismConfig = match serde_yaml::from_str(&yaml_content) {
        Ok(c) => c,
        Err(e) => return (StatusCode::INTERNAL_SERVER_ERROR, Json(serde_json::json!({"error": format!("Failed to parse config file: {}", e)}))).into_response(),
    };

    // Remove template
    if core_config.project_templates.remove(&name).is_none() {
        return (StatusCode::NOT_FOUND, Json(serde_json::json!({"error": "Template not found"}))).into_response();
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

    // Update in-memory core_config
    *state.core_config.write().unwrap() = core_config;

    Json(serde_json::json!({"status": "ok", "message": format!("Template '{}' deleted", name)})).into_response()
}
