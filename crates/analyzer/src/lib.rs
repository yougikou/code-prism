use async_trait::async_trait;
use codeprism_core::{IntermediateBlock, MatchDetail, MetricEntry, TAG_CATEGORY, TAG_METRIC};
use regex::{Regex, RegexBuilder};
use std::collections::HashMap;

pub trait Analyzer: Send + Sync {
    fn id(&self) -> &str;
    fn analyze(&self, path: &str, content: &str) -> Vec<MetricEntry>;
    /// Extract individual match details (line numbers, matched text, context).
    /// Default returns empty — override in analyzers that support per-match extraction.
    fn extract_matches(&self, _path: &str, _content: &str) -> Vec<MatchDetail> {
        Vec::new()
    }
    /// Restrict to specific scan mode: "snapshot" or "diff". None = all modes.
    fn scan_mode(&self) -> Option<&str> {
        None
    }
    /// Restrict to specific change type: "A", "M", or "D". None = all types.
    fn change_type(&self) -> Option<&str> {
        None
    }
    /// Set per-file context (change_type, scan_mode) before analyze().
    /// Default is no-op — override in analyzers that need per-file context.
    fn set_file_context(&self, _change_type: &str, _scan_mode: &str) {}
    /// If this analyzer supports cross-file processing, return the FileProcessor interface.
    /// Default returns None — override in analyzers that implement FileProcessor.
    fn as_file_processor(&self) -> Option<&dyn FileProcessor> {
        None
    }
}

/// A cross-file analyzer that can extract intermediate blocks per file
/// and perform global aggregation after all files are scanned.
///
/// This trait extends `Analyzer`. Implementations should override
/// `as_file_processor()` on the `Analyzer` impl to return `Some(self)`.
#[async_trait]
pub trait FileProcessor: Analyzer + Send + Sync {
    /// Extract cross-file analysis blocks from a single file.
    /// The pipeline coordinator sets `analyzer_id` and `file_path` on each block
    /// before saving to the database.
    fn extract_blocks(&self, file_path: &str, content: &str) -> Vec<IntermediateBlock>;

    /// Global aggregation callback after all files in the scan have been processed.
    /// Implementations query `intermediate_blocks` for their `analyzer_id`,
    /// perform the aggregation logic, write results to `metrics` and `matches` tables,
    /// and clean up their intermediate data.
    async fn finalize(&self, scan_id: i64, pool: &sqlx::Pool<sqlx::Sqlite>) -> anyhow::Result<()>;
}

mod wasm;
pub use wasm::WasmAnalyzer;

mod script;
pub use script::ScriptAnalyzer;

mod script_cross_file;
pub use script_cross_file::ScriptCrossFileAnalyzer;

// 1. File Count Analyzer
pub struct FileCountAnalyzer;

impl Default for FileCountAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl FileCountAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

impl Analyzer for FileCountAnalyzer {
    fn id(&self) -> &str {
        "file_count"
    }

    fn analyze(&self, _path: &str, _content: &str) -> Vec<MetricEntry> {
        let mut tags = HashMap::new();
        tags.insert(TAG_METRIC.to_string(), "file_count".to_string());
        tags.insert(TAG_CATEGORY.to_string(), "size".to_string());
        vec![MetricEntry {
            analyzer_id: self.id().to_string(),
            tags,
            value: 1.0,
            scope: None,
            tech_stack: None,
        }]
    }
}

// 2. Char Count Analyzer
pub struct CharCountAnalyzer;

impl Default for CharCountAnalyzer {
    fn default() -> Self {
        Self::new()
    }
}

impl CharCountAnalyzer {
    pub fn new() -> Self {
        Self
    }
}

impl Analyzer for CharCountAnalyzer {
    fn id(&self) -> &str {
        "char_count"
    }

    fn analyze(&self, _path: &str, content: &str) -> Vec<MetricEntry> {
        let mut tags = HashMap::new();
        tags.insert(TAG_METRIC.to_string(), "char_count".to_string());
        tags.insert(TAG_CATEGORY.to_string(), "size".to_string());
        vec![MetricEntry {
            analyzer_id: self.id().to_string(),
            tags,
            value: content.chars().count() as f64,
            scope: None,
            tech_stack: None,
        }]
    }
}

// 3. Regex Analyzer
// 3. Regex Analyzer
pub struct RegexAnalyzer {
    id: String,
    regex: Regex,
    tags: HashMap<String, String>,
    scan_mode: Option<String>,
    change_type: Option<String>,
}

impl RegexAnalyzer {
    pub fn new(
        id: &str,
        pattern: &str,
        tags: HashMap<String, String>,
        scan_mode: Option<String>,
        change_type: Option<String>,
    ) -> Result<Self, regex::Error> {
        let regex = RegexBuilder::new(pattern)
            .dot_matches_new_line(true)
            .build()?;
        Ok(Self {
            id: id.to_string(),
            regex,
            tags,
            scan_mode,
            change_type,
        })
    }
}

impl Analyzer for RegexAnalyzer {
    fn id(&self) -> &str {
        &self.id
    }

    fn analyze(&self, _path: &str, content: &str) -> Vec<MetricEntry> {
        let count = self.regex.find_iter(content).count();
        vec![MetricEntry {
            analyzer_id: self.id.clone(),
            tags: self.tags.clone(),
            value: count as f64,
            scope: None,
            tech_stack: None,
        }]
    }

    fn extract_matches(&self, file_path: &str, content: &str) -> Vec<MatchDetail> {
        let lines: Vec<&str> = content.lines().collect();
        self.regex
            .find_iter(content)
            .map(|m| {
                // 1-based line number: count newlines before match start
                let line_number = content[..m.start()].matches('\n').count() as u32 + 1;

                // Column: position within the line (1-based)
                let line_start = content[..m.start()].rfind('\n').map(|i| i + 1).unwrap_or(0);
                let column_start = (m.start() - line_start + 1) as u32;
                let column_end = column_start + m.len() as u32;

                // Context: previous and next line (index as 0-based)
                let line_idx = (line_number - 1) as usize;
                let context_before = if line_idx > 0 {
                    Some(lines[line_idx - 1].trim().to_string())
                } else {
                    None
                };
                let context_after = if line_idx + 1 < lines.len() {
                    Some(lines[line_idx + 1].trim().to_string())
                } else {
                    None
                };

                MatchDetail {
                    file_path: file_path.to_string(),
                    line_number,
                    line_end: Some(line_number),
                    column_start: Some(column_start),
                    column_end: Some(column_end),
                    matched_text: m.as_str().to_string(),
                    side: None,
                    context_before,
                    context_after,
                    analyzer_id: self.id.clone(),
                }
            })
            .collect()
    }

    fn scan_mode(&self) -> Option<&str> {
        self.scan_mode.as_deref()
    }

    fn change_type(&self) -> Option<&str> {
        self.change_type.as_deref()
    }
}
