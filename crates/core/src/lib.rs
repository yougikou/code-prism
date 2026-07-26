use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Well-known tag keys
pub const TAG_METRIC: &str = "metric";
pub const TAG_CATEGORY: &str = "category";

/// Stable SHA-256 hash for exact match content stored in `match_contents`.
/// Analyzer-specific normalization belongs in a finding key, not this hash.
pub fn hash_match_content(content: &str) -> String {
    use sha2::Digest;
    hex::encode(sha2::Sha256::digest(content.as_bytes()))
}

/// A single match detail record produced by an analyzer (e.g., a regex match location).
/// Does not carry tags — tag info is available via the analyzer config referenced by `analyzer_id`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MatchDetail {
    pub file_path: String,
    pub line_number: u32,
    #[serde(default)]
    pub line_end: Option<u32>,
    pub column_start: Option<u32>,
    pub column_end: Option<u32>,
    pub matched_text: String,
    pub side: Option<bool>,
    pub context_before: Option<String>,
    pub context_after: Option<String>,
    pub analyzer_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricEntry {
    pub analyzer_id: String,
    /// Flexible key-value tags replacing old metric_key/category
    #[serde(default)]
    pub tags: HashMap<String, String>,
    pub value: f64,
    pub scope: Option<String>,
    pub tech_stack: Option<String>,
}

