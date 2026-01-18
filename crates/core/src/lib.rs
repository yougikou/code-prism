use serde::{Deserialize, Serialize};
use std::collections::HashMap;
// use indexmap::IndexMap; // Not needed if using fully qualified path

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MetricEntry {
    pub analyzer_id: String,
    pub metric_key: String,
    pub category: Option<String>,
    pub value: f64,
    pub scope: Option<String>,
    pub tech_stack: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct CodePrismConfig {
    pub tech_stacks: Vec<TechStack>,
    #[serde(default)]
    pub global_excludes: Vec<String>,
    pub database_url: Option<String>,
    #[serde(default)]
    pub custom_regex_analyzers: HashMap<String, CustomAnalyzerDef>,
    #[serde(default)]
    pub custom_impl_analyzers: HashMap<String, ImplAnalyzerConfig>,
    #[serde(default)]
    pub external_analyzers: HashMap<String, String>,
    #[serde(default)]
    pub aggregation_views: indexmap::IndexMap<String, AggregationView>,
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
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ImplAnalyzerConfig {
    pub metric_key: Option<String>,
    pub category: Option<String>,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum AggregationFunc {
    #[serde(rename = "top_n")]
    TopN {
        analyzer_id: Option<String>,
        metric_key: String,
        category: Option<String>,
        limit: usize,
        #[serde(default)]
        order: SortOrder,
    },
    #[serde(rename = "sum")]
    Sum {
        analyzer_id: Option<String>,
        metric_key: String,
        category: Option<String>,
    },
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
    pub func: AggregationFunc,
}

fn default_true() -> bool {
    true
}

fn default_metric_key() -> String {
    "matches".to_string()
}

impl CodePrismConfig {
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
                                case_sensitive: false, // Or true? Git is usually case sensitive but on windows... let's stick to default/permissive for now or use strict?
                                // Actually default is permissive for case on Windows.
                                // CRITICAL: require_literal_separator = true to prevent * from matching /
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

            // Check Local Excludes (Only if paths were empty, meaning generic extension match)
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
        // regardless of global settings.
        let path_obj = std::path::Path::new(path);
        let ext = path_obj
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_string();

        for stack in &self.tech_stacks {
            // Check extension match for the stack to be relevant?
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

        // 2. Check Global Excludes
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

    pub fn load_from_file<P: AsRef<std::path::Path>>(path: P) -> Result<Self, AppError> {
        let content = std::fs::read_to_string(path)
            .map_err(|e| AppError::Config(format!("Failed to read config file: {}", e)))?;
        serde_yaml::from_str(&content)
            .map_err(|e| AppError::Config(format!("Failed to parse config file: {}", e)))
    }

    pub fn generate_template() -> String {
        r#"# CodePrism Configuration
tech_stacks:
  - name: "Rust"
    extensions: ["rs", "toml"]
    analyzers: ["file_count", "char_count"]
    paths: ["crates/**"]

  - name: "Web"
    extensions: ["js", "ts", "jsx", "tsx", "html", "css"]
    analyzers: ["file_count"]
    # paths: [] # Optional: restrict to specific folders

  - name: "Java"
    extensions: ["java", "jsp", "xml"]
    analyzers: ["file_count", "char_count"]



global_excludes: ["**/.git/**", "**/node_modules/**", "**/target/**", "**/dist/**"]
# database_url: "sqlite:.codeprism.db" # Optional: Override DB URL
# custom_analyzers:
#   todo_finder: "(TODO|FIXME):.*"
# external_analyzers:
#   java_complexity: "analyzers/java_complexity.wasm"
"#
        .to_string()
    }

    pub fn validate(&self) -> Result<(), AppError> {
        for stack in &self.tech_stacks {
            if stack.name.is_empty() {
                return Err(AppError::Config(
                    "Tech stack name cannot be empty".to_string(),
                ));
            }
            if stack.extensions.is_empty() {
                return Err(AppError::Config(format!(
                    "Tech stack '{}' has no extensions",
                    stack.name
                )));
            }
        }
        Ok(())
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
