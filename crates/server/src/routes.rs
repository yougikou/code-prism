use crate::aggregation::{AggregationResult, TopNAggregator, ViewFilters};
use crate::config::{AppConfig, ViewKind};
use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Json},
};
use codeprism_database::Db;
use serde::Serialize;
use std::sync::Arc;
// Route macro ViewFilters usage might need IntoParams available?
// Actually ViewFilters derives IntoParams.
// "params(..., ViewFilters)" usage needs ToSchema? ToParams?
// Utoipa: params(..., ViewFilters) works if ViewFilters implements IntoParams.

#[derive(Clone)]
pub struct AppState {
    pub(crate) config: Arc<AppConfig>,
    pub(crate) db: Db,
}

#[derive(Serialize, utoipa::ToSchema)]
struct ViewResponse {
    view_id: String,
    items: Vec<AggregationResult>,
}

#[utoipa::path(
    get,
    path = "/api/v1/projects/{project_id}/scans/{scan_id}/views/{view_id}",
    params(
        ("project_id" = i64, Path, description = "Project ID"),
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
    Path((_project_id, scan_id, view_id)): Path<(i64, i64, String)>,
    Query(filters): Query<ViewFilters>,
) -> impl IntoResponse {
    // 1. Find the view config
    let view_config = state.config.views.iter().find(|v| v.id == view_id);

    if let Some(config) = view_config {
        // 2. Execute Aggregation
        // We currently only support TopN
        match &config.kind {
            ViewKind::TopN { .. } => {
                match TopNAggregator::execute(&state.db.pool(), scan_id, config, &filters).await {
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
                    config,
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