impl MetricEntry {
    pub fn metric_key(&self) -> Option<&str> {
        self.tags.get(TAG_METRIC).map(|s| s.as_str())
    }
    pub fn category(&self) -> Option<&str> {
        self.tags.get(TAG_CATEGORY).map(|s| s.as_str())
    }
    pub fn tag(&self, key: &str) -> Option<&str> {
        self.tags.get(key).map(|s| s.as_str())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TechStack {
    pub name: String,
    pub extensions: Vec<String>,
    pub analyzers: Vec<String>,
    #[serde(default)]
    pub paths: Vec<String>,
    #[serde(default)]
    pub excludes: Vec<String>,
    #[serde(default)]
    pub category: Option<String>,
}

/// Project-specific configuration (all settings except database_url)
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectConfig {
    pub name: String,
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repo_path: Option<String>,
    #[serde(default)]
    pub tech_stacks: Vec<TechStack>,
    #[serde(default)]
    pub global_excludes: Vec<String>,
    #[serde(default)]
    pub custom_regex_analyzers: HashMap<String, CustomAnalyzerDef>,
    #[serde(default)]
    pub custom_impl_analyzers: HashMap<String, ImplAnalyzerConfig>,
    #[serde(default)]
    pub external_analyzers: HashMap<String, String>,
    #[serde(default = "default_columns")]
    pub columns: u32,
    #[serde(default)]
    pub aggregation_views: indexmap::IndexMap<String, AggregationView>,
    #[serde(default)]
    pub custom_cross_file_analyzers: HashMap<String, CrossFileAnalyzerConfig>,
}

impl Default for ProjectConfig {
    fn default() -> Self {
        Self {
            name: String::default(),
            repo_path: None,
            tech_stacks: Vec::default(),
            global_excludes: Vec::default(),
            custom_regex_analyzers: HashMap::default(),
            custom_impl_analyzers: HashMap::default(),
            external_analyzers: HashMap::default(),
            columns: default_columns(),
            aggregation_views: indexmap::IndexMap::default(),
            custom_cross_file_analyzers: HashMap::default(),
        }
    }
}

/// Root configuration supporting both single-project (legacy) and multi-project formats
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodePrismConfig {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub database_url: Option<String>,

    // Multi-project format: list of project configs
    #[serde(default)]
    pub projects: Vec<ProjectConfig>,

    // Project templates keyed by template name (stored separately from projects)
    #[serde(default)]
    pub project_templates: HashMap<String, ProjectConfig>,

    // Legacy single-project format (for backward compatibility)
    // These fields are merged into a default project if 'projects' is empty.
    // They are skipped when empty to avoid polluting multi-project YAML output.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tech_stacks: Vec<TechStack>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub global_excludes: Vec<String>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub custom_regex_analyzers: HashMap<String, CustomAnalyzerDef>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub custom_impl_analyzers: HashMap<String, ImplAnalyzerConfig>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub external_analyzers: HashMap<String, String>,
    #[serde(default, skip_serializing_if = "indexmap::IndexMap::is_empty")]
    pub aggregation_views: indexmap::IndexMap<String, AggregationView>,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub custom_cross_file_analyzers: HashMap<String, CrossFileAnalyzerConfig>,
}

impl CodePrismConfig {
    /// Get all project configurations.
    /// If using legacy format (no 'projects' list), returns a single default project.
    pub fn get_projects(&self) -> Vec<ProjectConfig> {
        if !self.projects.is_empty() {
            self.projects.clone()
        } else {
            // Legacy format: create a default project from root-level settings
            vec![ProjectConfig {
                name: "default".to_string(),
                repo_path: None,
                tech_stacks: self.tech_stacks.clone(),
                global_excludes: self.global_excludes.clone(),
                custom_regex_analyzers: self.custom_regex_analyzers.clone(),
                custom_impl_analyzers: self.custom_impl_analyzers.clone(),
                external_analyzers: self.external_analyzers.clone(),
                columns: 2,
                aggregation_views: self.aggregation_views.clone(),
                custom_cross_file_analyzers: self.custom_cross_file_analyzers.clone(),
            }]
        }
    }

    /// Get project config by name
    pub fn get_project(&self, name: &str) -> Option<ProjectConfig> {
        self.get_projects().into_iter().find(|p| p.name == name)
    }

    /// Get the first/default project config (for backward compatibility)
    pub fn get_default_project(&self) -> ProjectConfig {
        self.get_projects().into_iter().next().unwrap_or_default()
    }

    /// Get a project template by name
    pub fn get_template(&self, name: &str) -> Option<ProjectConfig> {
        self.project_templates.get(name).cloned()
    }

    /// List all project template names
    pub fn list_templates(&self) -> Vec<String> {
        self.project_templates.keys().cloned().collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CustomAnalyzerDef {
    Pattern(String),
    Config {
        pattern: String,
        #[serde(default = "default_metric_key")]
        metric_key: String,
        #[serde(default)]
        category: Option<String>,
        /// Human-readable description of the analyzer's purpose
        #[serde(default)]
        description: Option<String>,
        /// Arbitrary key-value tags attached to analyzer results
        #[serde(default)]
        tags: HashMap<String, String>,
        /// Scan modes this analyzer applies to: "all" (default), "snapshot", "diff"
        #[serde(default)]
        scan_mode: Option<String>,
        /// Change types this analyzer applies to: "all" (default), "A", "M", "D"
        #[serde(default)]
        change_type: Option<String>,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImplAnalyzerConfig {
    pub metric_key: Option<String>,
    pub category: Option<String>,
    /// Human-readable description of the analyzer's purpose
    #[serde(default)]
    pub description: Option<String>,
    /// Override or add tags for analyzer results (merged on top of script output)
    #[serde(default)]
    pub tags: HashMap<String, String>,
    /// Scan modes this analyzer applies to: "all" (default), "snapshot", "diff"
    #[serde(default)]
    pub scan_mode: Option<String>,
    /// Change types this analyzer applies to: "all" (default), "A", "M", "D"
    #[serde(default)]
    pub change_type: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
#[derive(Default)]
pub enum SortOrder {
    Asc,
    #[default]
    Desc,
}

pub fn deserialize_string_or_vec<'de, D>(deserializer: D) -> Result<Vec<String>, D::Error>
where
    D: serde::Deserializer<'de>,
{
    use serde::de;
    struct StringOrVec;
    impl<'de> de::Visitor<'de> for StringOrVec {
        type Value = Vec<String>;
        fn expecting(&self, formatter: &mut std::fmt::Formatter) -> std::fmt::Result {
            formatter.write_str("a string or list of strings")
        }
        fn visit_str<E: de::Error>(self, v: &str) -> Result<Vec<String>, E> {
            Ok(vec![v.to_string()])
        }
        fn visit_unit<E: de::Error>(self) -> Result<Vec<String>, E> {
            Ok(vec![])
        }
        fn visit_none<E: de::Error>(self) -> Result<Vec<String>, E> {
            Ok(vec![])
        }
        fn visit_string<E: de::Error>(self, v: String) -> Result<Vec<String>, E> {
            Ok(vec![v])
        }
        fn visit_seq<A: de::SeqAccess<'de>>(self, mut seq: A) -> Result<Vec<String>, A::Error> {
            let mut values = Vec::new();
            while let Some(value) = seq.next_element::<String>()? {
                values.push(value);
            }
            Ok(values)
        }
    }
    deserializer.deserialize_any(StringOrVec)
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AggregationFunc {
    #[serde(rename = "top_n")]
    TopN {
        #[serde(default, deserialize_with = "deserialize_string_or_vec")]
        analyzer_id: Vec<String>,
        #[serde(default)]
        tag_filters: HashMap<String, String>,
        #[serde(default)]
        limit: Option<usize>,
        #[serde(default)]
        order: SortOrder,
    },
    #[serde(rename = "sum")]
    Sum {
        #[serde(default, deserialize_with = "deserialize_string_or_vec")]
        analyzer_id: Vec<String>,
        #[serde(default)]
        tag_filters: HashMap<String, String>,
    },
    #[serde(rename = "avg")]
    Avg {
        #[serde(default, deserialize_with = "deserialize_string_or_vec")]
        analyzer_id: Vec<String>,
        #[serde(default)]
        tag_filters: HashMap<String, String>,
    },
    #[serde(rename = "min")]
    Min {
        #[serde(default, deserialize_with = "deserialize_string_or_vec")]
        analyzer_id: Vec<String>,
        #[serde(default)]
        tag_filters: HashMap<String, String>,
    },
    #[serde(rename = "max")]
    Max {
        #[serde(default, deserialize_with = "deserialize_string_or_vec")]
        analyzer_id: Vec<String>,
        #[serde(default)]
        tag_filters: HashMap<String, String>,
    },
    #[serde(rename = "distribution")]
    Distribution {
        #[serde(default, deserialize_with = "deserialize_string_or_vec")]
        analyzer_id: Vec<String>,
        #[serde(default)]
        tag_filters: HashMap<String, String>,
        buckets: Vec<f64>,
    },
}

impl AggregationFunc {
    /// Return the tag_filters for this aggregation function.
    pub fn effective_tag_filters(&self) -> &HashMap<String, String> {
        match self {
            AggregationFunc::TopN { tag_filters, .. } => tag_filters,
            AggregationFunc::Sum { tag_filters, .. } => tag_filters,
            AggregationFunc::Avg { tag_filters, .. } => tag_filters,
            AggregationFunc::Min { tag_filters, .. } => tag_filters,
            AggregationFunc::Max { tag_filters, .. } => tag_filters,
            AggregationFunc::Distribution { tag_filters, .. } => tag_filters,
        }
    }
}

impl CustomAnalyzerDef {
    /// Resolve tags by merging old metric_key/category with new tags field.
    /// Old fields take precedence.
    pub fn resolve_tags(&self) -> HashMap<String, String> {
        match self {
            CustomAnalyzerDef::Pattern(_) => {
                let mut tags = HashMap::new();
                tags.insert(TAG_METRIC.to_string(), "matches".to_string());
                tags
            }
            CustomAnalyzerDef::Config {
                metric_key,
                category,
                tags,
                ..
            } => {
                let mut result = tags.clone();
                result.insert(TAG_METRIC.to_string(), metric_key.clone());
                if let Some(cat) = category {
                    result.insert(TAG_CATEGORY.to_string(), cat.clone());
                }
                result
            }
        }
    }
}

impl ImplAnalyzerConfig {
    /// Resolve tags by merging old metric_key/category with new tags field.
    pub fn resolve_tags(&self) -> HashMap<String, String> {
        let mut result = self.tags.clone();
        if let Some(mk) = &self.metric_key {
            result.insert(TAG_METRIC.to_string(), mk.clone());
        }
        if let Some(cat) = &self.category {
            result.insert(TAG_CATEGORY.to_string(), cat.clone());
        }
        result
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AggregationView {
    pub title: String,
    #[serde(default)]
    pub tech_stacks: Vec<String>,
    #[serde(default = "default_true")]
    pub include_children: bool,
    #[serde(default)]
    pub group_by: Vec<String>,
    #[serde(default)]
    pub chart_type: Option<String>,
    /// Display mode for change types: "all" (stacked) or "switchable" (A/M/D toggle)
    #[serde(default)]
    pub change_type_mode: Option<String>,
    /// Width in grid columns (1 or 2). Defaults to 1.
    #[serde(default = "default_width")]
    pub width: u32,
    pub func: AggregationFunc,

    // Trend chart field
    #[serde(default)]
    pub trend: bool,
    /// Enable drill-down detail view for cross-file analysis charts.
    /// When true, chart items are clickable and open a detail modal.
    #[serde(default)]
    pub detail_view: bool,
}

fn default_true() -> bool {
    true
}

fn default_width() -> u32 {
    1
}

fn default_columns() -> u32 {
    2
}

fn default_metric_key() -> String {
    "matches".to_string()
}

impl ProjectConfig {
    pub fn get_tech_stack_for_file(&self, path: &str) -> Option<String> {
        let path_obj = std::path::Path::new(path);
        let ext = path_obj
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();

        for stack in &self.tech_stacks {
            // Check extensions first (fast fail)
            if !stack.extensions.iter().any(|e| e == &ext) {
                continue;
            }

            // If paths are defined, must match at least one
            if !stack.paths.is_empty() {
                let mut matched = false;
                for pattern in &stack.paths {
                    if let Ok(glob) = glob::Pattern::new(pattern)
                        && glob.matches_with(
                            path,
                            glob::MatchOptions {
                                case_sensitive: false,
                                require_literal_separator: true,
                                require_literal_leading_dot: false,
                            },
                        )
                    {
                        matched = true;
                        break;
                    }
                }

                if !matched {
                    continue; // Extension matched but path didn't
                }
            }

            // Check Local Excludes
            if !stack.excludes.is_empty() {
                let mut excluded = false;
                for pattern in &stack.excludes {
                    if let Ok(glob) = glob::Pattern::new(pattern)
                        && glob.matches_with(
                            path,
                            glob::MatchOptions {
                                require_literal_separator: true,
                                case_sensitive: false,
                                require_literal_leading_dot: false,
                            },
                        )
                    {
                        excluded = true;
                        break;
                    }
                }

                if excluded {
                    continue;
                }
            }

            return Some(stack.name.clone());
        }

        None
    }

    pub fn is_excluded(&self, path: &str) -> bool {
        // 1. Priority: If matched by any Tech Stack's Explicit include paths, it is NOT excluded.
        let path_obj = std::path::Path::new(path);
        let ext = path_obj
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();

        for stack in &self.tech_stacks {
            if !stack.extensions.iter().any(|e| e == &ext) {
                continue;
            }

            if !stack.paths.is_empty() {
                for pattern in &stack.paths {
                    if let Ok(glob) = glob::Pattern::new(pattern)
                        && glob.matches_with(
                            path,
                            glob::MatchOptions {
                                require_literal_separator: true,
                                case_sensitive: false,
                                require_literal_leading_dot: false,
                            },
                        )
                    {
                        return false; // Explicitly included -> Not excluded
                    }
                }
            }
        }

        // 2. Check Global Excludes (Project-specific in this case)
        for pattern in &self.global_excludes {
            if let Ok(glob) = glob::Pattern::new(pattern)
                && glob.matches_with(
                    path,
                    glob::MatchOptions {
                        require_literal_separator: true,
                        case_sensitive: false,
                        require_literal_leading_dot: false,
                    },
                )
            {
                return true;
            }
        }

        false
    }
}

impl CodePrismConfig {
    pub fn get_tech_stack_for_file(&self, path: &str) -> Option<String> {
        // For general usage, use the root-level tech_stacks or the first project
        if !self.tech_stacks.is_empty() {
            // Legacy/Root-level check (duplicated logic for simplicity/speed)
            // But we actually want to unify this. Let's create a temporary ProjectConfig
            // to reuse the logic.
            let p = ProjectConfig {
                tech_stacks: self.tech_stacks.clone(),
                ..Default::default()
            };
            p.get_tech_stack_for_file(path)
        } else {
            self.get_default_project().get_tech_stack_for_file(path)
        }
    }

    pub fn is_excluded(&self, path: &str) -> bool {
        if !self.global_excludes.is_empty() {
            let p = ProjectConfig {
                tech_stacks: self.tech_stacks.clone(),
                global_excludes: self.global_excludes.clone(),
                ..Default::default()
            };
            p.is_excluded(path)
        } else {
            self.get_default_project().is_excluded(path)
        }
    }

    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, AppError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| AppError::Config(format!("Failed to read config file: {}", e)))?;
        serde_yaml::from_str(&content)
            .map_err(|e| AppError::Config(format!("Failed to parse config file: {}", e)))
    }

    pub fn generate_template() -> String {
        let template = r#"# CodePrism Configuration
# For VS Code autocomplete: add this line at the top of the file (adjust the path):
# yaml-language-server: $schema=schemas/codeprism-config.schema.json

# Database connection URL. Currently supports SQLite.
# To use a custom path, change the URL after "sqlite:" (relative to this config file).
database_url: "sqlite:codeprism.db"

# Project templates can be applied when adding a new project via the UI.
# The key is the template name; the value contains all project settings.
project_templates:
  code-prism:
    name: "code-prism"

    # Files and directories to exclude from analysis
    global_excludes:
      - "*"
      - "**/.git/**"
      - "**/node_modules/**"
      - "**/target/**"

    # Define custom regex analyzers (Name -> Pattern with optional config)
    custom_regex_analyzers:
      log_info_finder:
        pattern: "\\b(?:(?:info|print(?:ln)?)!|print(?:ln)?\\b|console\\.(?:log|info)\\b|(?:\\w*[Ll]og(?:ger|ging)?)\\.info\\b)"
        metric_key: "log_info"
        category: "logging"
        description: "Counts info-level logging calls across languages"
      log_warn_finder:
        pattern: "\\b(?:warn!|console\\.warn\\b|(?:\\w*[Ll]og(?:ger|ging)?)\\.(?:warn|warning)\\b)"
        metric_key: "log_warn"
        category: "logging"
      log_error_finder:
        pattern: "\\b(?:(?:error|eprint(?:ln)?)!|eprint(?:ln)?\\b|console\\.error\\b|(?:\\w*[Ll]og(?:ger|ging)?)\\.(?:error|fatal|exception)\\b)"
        metric_key: "log_error"
        category: "logging"
      log_trace_finder:
        pattern: "\\b(?:trace!|console\\.trace\\b|(?:\\w*[Ll]og(?:ger|ging)?)\\.trace\\b)"
        metric_key: "log_trace"
        category: "logging"
      log_debug_finder:
        pattern: "\\b(?:debug!|console\\.debug\\b|(?:\\w*[Ll]og(?:ger|ging)?)\\.debug\\b)"
        metric_key: "log_debug"
        category: "logging"

    # Python script analyzers — place .py files in custom_analyzers/ dir
    custom_impl_analyzers:
      java_complexity:
        description: "Analyzes cyclomatic complexity of Java methods"
        tags:
          metric: "complexity"
          category: "maintainability"

    # Cross-file duplicate detection analyzers.
    # Place Python scripts in custom_analyzers/ dir (e.g. duplicate_rust_fns.py).
    #
    # Protocol (two-phase):
    #   1. extract — per-file: {"action":"extract","file_path":"...","content":"..."}
    #      → script returns a JSON array of extracted blocks.
    #   2. finalize — global: {"action":"finalize","blocks":[...]}
    #      → script returns {"findings":[...]} with analyzer-defined metrics.
    #
    # Thresholds (min_file_count, min_block_count, etc.) are defined INSIDE each
    # Python script — not in YAML. The framework validates and persists the
    # analyzer-defined findings without adding duplication-specific rules.
    #
    # Each script defines its own default metric_key/category; you can override
    # them via tags below.
    custom_cross_file_analyzers:
      duplicate_rust_fns:
        tags:
          metric: duplicate_block
          category: duplication
      duplicate_python_defs:
        tags:
          metric: duplicate_block
          category: duplication
      duplicate_xml_elements:
        tags:
          metric: duplicate_block
          category: duplication

    # Tech stack classification — files are categorized by extension
    tech_stacks:
      - name: "Rust"
        extensions: ["rs", "toml"]
        analyzers: ["char_count", "log_info_finder", "log_warn_finder", "log_error_finder", "log_trace_finder", "log_debug_finder"]

      - name: "Web"
        extensions: ["js", "ts", "jsx", "tsx", "html", "css"]
        analyzers: ["char_count", "log_info_finder", "log_warn_finder", "log_error_finder", "log_trace_finder", "log_debug_finder"]

      - name: "Python"
        extensions: ["py"]
        analyzers: ["char_count", "log_info_finder", "log_warn_finder", "log_error_finder", "log_trace_finder", "log_debug_finder"]

    # Dashboard views — each view creates a chart on the dashboard
    aggregation_views:
      sum_file_count_by_tech_stack_pie:
        title: "Total File Count"
        func:
          type: "sum"
          analyzer_id: "file_count"
          tag_filters:
            category: "size"
        group_by: ["tech_stack"]
        include_children: false
        chart_type: "pie"

      top_file_size:
        title: "Top 10 File Size"
        tech_stacks: ["All", "Rust", "Python", "Web"]
        change_type_mode: "switchable"
        func:
          type: "top_n"
          analyzer_id: "char_count"
          order: "desc"

      sum_file_count_by_tech_stack_table:
        title: "Total File Count by Tech Stack"
        change_type_mode: "switchable"
        func:
          type: "sum"
          analyzer_id: "file_count"
          tag_filters:
            category: "size"
        group_by: ["tech_stack"]
        include_children: false
        chart_type: "table"

      sum_char_count:
        title: "Total Char Count"
        tech_stacks: ["Rust"]
        func:
          type: "sum"
          analyzer_id: "char_count"
        chart_type: "gauge"

      avg_file_size:
        title: "Average File Size"
        func:
          type: "avg"
          analyzer_id: "char_count"
        group_by: ["tech_stack"]
        chart_type: "bar_col"

      file_size_distribution:
        title: "File Size Distribution"
        func:
          type: "distribution"
          analyzer_id: "char_count"
          buckets: [1000, 3000, 5000, 10000, 50000]
        chart_type: "bar_col"

      log_stat_count:
        title: "Log Stat Count"
        tech_stacks: ["Rust", "Python", "Web"]
        func:
          type: "sum"
          tag_filters:
            category: "logging"
        group_by: ["metric_key"]
        chart_type: "table"

      top_complexity:
        title: "Top 10 Complexity"
        tech_stacks: ["Rust", "Python", "Web"]
        func:
          type: "top_n"
          tag_filters:
            metric: "complexity"
            category: "maintainability"
          order: "desc"

      complexity_radar:
        title: "Complexity Overview (Radar)"
        tech_stacks: ["Rust", "Python", "Web"]
        func:
          type: "top_n"
          tag_filters:
            metric: "complexity"
          limit: 6
        chart_type: "radar"

      # ── Duplication detection views ─────────────────────
      # Cross-file analyzers emit generic metrics: finding_count,
      # occurrence_count, affected_file_count, and affected_line_count.
      top_duplicate_funcs:
        title: "Top 10 Duplicate Functions"
        tech_stacks: ["All"]
        detail_view: true
        func:
          type: "top_n"
          order: "desc"
          tag_filters:
            category: "duplication"
            metric: "occurrence_count"
        group_by: ["analyzer_id", "file_path"]
        chart_type: "bar_horizontal"

      dup_block_size_distribution:
        title: "Duplication Block Size Distribution"
        tech_stacks: ["All"]
        func:
          type: "distribution"
          tag_filters:
            category: "duplication"
            metric: "affected_line_count"
          buckets: [50, 200, 500, 1000, 5000]
        chart_type: "bar_col"

      dup_by_analyzer:
        title: "Duplicates by Analyzer"
        tech_stacks: ["All"]
        func:
          type: "top_n"
          order: "desc"
          tag_filters:
            category: "duplication"
            metric: "finding_count"
        group_by: ["analyzer_id"]
        chart_type: "pie"
"#
        .to_string();

        // Keep the built-in Camel template sourced from the standalone,
        // validated Camel configuration so release users receive one canonical
        // analyzer/report definition in both forms.
        let camel_config: CodePrismConfig =
            serde_yaml::from_str(include_str!("../../../codeprism.camel-java-dsl.yaml"))
                .expect("embedded Camel Java DSL configuration must be valid");
        let camel_project = camel_config
            .projects
            .first()
            .expect("embedded Camel Java DSL configuration must contain a project");
        let camel_yaml =
            serde_yaml::to_string(camel_project).expect("Camel project template must serialize");
        let indented_camel = camel_yaml
            .lines()
            .map(|line| format!("    {}", line))
            .collect::<Vec<_>>()
            .join("\n");
        let project_templates = format!(
            "project_templates:\n  camel-java-dsl:\n{}\n",
            indented_camel
        );

        template.replacen("project_templates:\n", &project_templates, 1)
    }

    pub fn validate(&self) -> Result<(), AppError> {
        // Collect per-project validation errors
        let mut errors: Vec<String> = Vec::new();

        let projects = self.get_projects();
        for project in &projects {
            // Build set of valid analyzer IDs for this project
            let mut valid_ids: Vec<&str> = vec!["file_count", "char_count"];
            valid_ids.extend(project.custom_regex_analyzers.keys().map(|s| s.as_str()));
            valid_ids.extend(project.custom_impl_analyzers.keys().map(|s| s.as_str()));
            valid_ids.extend(project.external_analyzers.keys().map(|s| s.as_str()));
            valid_ids.extend(
                project
                    .custom_cross_file_analyzers
                    .keys()
                    .map(|s| s.as_str()),
            );
            let valid_set: std::collections::HashSet<&str> = valid_ids.iter().copied().collect();

            for stack in &project.tech_stacks {
                if stack.name.is_empty() {
                    errors.push("Tech stack name cannot be empty".to_string());
                    continue;
                }
                if stack.extensions.is_empty() {
                    errors.push(format!("Tech stack '{}' has no extensions", stack.name));
                }

                // Check that referenced analyzer IDs exist
                for analyzer_id in &stack.analyzers {
                    if !valid_set.contains(analyzer_id.as_str()) {
                        errors.push(format!(
                            "Tech stack '{}' in project '{}' references unknown \
                             analyzer '{}'. Available analyzers: {}",
                            stack.name,
                            project.name,
                            analyzer_id,
                            valid_ids.join(", ")
                        ));
                    }
                }
            }

            // Validate aggregation views
            for (view_id, view) in &project.aggregation_views {
                if view.title.is_empty() {
                    errors.push(format!(
                        "Aggregation view '{}' in project '{}' has an empty title",
                        view_id, project.name
                    ));
                }

                // Validate func-specific fields
                match &view.func {
                    AggregationFunc::TopN { .. } => {}
                    AggregationFunc::Distribution { buckets, .. } if buckets.is_empty() => {
                        errors.push(format!(
                            "Distribution view '{}' in project '{}' has no buckets defined",
                            view_id, project.name
                        ));
                    }
                    _ => {}
                }

                // Validate chart_type if set
                if let Some(chart_type) = &view.chart_type {
                    const VALID_CHART_TYPES: &[&str] = &[
                        "card",
                        "table",
                        "bar_row",
                        "bar_horizontal",
                        "bar_col",
                        "bar_vertical",
                        "pie",
                        "line",
                        "stacked_bar",
                        "heatmap",
                        "radar",
                        "gauge",
                    ];
                    if !VALID_CHART_TYPES.contains(&chart_type.as_str()) {
                        errors.push(format!(
                            "Aggregation view '{}' in project '{}' has unknown chart_type '{}'",
                            view_id, project.name, chart_type
                        ));
                    }
                }

                // Validate change_type_mode if set
                if let Some(ctm) = &view.change_type_mode
                    && ctm != "all"
                    && ctm != "switchable"
                {
                    errors.push(format!(
                            "Aggregation view '{}' in project '{}' has invalid change_type_mode '{}' (expected 'all' or 'switchable')",
                            view_id, project.name, ctm
                        ));
                }
            }

            // Validate cross-file analyzer names
            for analyzer_id in project.custom_cross_file_analyzers.keys() {
                if analyzer_id.is_empty() {
                    errors.push(format!(
                        "Cross-file analyzer in project '{}' has an empty name",
                        project.name
                    ));
                }
            }
        }

        if errors.is_empty() {
            Ok(())
        } else {
            Err(AppError::Config(errors.join("\n")))
        }
    }
}

/// Configuration for a cross-file analysis analyzer.
///
/// Cross-file analyzers use a two-phase protocol:
///   1. **extract** — per-file: the script receives file content and returns blocks.
///   2. **finalize** — global: the script receives ALL blocks and returns generic
///      findings, occurrences, tags, and metrics. All domain logic is script-owned.
///
/// This struct holds only framework-level settings: tags, scan_mode, change_type.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CrossFileAnalyzerConfig {
    /// Tags for the analyzer results (merged on top of script output).
    /// When set, these override whatever the Python script defines as defaults.
    #[serde(default)]
    pub tags: HashMap<String, String>,
    /// Old-style metric_key field (merged into tags as `metric` key).
    /// Takes precedence over `tags.metric`.
    #[serde(default)]
    pub metric_key: Option<String>,
    /// Old-style category field (merged into tags as `category` key).
    /// Takes precedence over `tags.category`.
    #[serde(default)]
    pub category: Option<String>,
    /// Human-readable description
    #[serde(default)]
    pub description: Option<String>,
    /// Scan modes this analyzer applies to: "all" (default), "snapshot", "diff"
    #[serde(default)]
    pub scan_mode: Option<String>,
    /// Change types this analyzer applies to: "all" (default), "A", "M", "D"
    #[serde(default)]
    pub change_type: Option<String>,
}

impl CrossFileAnalyzerConfig {
    /// Resolve tags by merging old metric_key/category with new tags field.
    /// Old fields take precedence.
    pub fn resolve_tags(&self) -> HashMap<String, String> {
        let mut result = self.tags.clone();
        if let Some(mk) = &self.metric_key {
            result.insert(TAG_METRIC.to_string(), mk.clone());
        }
        if let Some(cat) = &self.category {
            result.insert(TAG_CATEGORY.to_string(), cat.clone());
        }
        result
    }
}

/// Legacy duplication-specific finalize output. Kept only so existing custom
/// analyzers continue to work; new analyzers should return `FinalizeOutput`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalizeMatchResult {
    /// Hash identifying this block group (matches IntermediateBlock::group_key).
    pub block_hash: String,
    /// Display content of the duplicated block (e.g. the first occurrence's body).
    pub block_content: String,
    /// Size metric for display (e.g. lines of code, or a sentinel like -1 for XML).
    pub block_size: i32,
    /// Per-file occurrences that comprise this match group.
    pub occurrences: Vec<FinalizeOccurrence>,
}

/// A single file-level occurrence within a `FinalizeMatchResult` match group.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalizeOccurrence {
    pub file_path: String,
    #[serde(default)]
    pub line_start: i32,
    #[serde(default)]
    pub line_end: i32,
    /// Change type: "A" (Add), "M" (Modify), "D" (Delete), or None.
    pub change_type: Option<String>,
    /// Diff side: "0" (old) or "1" (new), or None for snapshot mode.
    pub side: Option<String>,
}

/// Generic output of a cross-file analyzer's finalize phase.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct FinalizeOutput {
    #[serde(default)]
    pub findings: Vec<FinalizeFinding>,
}

/// One logical cross-file finding. The analyzer owns its identity, display
/// content, occurrences, tags, and metrics; the framework only validates and
/// persists this data.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalizeFinding {
    pub finding_key: String,
    #[serde(default)]
    pub content: Option<String>,
    #[serde(default)]
    pub occurrences: Vec<FinalizeOccurrence>,
    #[serde(default)]
    pub tags: HashMap<String, String>,
    #[serde(default)]
    pub metrics: Vec<FinalizeMetric>,
}

/// A metric explicitly emitted by a cross-file analyzer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FinalizeMetric {
    pub metric_key: String,
    pub file_path: String,
    #[serde(default)]
    pub value_before: f64,
    #[serde(default)]
    pub value_after: f64,
    #[serde(default)]
    pub change_type: Option<String>,
    #[serde(default)]
    pub scope: Option<String>,
    #[serde(default)]
    pub tags: HashMap<String, String>,
}

/// A single content block extracted from a file for duplicate detection
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContentBlock {
    pub block_hash: String,
    pub block_size: i32,
    pub line_start: i32,
    pub line_end: i32,
    /// The actual content text of this block (stored temporarily for match creation)
    pub block_content: String,
}

/// A generic intermediate block for cross-file analysis pipelines.
///
/// Each analyzer type maps its own semantics onto the generic fields:
///
/// | 分析类型      | group_key    | blob_data      | int_data1    | int_data2   | int_data3  | str_data1     | str_data2 |
/// |--------------|-------------|----------------|-------------|------------|-----------|--------------|----------|
/// | 重复代码检测   | block_hash  | block_content  | block_size  | line_start | line_end   | change_type  | side     |
/// | 跨文件引用统计 | import目标   | 导入语句       | 行号         | —          | —          | change_type  | —        |
/// | TODO/FIXME   | 标签名       | 注释内容       | 行号         | —          | —          | change_type  | —        |
///
/// The `analyzer_id` and `file_path` fields are always populated by the pipeline,
/// not by the analyzer's `extract_blocks()` implementation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IntermediateBlock {
    /// Set by the pipeline coordinator before saving — not by `extract_blocks()`.
    #[serde(default)]
    pub analyzer_id: String,
    /// Set by the pipeline coordinator before saving — not by `extract_blocks()`.
    #[serde(default)]
    pub file_path: String,
    /// Primary aggregation key (e.g. block_hash for duplication, import target for cross-ref)
    pub group_key: String,
    /// Text payload (e.g. block_content for duplication, import statement for cross-ref)
    pub blob_data: Option<String>,
    /// Generic integer field 1 (e.g. block_size, line_number)
    pub int_data1: Option<i64>,
    /// Generic integer field 2 (e.g. line_start, column)
    pub int_data2: Option<i64>,
    /// Generic integer field 3 (e.g. line_end)
    pub int_data3: Option<i64>,
    /// Generic string field 1 (e.g. change_type)
    pub str_data1: Option<String>,
    /// Generic string field 2 (e.g. side for diff mode: "0"=old, "1"=new)
    pub str_data2: Option<String>,
}

