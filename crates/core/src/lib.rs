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

/// Project-specific configuration (all settings except database_url)
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProjectConfig {
    pub name: String,
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
    pub database_url: Option<String>,

    // Multi-project format: list of project configs
    #[serde(default)]
    pub projects: Vec<ProjectConfig>,

    // Project templates keyed by template name (stored separately from projects)
    #[serde(default)]
    pub project_templates: HashMap<String, ProjectConfig>,

    // Legacy single-project format (for backward compatibility)
    // These fields are merged into a default project if 'projects' is empty
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
        #[serde(default)]
        analyzer_id: Option<String>,
        #[serde(default)]
        metric_key: Option<String>,
        #[serde(default)]
        category: Option<String>,
        limit: usize,
        #[serde(default)]
        order: SortOrder,
    },
    #[serde(rename = "sum")]
    Sum {
        #[serde(default)]
        analyzer_id: Option<String>,
        #[serde(default)]
        metric_key: Option<String>,
        #[serde(default)]
        category: Option<String>,
    },
    #[serde(rename = "avg")]
    Avg {
        #[serde(default)]
        analyzer_id: Option<String>,
        #[serde(default)]
        metric_key: Option<String>,
        #[serde(default)]
        category: Option<String>,
    },
    #[serde(rename = "min")]
    Min {
        #[serde(default)]
        analyzer_id: Option<String>,
        #[serde(default)]
        metric_key: Option<String>,
        #[serde(default)]
        category: Option<String>,
    },
    #[serde(rename = "max")]
    Max {
        #[serde(default)]
        analyzer_id: Option<String>,
        #[serde(default)]
        metric_key: Option<String>,
        #[serde(default)]
        category: Option<String>,
    },
    #[serde(rename = "distribution")]
    Distribution {
        #[serde(default)]
        analyzer_id: Option<String>,
        #[serde(default)]
        metric_key: Option<String>,
        #[serde(default)]
        category: Option<String>,
        buckets: Vec<f64>,
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
    /// Display mode for change types: "all" (stacked) or "switchable" (A/M/D toggle)
    #[serde(default)]
    pub change_type_mode: Option<String>,
    pub func: AggregationFunc,
}

fn default_true() -> bool {
    true
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
# For VS Code autocomplete, copy schemas/codeprism-config.schema.json to
# your project's .vscode/ or add this line (adjust the path):
# yaml-language-server: $schema=schemas/codeprism-config.schema.json

tech_stacks:
  - name: "Rust"
    extensions: ["rs", "toml"]
    # Built-in analyzers: file_count (always active), char_count
    # Custom analyzers: add IDs from custom_regex_analyzers below
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
# database_url: "sqlite:codeprism.db" # Optional: Override DB URL

# Custom regex analyzers — each key becomes an analyzer ID usable in tech_stacks
# custom_regex_analyzers:
#   todo_finder: "(TODO|FIXME):.*"
#   my_pattern:
#     pattern: "\\b(regex)\\b"
#     metric_key: "matches"   # default: "matches"
#     category: "pattern"     # optional

# External WASM analyzers — key is the analyzer ID, value is the .wasm path
# external_analyzers:
#   java_complexity: "analyzers/java_complexity.wasm"

# Python script analyzers — place .py files in custom_analyzers/ dir, filename stem = analyzer ID
# Optionally register overrides here:
# custom_impl_analyzers:
#   my_script:
#     metric_key: "result"
#     category: "custom"
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
