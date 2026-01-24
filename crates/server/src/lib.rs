pub mod aggregation;
pub mod assets;
pub mod config;
pub mod routes;

use anyhow::Result;
use axum::{Router, routing::get};
use codeprism_core::{AggregationFunc, CodePrismConfig, ProjectConfig};
use codeprism_database::Db;
use std::sync::Arc;
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::config::{AppConfig, ProjectAppConfig, SourceConfig, TopNParams, ViewConfig, ViewKind};
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
        crate::config::ProjectAppConfig,
        crate::config::ViewConfig,
        crate::config::ViewKind,
        crate::config::SourceConfig,
        crate::config::TopNParams
    ))
)]
struct ApiDoc;

/// Convert a ProjectConfig to a list of ViewConfigs
fn convert_project_views(project: &ProjectConfig) -> Vec<ViewConfig> {
    let mut views = Vec::new();

    for (key, view_def) in &project.aggregation_views {
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
                    category: category.clone(),
                };
                let params = TopNParams {
                    limit: *limit as u32,
                };
                views.push(ViewConfig {
                    id: key.clone(),
                    title: view_def.title.clone(),
                    tech_stacks: view_def.tech_stacks.clone(),
                    category: category.clone(),
                    include_children: view_def.include_children,
                    group_by: view_def.group_by.clone(),
                    chart_type: view_def.chart_type.clone(),
                    change_type_mode: view_def.change_type_mode.clone(),
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
                    category: category.clone(),
                };
                views.push(ViewConfig {
                    id: key.clone(),
                    title: view_def.title.clone(),
                    tech_stacks: view_def.tech_stacks.clone(),
                    category: category.clone(),
                    include_children: view_def.include_children,
                    group_by: view_def.group_by.clone(),
                    chart_type: view_def.chart_type.clone(),
                    change_type_mode: view_def.change_type_mode.clone(),
                    kind: ViewKind::Sum { source },
                });
            }
            AggregationFunc::Avg {
                analyzer_id,
                metric_key,
                category,
            } => {
                let source = SourceConfig {
                    analyzer_id: analyzer_id.clone().unwrap_or_default(),
                    metric_key: metric_key.clone(),
                    category: category.clone(),
                };
                views.push(ViewConfig {
                    id: key.clone(),
                    title: view_def.title.clone(),
                    tech_stacks: view_def.tech_stacks.clone(),
                    category: category.clone(),
                    include_children: view_def.include_children,
                    group_by: view_def.group_by.clone(),
                    chart_type: view_def.chart_type.clone(),
                    change_type_mode: view_def.change_type_mode.clone(),
                    kind: ViewKind::Avg { source },
                });
            }
            AggregationFunc::Min {
                analyzer_id,
                metric_key,
                category,
            } => {
                let source = SourceConfig {
                    analyzer_id: analyzer_id.clone().unwrap_or_default(),
                    metric_key: metric_key.clone(),
                    category: category.clone(),
                };
                views.push(ViewConfig {
                    id: key.clone(),
                    title: view_def.title.clone(),
                    tech_stacks: view_def.tech_stacks.clone(),
                    category: category.clone(),
                    include_children: view_def.include_children,
                    group_by: view_def.group_by.clone(),
                    chart_type: view_def.chart_type.clone(),
                    change_type_mode: view_def.change_type_mode.clone(),
                    kind: ViewKind::Min { source },
                });
            }
            AggregationFunc::Max {
                analyzer_id,
                metric_key,
                category,
            } => {
                let source = SourceConfig {
                    analyzer_id: analyzer_id.clone().unwrap_or_default(),
                    metric_key: metric_key.clone(),
                    category: category.clone(),
                };
                views.push(ViewConfig {
                    id: key.clone(),
                    title: view_def.title.clone(),
                    tech_stacks: view_def.tech_stacks.clone(),
                    category: category.clone(),
                    include_children: view_def.include_children,
                    group_by: view_def.group_by.clone(),
                    chart_type: view_def.chart_type.clone(),
                    change_type_mode: view_def.change_type_mode.clone(),
                    kind: ViewKind::Max { source },
                });
            }
            AggregationFunc::Distribution {
                analyzer_id,
                metric_key,
                category,
                buckets,
            } => {
                let source = SourceConfig {
                    analyzer_id: analyzer_id.clone().unwrap_or_default(),
                    metric_key: metric_key.clone(),
                    category: category.clone(),
                };
                let params = crate::config::DistributionParams {
                    buckets: buckets.clone(),
                };
                views.push(ViewConfig {
                    id: key.clone(),
                    title: view_def.title.clone(),
                    tech_stacks: view_def.tech_stacks.clone(),
                    category: category.clone(),
                    include_children: view_def.include_children,
                    group_by: view_def.group_by.clone(),
                    chart_type: view_def.chart_type.clone(),
                    change_type_mode: view_def.change_type_mode.clone(),
                    kind: ViewKind::Distribution { source, params },
                });
            }
        }
    }

    views
}

pub async fn run_server(db: Db, core_config: CodePrismConfig, port: u16) -> Result<()> {
    // Convert CodePrismConfig (Core) to AppConfig (Server) with multi-project support
    let projects_config = core_config.get_projects();

    let mut project_app_configs: Vec<ProjectAppConfig> = Vec::new();

    for project in &projects_config {
        let views = convert_project_views(project);

        let mut tech_stacks: Vec<String> = project
            .tech_stacks
            .iter()
            .map(|ts| ts.name.clone())
            .collect();
        tech_stacks.sort();

        project_app_configs.push(ProjectAppConfig {
            name: project.name.clone(),
            views,
            tech_stacks,
        });
    }

    let app_config = AppConfig {
        projects: project_app_configs,
    };

    // Initialize AppState
    let state = AppState {
        config: Arc::new(app_config),
        db,
    };

    // Setup Router
    let router = Router::new()
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .route(
            "/api/v1/projects/:project_name/scans/:scan_id/views/:view_id",
            get(get_view),
        )
        .route(
            "/api/v1/projects/:project_name/scans",
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
