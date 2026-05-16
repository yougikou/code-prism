use codeprism_core::{MetricEntry, TAG_CATEGORY, TAG_METRIC};
use regex::Regex;
use std::collections::HashMap;

pub trait Analyzer: Send + Sync {
    fn id(&self) -> &str;
    fn analyze(&self, path: &str, content: &str) -> Vec<MetricEntry>;
    /// Restrict to specific scan mode: "snapshot" or "diff". None = all modes.
    fn scan_mode(&self) -> Option<&str> {
        None
    }
    /// Restrict to specific change type: "A", "M", or "D". None = all types.
    fn change_type(&self) -> Option<&str> {
        None
    }
}

mod wasm;
pub use wasm::WasmAnalyzer;

mod script;
pub use script::ScriptAnalyzer;

// 1. File Count Analyzer
pub struct FileCountAnalyzer;

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
        let regex = Regex::new(pattern)?;
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

    fn scan_mode(&self) -> Option<&str> {
        self.scan_mode.as_deref()
    }

    fn change_type(&self) -> Option<&str> {
        self.change_type.as_deref()
    }
}
