pub mod aggregation;
pub mod api_error;
pub mod assets;
pub mod config;
pub mod git_cache;
pub mod git_routes;
pub mod routes;
pub mod scan_routes;
pub mod state;
pub mod template_routes;

use anyhow::Result;
use axum::{
    Router,
    routing::{delete, get, post},
};
use codeprism_core::{AggregationFunc, CodePrismConfig, ProjectConfig};
use codeprism_database::Db;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::config::{AppConfig, ProjectAppConfig, SourceConfig, TopNParams, ViewConfig, ViewKind};
use crate::git_routes::{
    checkout_branch, clone_repo, delete_repo, extract_branches, list_branches, list_commits,
    list_repos, pull_branch,
};
use crate::routes::{
    add_local_project, create_project, delete_project, execute_scan, get_findings, get_matches,
    get_scans, get_view, list_unified_projects, static_handler,
};
use crate::scan_routes::{get_scan_job, get_scan_summary};
use crate::state::AppState;

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::get_view,
        crate::routes::get_scans,
        crate::routes::get_config,
        crate::scan_routes::get_scan_job,
        crate::routes::get_trend,
    ),
    components(schemas(
        crate::aggregation::AggregationResult,
        crate::config::AppConfig,
        crate::config::ProjectAppConfig,
        crate::config::ViewConfig,
        crate::config::ViewKind,
        crate::config::SourceConfig,
        crate::config::TopNParams,
        crate::scan_routes::ScanStartedResponse,
        crate::scan_routes::ScanJobResponse,
    ))
)]
struct ApiDoc;

/// Convert a ProjectConfig to a list of ViewConfigs
pub(crate) fn convert_project_views(project: &ProjectConfig) -> Vec<ViewConfig> {
    let mut views = Vec::new();

    for (key, view_def) in &project.aggregation_views {
        match &view_def.func {
            AggregationFunc::TopN {
                analyzer_id,
                tag_filters,
                order,
                ..
            } => {
                let source = SourceConfig {
                    analyzer_id: analyzer_id.clone(),
                    tag_filters: tag_filters.clone(),
                };
                let params = TopNParams {
                    order: order.clone(),
                };
                views.push(ViewConfig {
                    id: key.clone(),
                    title: view_def.title.clone(),
                    tech_stacks: view_def.tech_stacks.clone(),
                    include_children: view_def.include_children,
                    group_by: view_def.group_by.clone(),
                    chart_type: view_def.chart_type.clone(),
                    change_type_mode: view_def.change_type_mode.clone(),
                    width: view_def.width,
                    trend: view_def.trend,
                    detail_view: view_def.detail_view,
                    kind: ViewKind::TopN { source, params },
                });
            }
            AggregationFunc::Sum {
                analyzer_id,
                tag_filters,
            } => {
                let source = SourceConfig {
                    analyzer_id: analyzer_id.clone(),
                    tag_filters: tag_filters.clone(),
                };
                views.push(ViewConfig {
                    id: key.clone(),
                    title: view_def.title.clone(),
                    tech_stacks: view_def.tech_stacks.clone(),
                    include_children: view_def.include_children,
                    group_by: view_def.group_by.clone(),
                    chart_type: view_def.chart_type.clone(),
                    change_type_mode: view_def.change_type_mode.clone(),
                    width: view_def.width,
                    trend: view_def.trend,
                    detail_view: view_def.detail_view,
                    kind: ViewKind::Sum { source },
                });
            }
            AggregationFunc::Avg {
                analyzer_id,
                tag_filters,
            } => {
                let source = SourceConfig {
                    analyzer_id: analyzer_id.clone(),
                    tag_filters: tag_filters.clone(),
                };
                views.push(ViewConfig {
                    id: key.clone(),
                    title: view_def.title.clone(),
                    tech_stacks: view_def.tech_stacks.clone(),
                    include_children: view_def.include_children,
                    group_by: view_def.group_by.clone(),
                    chart_type: view_def.chart_type.clone(),
                    change_type_mode: view_def.change_type_mode.clone(),
                    width: view_def.width,
                    trend: view_def.trend,
                    detail_view: view_def.detail_view,
                    kind: ViewKind::Avg { source },
                });
            }
            AggregationFunc::Min {
                analyzer_id,
                tag_filters,
            } => {
                let source = SourceConfig {
                    analyzer_id: analyzer_id.clone(),
                    tag_filters: tag_filters.clone(),
                };
                views.push(ViewConfig {
                    id: key.clone(),
                    title: view_def.title.clone(),
                    tech_stacks: view_def.tech_stacks.clone(),
                    include_children: view_def.include_children,
                    group_by: view_def.group_by.clone(),
                    chart_type: view_def.chart_type.clone(),
                    change_type_mode: view_def.change_type_mode.clone(),
                    width: view_def.width,
                    trend: view_def.trend,
                    detail_view: view_def.detail_view,
                    kind: ViewKind::Min { source },
                });
            }
            AggregationFunc::Max {
                analyzer_id,
                tag_filters,
            } => {
                let source = SourceConfig {
                    analyzer_id: analyzer_id.clone(),
                    tag_filters: tag_filters.clone(),
                };
                views.push(ViewConfig {
                    id: key.clone(),
                    title: view_def.title.clone(),
                    tech_stacks: view_def.tech_stacks.clone(),
                    include_children: view_def.include_children,
                    group_by: view_def.group_by.clone(),
                    chart_type: view_def.chart_type.clone(),
                    change_type_mode: view_def.change_type_mode.clone(),
                    width: view_def.width,
                    trend: view_def.trend,
                    detail_view: view_def.detail_view,
                    kind: ViewKind::Max { source },
                });
            }
            AggregationFunc::Distribution {
                analyzer_id,
                tag_filters,
                buckets,
            } => {
                let source = SourceConfig {
                    analyzer_id: analyzer_id.clone(),
                    tag_filters: tag_filters.clone(),
                };
                let params = crate::config::DistributionParams {
                    buckets: buckets.clone(),
                };
                views.push(ViewConfig {
                    id: key.clone(),
                    title: view_def.title.clone(),
                    tech_stacks: view_def.tech_stacks.clone(),
                    include_children: view_def.include_children,
                    group_by: view_def.group_by.clone(),
                    chart_type: view_def.chart_type.clone(),
                    change_type_mode: view_def.change_type_mode.clone(),
                    width: view_def.width,
                    trend: view_def.trend,
                    detail_view: view_def.detail_view,
                    kind: ViewKind::Distribution { source, params },
                });
            }
        }
    }

    views
}

