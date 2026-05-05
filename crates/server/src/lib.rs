pub mod aggregation;
pub mod assets;
pub mod config;
pub mod git_cache;
pub mod git_routes;
pub mod routes;
pub mod template_routes;

use anyhow::Result;
use axum::{Router, routing::{get, post, delete}};
use codeprism_core::{AggregationFunc, CodePrismConfig, ProjectConfig};
use codeprism_database::Db;
use std::path::PathBuf;
use std::sync::{Arc, RwLock};
use tower_http::cors::CorsLayer;
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

use crate::config::{AppConfig, ProjectAppConfig, SourceConfig, TopNParams, ViewConfig, ViewKind};
use crate::routes::{
    AppState, get_scan_summary, get_view, get_scans, static_handler, execute_scan, get_scan_job, add_local_project,
};
use crate::git_routes::{clone_repo, list_branches, checkout_branch, list_commits, list_repos, delete_repo, extract_branches};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::routes::get_view,
        crate::routes::get_scans,
        crate::routes::get_config,
        crate::routes::get_scan_job,
    ),
    components(schemas(
        crate::aggregation::AggregationResult,
        crate::config::AppConfig,
        crate::config::ProjectAppConfig,
        crate::config::ViewConfig,
        crate::config::ViewKind,
        crate::config::SourceConfig,
        crate::config::TopNParams,
        crate::routes::ScanStartedResponse,
        crate::routes::ScanJobResponse,
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

pub async fn run_server(db: Db, core_config: CodePrismConfig, config_path: String, port: u16) -> Result<()> {
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

    // Initialize Git cache with persistent storage in the cloned_repos directory
    let cloned_repos_dir = std::env::current_dir()
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("cloned_repos");
    let git_cache = crate::git_cache::GitCache::new(cloned_repos_dir);

    // Pre-populate GitCache with projects that have repo_path in config
    for project in &projects_config {
        if let Some(ref repo_path) = project.repo_path {
            if std::path::Path::new(repo_path).exists() {
                let already_cached = git_cache.list_all().iter().any(|(_, r)| r.path == *repo_path);
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
        .route("/api/v1/config/projects/:project_name", get(crate::routes::get_full_project_config).put(crate::routes::update_project_config))
        .route("/api/v1/config/templates", get(crate::template_routes::list_templates))
        .route("/api/v1/config/templates/:name", get(crate::template_routes::get_template).put(crate::template_routes::upsert_template).delete(crate::template_routes::delete_template))
        .route("/api/v1/config/reload", post(crate::routes::reload_config))
        .route("/api/v1/projects", get(crate::routes::list_projects))
        .route("/api/v1/projects/add-local", post(add_local_project))
        // Git operations
        .route("/api/v1/git/repos", get(list_repos))
        .route("/api/v1/git/clone", post(clone_repo))
        .route("/api/v1/git/:repo_id", delete(delete_repo))
        .route("/api/v1/git/:repo_id/branches", get(list_branches))
        .route("/api/v1/git/:repo_id/checkout", post(checkout_branch))
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
