//! Cross-file analysis pipeline coordinator.
//!
//! After all files in a scan have been processed per-file (Phase 1 extraction),
//! this module coordinates the global aggregation Phase 3 (`finalize`) for all
//! `FileProcessor` analyzers.
//!
//! ## Pipeline
//!
//! ```text
//! Phase 1: per-file extract_blocks()  →  intermediate_blocks table
//! Phase 2: (scan loop continues for remaining files)
//! Phase 3: finalize_all()             →  metrics + matches tables, cleanup
//! ```
//!
//! Each `FileProcessor` implementation is responsible for its own aggregation
//! logic. This module simply iterates and dispatches.

use anyhow::Result;
use codeprism_analyzer::FileProcessor;
use std::collections::HashMap;

/// Run `finalize` on all registered cross-file analyzers.
///
/// Called once after all files in a scan have been processed.
/// Each analyzer reads its own `intermediate_blocks`, performs aggregation,
/// writes `metrics`/`matches`, and cleans up its intermediate data.
pub async fn finalize_all(
    scan_id: i64,
    pool: &sqlx::Pool<sqlx::Sqlite>,
    cross_file_analyzers: &HashMap<String, Box<dyn FileProcessor>>,
) -> Result<()> {
    if cross_file_analyzers.is_empty() {
        return Ok(());
    }

    for (name, fp) in cross_file_analyzers {
        println!("Running cross-file aggregation for '{}'...", name);
        fp.finalize(scan_id, pool).await?;
    }

    Ok(())
}
