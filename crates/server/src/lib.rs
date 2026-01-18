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
    paths(crate::routes::get_view),
    components(schemas(crate::aggregation::AggregationResult)) // ViewResponse is internal, try inline or expose it?
    // AggregationResult is public.
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
                // Determine tech stack (first one or specific string if singular)
                // AppConfig expects singular String tech_stack.
                // AggregationView has Vec<String> tech_stacks.
                // Simple logic: Take first one or "Global"?
                let _tech_stack = view_def
                    .tech_stacks
                    .first()
                    .cloned()
                    .unwrap_or_else(|| "Global".to_string());

                // AggregationFunc::TopN has optional analyzer_id. SourceConfig needs String.
                // If analyzer_id is None, we default to empty string or some placeholder?
                // Or "matches" if implicit regex?
                // Let's use analyzer_id if present, else empty? Or "any"?
                // Aggregator uses it to filter.

                let source = SourceConfig {
                    analyzer_id: analyzer_id.clone().unwrap_or_default(),
                    metric_key: metric_key.clone(),
                };

                let params = TopNParams {
                    limit: *limit as u32,
                };

                // IMPORTANT: The aggregation.rs implementation reads `AppConfig` struct `tech_stack`.
                // BUT it seems `TopNAggregator` also checks `category`.
                // We need to ensure logic aligns.
                // `SourceConfig` matches core `AggregationFunc`.
                // `category` from Core is missing in `SourceConfig`?
                // `crates/server/src/config.rs` SourceConfig definition: { analyzer_id, metric_key }.
                // It does NOT have category!
                // `AggregationView` (Core) has `category`.
                // `TopNAggregator` (Server) logic:
                // SELECT ... WHERE ...
                // if !view_config.tech_stack.is_empty() { query.push_str(" AND category = ?"); }
                // So Server uses `tech_stack` field as `category` filter!
                // This means map `category` (Core) -> `tech_stack` (Server ViewConfig).

                // Wait. `category` in Core is "maintainability".
                // `tech_stack` in Core is "Gosu".
                // If Server uses `tech_stack` field to filter `category` column...
                // Then `ViewConfig.tech_stack` MUST be set to `category` value!

                // Let's verify `TopNAggregator` logic again (Step 2347).
                // "if !view_config.tech_stack.is_empty() { query.push_str(" AND category = ?"); ... bind(&view_config.tech_stack) }"
                // Yes. Server maps `tech_stack` field to `category` column.
                // So we should map `Core::category` -> `Server::tech_stack`.

                let category_val = category.clone().unwrap_or_default();

                views.push(ViewConfig {
                    id: key.clone(),
                    tech_stack: category_val, // Mapping category to tech_stack field for filtering
                    kind: ViewKind::TopN { source, params },
                });
            }
            AggregationFunc::Sum {
                analyzer_id,
                metric_key,
                category,
            } => {
                let category_val = category.clone().unwrap_or_default();
                let source = SourceConfig {
                    analyzer_id: analyzer_id.clone().unwrap_or_default(),
                    metric_key: metric_key.clone(),
                };

                views.push(ViewConfig {
                    id: key.clone(),
                    tech_stack: category_val, // Currently using tech_stack field for category storage
                    kind: ViewKind::Sum { source },
                });
            }
        }
    }

    let app_config = AppConfig { views };

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