impl IntermediateBlock {
    /// Create an IntermediateBlock from duplication-style analysis data.
    pub fn for_duplication(
        group_key: String,
        blob_data: Option<String>,
        block_size: Option<i64>,
        line_start: Option<i64>,
        line_end: Option<i64>,
    ) -> Self {
        Self {
            analyzer_id: String::new(),
            file_path: String::new(),
            group_key,
            blob_data,
            int_data1: block_size,
            int_data2: line_start,
            int_data3: line_end,
            str_data1: None,
            str_data2: None,
        }
    }
}

/// Intermediate block format returned by Python scripts. New scripts should
/// provide `group_key`; normalized/content hashing is a legacy fallback.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptContentBlock {
    /// Analyzer-defined aggregation identity. When omitted, the legacy
    /// normalized-content/content hash fallback is used.
    #[serde(default)]
    pub group_key: Option<String>,
    pub block_size: i32,
    pub line_start: i32,
    pub line_end: i32,
    pub block_content: String,
    /// Optional normalized version (e.g. whitespace+comments stripped) used for
    /// hash computation so that the same effective code with different formatting
    /// is detected as a duplicate. If absent, `block_content` is hashed instead.
    #[serde(default)]
    pub normalized_content: Option<String>,
}

impl From<ScriptContentBlock> for ContentBlock {
    fn from(s: ScriptContentBlock) -> Self {
        use sha2::Digest;
        let hash = s.group_key.unwrap_or_else(|| {
            let hash_source = s.normalized_content.as_ref().unwrap_or(&s.block_content);
            hex::encode(sha2::Sha256::digest(hash_source.as_bytes()))
        });
        ContentBlock {
            block_hash: hash,
            block_size: s.block_size,
            line_start: s.line_start,
            line_end: s.line_end,
            block_content: s.block_content,
        }
    }
}