pub async fn run_server(
    db: Db,
    core_config: CodePrismConfig,
    config_path: String,
    port: u16,
) -> Result<()> {
    // Convert CodePrismConfig (Core) to AppConfig (Server) with multi-project support
    let projects_config = core_config.get_projects();

    let mut project_app_configs: Vec<ProjectAppConfig> = Vec::new();

    for project in &projects_config {
        let views = convert_project_views(project);

        let mut tech_stacks: Vec<crate::config::TechStackInfo> = project
            .tech_stacks
            .iter()
            .map(|ts| crate::config::TechStackInfo {
                name: ts.name.clone(),
                category: ts.category.clone(),
            })
            .collect();
        tech_stacks.sort_by(|a, b| a.name.cmp(&b.name));

        project_app_configs.push(ProjectAppConfig {
            name: project.name.clone(),
            views,
            tech_stacks,
            columns: project.columns,
        });
    }

    let app_config = AppConfig {
        projects: project_app_configs,
    };

    // Initialize Git cache with persistent storage in the cloned_repos directory
    let cloned_repos_dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("cloned_repos");
    let git_cache = crate::git_cache::GitCache::new(cloned_repos_dir);

    // Pre-populate GitCache with projects that have repo_path in config
    for project in &projects_config {
        if let Some(ref repo_path) = project.repo_path
            && std::path::Path::new(repo_path).exists()
        {
            let already_cached = git_cache
                .list_all()
                .iter()
                .any(|(_, r)| r.path == *repo_path);
            if !already_cached {
                let (_branches, current_branch) = match git2::Repository::open(repo_path) {
                    Ok(repo) => extract_branches(&repo).unwrap_or_default(),
                    Err(_) => (vec![], String::new()),
                };
                let repo_id = uuid::Uuid::new_v4().to_string();
                git_cache.insert(
                    repo_id,
                    crate::git_cache::GitRepo {
                        path: repo_path.clone(),
                        git_url: String::new(),
                        current_branch,
                        project_name: Some(project.name.clone()),
                    },
                );
            }
        }
    }

    // Initialize AppState
    let state = AppState {
        config: Arc::new(RwLock::new(app_config)),
        db,
        core_config: Arc::new(RwLock::new(core_config)),
        git_cache,
        config_path,
    };

    // Setup Router
    let router = Router::new()
        // Config & Projects (listing)
        .route("/api/v1/config", get(crate::routes::get_config))
        .route(
            "/api/v1/config/projects/:project_name",
            get(crate::routes::get_full_project_config).put(crate::routes::update_project_config),
        )
        .route(
            "/api/v1/config/templates",
            get(crate::template_routes::list_templates),
        )
        .route(
            "/api/v1/config/templates/:name",
            get(crate::template_routes::get_template)
                .put(crate::template_routes::upsert_template)
                .delete(crate::template_routes::delete_template),
        )
        .route("/api/v1/config/reload", post(crate::routes::reload_config))
        .route(
            "/api/v1/projects",
            get(crate::routes::list_projects).post(create_project),
        )
        .route("/api/v1/projects/unified", get(list_unified_projects))
        .route("/api/v1/projects/:project_name", delete(delete_project))
        .route("/api/v1/projects/add-local", post(add_local_project))
        // Git operations
        .route("/api/v1/git/repos", get(list_repos))
        .route("/api/v1/git/clone", post(clone_repo))
        .route("/api/v1/git/:repo_id", delete(delete_repo))
        .route("/api/v1/git/:repo_id/branches", get(list_branches))
        .route("/api/v1/git/:repo_id/checkout", post(checkout_branch))
        .route("/api/v1/git/:repo_id/pull", post(pull_branch))
        .route("/api/v1/git/:repo_id/commits", get(list_commits))
        // Scan operations
        .route("/api/v1/scan", post(execute_scan))
        .route("/api/v1/scan-jobs/:id", get(get_scan_job))
        // Project operations (views, scans listing)
        .route("/api/v1/projects/:project_name/scans", get(get_scans))
        .route(
            "/api/v1/projects/:project_name/scans/:scan_id/views/:view_id",
            get(get_view),
        )
        .route(
            "/api/v1/projects/:project_name/scans/:scan_id/summary",
            get(get_scan_summary),
        )
        .route(
            "/api/v1/projects/:project_name/scans/:scan_id/matches",
            get(get_matches),
        )
        .route(
            "/api/v1/projects/:project_name/scans/:scan_id/duplications",
            get(get_findings),
        )
        .route(
            "/api/v1/projects/:project_name/scans/:scan_id/findings",
            get(get_findings),
        )
        // Trend endpoint
        .route(
            "/api/v1/projects/:project_name/trends/:view_id",
            get(crate::routes::get_trend),
        )
        // Swagger UI
        .merge(SwaggerUi::new("/swagger-ui").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .layer(CorsLayer::permissive())
        .fallback(static_handler)
        .with_state(state);

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
