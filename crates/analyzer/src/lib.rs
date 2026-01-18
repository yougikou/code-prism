use codeprism_core::MetricEntry;
use regex::Regex;

pub trait Analyzer: Send + Sync {
    fn id(&self) -> &str;
    fn analyze(&self, path: &str, content: &str) -> Vec<MetricEntry>;
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
        vec![MetricEntry {
            analyzer_id: self.id().to_string(),
            metric_key: "file_count".to_string(),
            category: Some("size".to_string()),
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
        vec![MetricEntry {
            analyzer_id: self.id().to_string(),
            metric_key: "char_count".to_string(),
            category: Some("size".to_string()),
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
    metric_key: String,
    category: Option<String>,
}

impl RegexAnalyzer {
    pub fn new(
        id: &str,
        pattern: &str,
        metric_key: Option<String>,
        category: Option<String>,
    ) -> Result<Self, regex::Error> {
        let regex = Regex::new(pattern)?;
        Ok(Self {
            id: id.to_string(),
            regex,
            metric_key: metric_key.unwrap_or_else(|| "matches".to_string()),
            category: category.or_else(|| Some("regex".to_string())),
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
            metric_key: self.metric_key.clone(),
            category: self.category.clone(),
            value: count as f64,
            scope: None,
            tech_stack: None,
        }]
    }
}
