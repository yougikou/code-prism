pub mod aggregation;
pub mod assets;
pub mod config;
pub mod routes;

use anyhow::Result;
use axum::{Router, routing::get};
use codeprism_core::{AggregationFunc, CodePrismConfig};
use codeprism_database::Db;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::config::{AppConfig, SourceConfig, TopNParams, ViewConfig, ViewKind};
use crate::routes::{AppState, get_view, static_handler};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::get_view,
        crate::routes::get_scans,
        crate::routes::get_config
    ),
    components(schemas(
        crate::aggregation::AggregationResult,
        crate::config::AppConfig,
        crate::config::ViewConfig,
        crate::config::ViewKind,
        crate::config::SourceConfig,
        crate::config::TopNParams
    ))
)]
struct ApiDoc;

pub async fn run_server(db: Db, core_config: CodePrismConfig, port: u16) -> Result<()> {
    // 1. Convert CodePrismConfig (Core) to AppConfig (Server)
    // The server modules (routes, aggregation) use AppConfig/ViewConfig.
    // The previous implementation of top_n used a different yaml struct.

    let mut views = Vec::new();

    for (key, view_def) in &core_config.aggregation_views {
        // Map AggregationFunc to ViewKind
        match &view_def.func {
            AggregationFunc::TopN {
                analyzer_id,
                metric_key,
                category,
                limit,
                order: _,
            } => {
                let source = SourceConfig {
                    analyzer_id: analyzer_id.clone().unwrap_or_default(),
                    metric_key: metric_key.clone(),
                };

                let params = TopNParams {
                    limit: *limit as u32,
                };

                // Map tech_stacks and category directly
                let tech_stacks = view_def.tech_stacks.clone();
                let category = category.clone(); // Option<String>
                let include_children = view_def.include_children;
                let group_by = view_def.group_by.clone();
                let chart_type = view_def.chart_type.clone();

                views.push(ViewConfig {
                    id: key.clone(),
                    title: view_def.title.clone(),
                    tech_stacks,
                    category,
                    include_children,
                    group_by,
                    chart_type,
                    kind: ViewKind::TopN { source, params },
                });
            }
            AggregationFunc::Sum {
                analyzer_id,
                metric_key,
                category,
            } => {
                let source = SourceConfig {
                    analyzer_id: analyzer_id.clone().unwrap_or_default(),
                    metric_key: metric_key.clone(),
                };

                let tech_stacks = view_def.tech_stacks.clone();
                let category = category.clone();
                let include_children = view_def.include_children;
                let group_by = view_def.group_by.clone();
                let chart_type = view_def.chart_type.clone();

                views.push(ViewConfig {
                    id: key.clone(),
                    title: view_def.title.clone(),
                    tech_stacks,
                    category,
                    include_children,
                    group_by,
                    chart_type,
                    kind: ViewKind::Sum { source },
                });
            }
        }
    }

    let mut tech_stacks: Vec<String> = core_config
        .tech_stacks
        .iter()
        .map(|ts| ts.name.clone())
        .collect();
    tech_stacks.sort();

    let app_config = AppConfig { views, tech_stacks };

    // 2. Initialize AppState
    let state = AppState {
        config: Arc::new(app_config),
        db,
    };

    // 3. Setup Router
    let router = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route(
            "/api/v1/projects/:project_id/scans/:scan_id/views/:view_id",
            get(get_view),
        )
        .route(
            "/api/v1/projects/:project_id/scans",
            get(crate::routes::get_scans),
        )
        .route("/api/v1/config", get(crate::routes::get_config))
        .fallback(static_handler)
        .with_state(state)
        .layer(CorsLayer::permissive());

    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    println!("Server running on http://0.0.0.0:{}", port);
    println!("Swagger UI: http://0.0.0.0:{}/swagger-ui", port);
    axum::serve(listener, router)
        .with_graceful_shutdown(shutdown_signal())
        .await?;
    Ok(())
}

async fn shutdown_signal() {
    let ctrl_c = async {
        tokio::signal::ctrl_c()
            .await
            .expect("failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
            .expect("failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {},
        _ = terminate => {},
    }
}
