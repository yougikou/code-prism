use serde::Deserialize;
use codeprism_core::SortOrder;
use std::collections::HashMap;

/// Project-specific configuration for the UI
#[derive(Debug, Deserialize, serde::Serialize, Clone, utoipa::ToSchema)]
pub struct ProjectAppConfig {
    pub name: String,
    pub views: Vec<ViewConfig>,
    pub tech_stacks: Vec<String>,
}

/// Root application config
#[derive(Debug, Deserialize, serde::Serialize, Clone, utoipa::ToSchema)]
pub struct AppConfig {
    /// List of project configurations
    pub projects: Vec<ProjectAppConfig>,
}

impl AppConfig {
    /// Get all project names
    pub fn get_project_names(&self) -> Vec<String> {
        self.projects.iter().map(|p| p.name.clone()).collect()
    }

    /// Get project config by name
    pub fn get_project(&self, name: &str) -> Option<&ProjectAppConfig> {
        self.projects.iter().find(|p| p.name == name)
    }

    /// Get the first/default project (for single-project mode)
    pub fn get_default_project(&self) -> Option<&ProjectAppConfig> {
        self.projects.first()
    }

    /// Check if there are multiple projects
    pub fn is_multi_project(&self) -> bool {
        self.projects.len() > 1
    }
}

#[derive(Debug, Deserialize, serde::Serialize, Clone, utoipa::ToSchema)]
pub struct ViewConfig {
    pub id: String,
    pub title: String,
    pub tech_stacks: Vec<String>,
    #[serde(default = "default_include_children")]
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
    #[serde(flatten)]
    pub kind: ViewKind,
}

fn default_width() -> u32 {
    1
}

fn default_include_children() -> bool {
    true
}

#[derive(Debug, Deserialize, serde::Serialize, Clone, utoipa::ToSchema)]
#[serde(tag = "type")]
pub enum ViewKind {
    #[serde(rename = "top_n")]
    TopN {
        source: SourceConfig,
        params: TopNParams,
    },
    #[serde(rename = "sum")]
    Sum { source: SourceConfig },
    #[serde(rename = "avg")]
    Avg { source: SourceConfig },
    #[serde(rename = "min")]
    Min { source: SourceConfig },
    #[serde(rename = "max")]
    Max { source: SourceConfig },
    #[serde(rename = "distribution")]
    Distribution {
        source: SourceConfig,
        params: DistributionParams,
    },
}

#[derive(Debug, Deserialize, serde::Serialize, Clone, utoipa::ToSchema)]
pub struct SourceConfig {
    #[serde(default, deserialize_with = "codeprism_core::deserialize_string_or_vec")]
    pub analyzer_id: Vec<String>,
    /// Tag key-value filters
    #[serde(default)]
    pub tag_filters: HashMap<String, String>,
}

impl SourceConfig {
    /// Return the tag_filters directly.
    pub fn effective_tag_filters(&self) -> &HashMap<String, String> {
        &self.tag_filters
    }
}

#[derive(Debug, Deserialize, serde::Serialize, Clone, utoipa::ToSchema)]
pub struct TopNParams {
    pub limit: u32,
    #[serde(default)]
    pub order: SortOrder,
}

#[derive(Debug, Deserialize, serde::Serialize, Clone, utoipa::ToSchema)]
pub struct DistributionParams {
    /// Bucket boundaries, e.g., [0, 10, 50, 100, 500] creates 5 buckets
    pub buckets: Vec<f64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_config() {
        let yaml = r#"
projects:
  - name: "test_project"
    views:
      - id: "top_file_size"
        title: "Top File Size"
        tech_stacks: ["Gosu"]
        type: "top_n"
        source: { analyzer_id: "char_count" }
        params: { limit: 10 }
    tech_stacks: ["Gosu"]
"#;
        let config: AppConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.projects.len(), 1);
        let project = &config.projects[0];
        assert_eq!(project.name, "test_project");
        assert_eq!(project.views.len(), 1);

        let view = &project.views[0];
        assert_eq!(view.id, "top_file_size");
        assert_eq!(view.tech_stacks, vec!["Gosu"]);

        match &view.kind {
            ViewKind::TopN { source, params } => {
                assert_eq!(source.analyzer_id, vec!["char_count"]);
                assert_eq!(params.limit, 10);
            }
            _ => panic!("Expected TopN view"),
        }
    }
}
