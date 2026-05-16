use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// Well-known tag keys
pub const TAG_METRIC: &str = "metric";
pub const TAG_CATEGORY: &str = "category";

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
}

/// Project-specific configuration (all settings except database_url)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
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
    #[serde(default)]
    pub aggregation_views: indexmap::IndexMap<String, AggregationView>,
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
                aggregation_views: self.aggregation_views.clone(),
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
pub enum SortOrder {
    Asc,
    Desc,
}

impl Default for SortOrder {
    fn default() -> Self {
        SortOrder::Desc
    }
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
        fn visit_seq<A: de::SeqAccess<'de>>(self, seq: A) -> Result<Vec<String>, A::Error> {
            de::Deserialize::deserialize(de::value::SeqAccessDeserializer::new(seq))
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
        limit: usize,
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
            CustomAnalyzerDef::Config { metric_key, category, tags, .. } => {
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
}

fn default_true() -> bool {
    true
}

fn default_width() -> u32 {
    1
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
                    if let Ok(glob) = glob::Pattern::new(pattern) {
                        if glob.matches_with(
                            path,
                            glob::MatchOptions {
                                case_sensitive: false,
                                require_literal_separator: true,
                                require_literal_leading_dot: false,
                            },
                        ) {
                            matched = true;
                            break;
                        }
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
                    if let Ok(glob) = glob::Pattern::new(pattern) {
                        if glob.matches_with(
                            path,
                            glob::MatchOptions {
                                require_literal_separator: true,
                                case_sensitive: false,
                                require_literal_leading_dot: false,
                            },
                        ) {
                            excluded = true;
                            break;
                        }
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
                    if let Ok(glob) = glob::Pattern::new(pattern) {
                        if glob.matches_with(
                            path,
                            glob::MatchOptions {
                                require_literal_separator: true,
                                case_sensitive: false,
                                require_literal_leading_dot: false,
                            },
                        ) {
                            return false; // Explicitly included -> Not excluded
                        }
                    }
                }
            }
        }

        // 2. Check Global Excludes (Project-specific in this case)
        for pattern in &self.global_excludes {
            if let Ok(glob) = glob::Pattern::new(pattern) {
                if glob.matches_with(
                    path,
                    glob::MatchOptions {
                        require_literal_separator: true,
                        case_sensitive: false,
                        require_literal_leading_dot: false,
                    },
                ) {
                    return true;
                }
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
        r#"# CodePrism Configuration
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
        tags:
          metric: "complexity"
          category: "maintainability"

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
          limit: 10
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
          limit: 10
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
"#
        .to_string()
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
            let valid_set: std::collections::HashSet<&str> =
                valid_ids.iter().copied().collect();

            for stack in &project.tech_stacks {
                if stack.name.is_empty() {
                    errors.push("Tech stack name cannot be empty".to_string());
                    continue;
                }
                if stack.extensions.is_empty() {
                    errors.push(format!(
                        "Tech stack '{}' has no extensions",
                        stack.name
                    ));
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
                    AggregationFunc::TopN { limit, .. } => {
                        if *limit == 0 {
                            errors.push(format!(
                                "TopN view '{}' in project '{}' has limit=0",
                                view_id, project.name
                            ));
                        }
                    }
                    AggregationFunc::Distribution { buckets, .. } => {
                        if buckets.is_empty() {
                            errors.push(format!(
                                "Distribution view '{}' in project '{}' has no buckets defined",
                                view_id, project.name
                            ));
                        }
                    }
                    _ => {}
                }

                // Validate chart_type if set
                if let Some(chart_type) = &view.chart_type {
                    const VALID_CHART_TYPES: &[&str] = &[
                        "card", "table",
                        "bar_row", "bar_horizontal",
                        "bar_col", "bar_vertical",
                        "pie", "line", "stacked_bar",
                        "heatmap", "radar", "gauge",
                    ];
                    if !VALID_CHART_TYPES.contains(&chart_type.as_str()) {
                        errors.push(format!(
                            "Aggregation view '{}' in project '{}' has unknown chart_type '{}'",
                            view_id, project.name, chart_type
                        ));
                    }
                }

                // Validate change_type_mode if set
                if let Some(ctm) = &view.change_type_mode {
                    if ctm != "all" && ctm != "switchable" {
                        errors.push(format!(
                            "Aggregation view '{}' in project '{}' has invalid change_type_mode '{}' (expected 'all' or 'switchable')",
                            view_id, project.name, ctm
                        ));
                    }
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