pub enum ChangeType {
    Add,
    Modify,
    Delete,
}

#[derive(thiserror::Error, Debug)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(String),
    #[error("IO error: {0}")]
    Io(String),
    #[error("Git error: {0}")]
    Git(String),
    #[error("Configuration error: {0}")]
    Config(String),
    #[error("Unknown error: {0}")]
    Unknown(String),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn script_content_block_prefers_analyzer_group_key() {
        let block = ScriptContentBlock {
            group_key: Some("domain:key".into()),
            block_size: 1,
            line_start: 1,
            line_end: 1,
            block_content: "display".into(),
            normalized_content: Some("legacy-hash-source".into()),
        };
        assert_eq!(ContentBlock::from(block).block_hash, "domain:key");
    }

    fn analyzer_ids(func: AggregationFunc) -> Vec<String> {
        match func {
            AggregationFunc::TopN { analyzer_id, .. }
            | AggregationFunc::Sum { analyzer_id, .. }
            | AggregationFunc::Avg { analyzer_id, .. }
            | AggregationFunc::Min { analyzer_id, .. }
            | AggregationFunc::Max { analyzer_id, .. }
            | AggregationFunc::Distribution { analyzer_id, .. } => analyzer_id,
        }
    }

    #[test]
    fn aggregation_analyzer_id_accepts_legacy_string() {
        let func: AggregationFunc =
            serde_yaml::from_str("type: sum\nanalyzer_id: duplicate_rust_fns_aggregated\n")
                .unwrap();

        assert_eq!(
            analyzer_ids(func),
            vec!["duplicate_rust_fns_aggregated".to_string()]
        );
    }

    #[test]
    fn aggregation_analyzer_id_accepts_empty_and_multiple_values() {
        let empty: AggregationFunc = serde_yaml::from_str("type: sum\nanalyzer_id: []\n").unwrap();
        assert!(analyzer_ids(empty).is_empty());

        let multiple: AggregationFunc = serde_yaml::from_str(
            "type: top_n\n\
             analyzer_id:\n\
               - duplicate_rust_fns_aggregated\n\
               - duplicate_python_defs_aggregated\n\
             order: desc\n",
        )
        .unwrap();
        assert_eq!(
            analyzer_ids(multiple),
            vec![
                "duplicate_rust_fns_aggregated".to_string(),
                "duplicate_python_defs_aggregated".to_string(),
            ]
        );
    }

    #[test]
    fn multi_analyzer_grouped_view_parses_and_round_trips() {
        let yaml = r#"
projects:
  - name: grouped-analysis
    aggregation_views:
      duplicates_by_analyzer:
        title: Duplicates by Analyzer
        group_by: [analyzer_id]
        func:
          type: sum
          analyzer_id:
            - duplicate_rust_fns_aggregated
            - duplicate_python_defs_aggregated
          tag_filters:
            category: duplication
"#;

        let config: CodePrismConfig = serde_yaml::from_str(yaml).unwrap();
        let project = &config.projects[0];
        let view = &project.aggregation_views["duplicates_by_analyzer"];
        assert_eq!(view.group_by, vec!["analyzer_id"]);
        assert_eq!(
            analyzer_ids(view.func.clone()),
            vec![
                "duplicate_rust_fns_aggregated".to_string(),
                "duplicate_python_defs_aggregated".to_string(),
            ]
        );

        let serialized = serde_yaml::to_string(&config).unwrap();
        let reparsed: CodePrismConfig = serde_yaml::from_str(&serialized).unwrap();
        assert_eq!(
            analyzer_ids(
                reparsed.projects[0].aggregation_views["duplicates_by_analyzer"]
                    .func
                    .clone()
            ),
            vec![
                "duplicate_rust_fns_aggregated".to_string(),
                "duplicate_python_defs_aggregated".to_string(),
            ]
        );
    }

    #[test]
    fn generated_template_is_parseable() {
        let template = CodePrismConfig::generate_template();
        let config = serde_yaml::from_str::<CodePrismConfig>(&template).unwrap();
        let camel = config
            .get_template("camel-java-dsl")
            .expect("Camel Java DSL must be available in a fresh configuration");
        assert_eq!(camel.tech_stacks[0].name, "Camel Java DSL");
        assert!(
            camel
                .custom_impl_analyzers
                .contains_key("camel_java_production_metrics")
        );
        assert!(
            camel
                .custom_cross_file_analyzers
                .contains_key("camel_java_test_project_metrics")
        );
        assert!(camel.aggregation_views.contains_key("route_count"));

        let camel_config = CodePrismConfig {
            projects: vec![camel],
            ..Default::default()
        };
        camel_config.validate().unwrap();
    }

    #[test]
    fn sample_config_is_parseable() {
        let sample = include_str!("../../../codeprism.sample.yaml");
        serde_yaml::from_str::<CodePrismConfig>(sample).unwrap();
    }
}
