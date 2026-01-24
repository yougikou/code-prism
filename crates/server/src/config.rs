use serde::Deserialize;

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
    pub category: Option<String>,
    #[serde(default = "default_include_children")]
    pub include_children: bool,
    #[serde(default)]
    pub group_by: Vec<String>,
    #[serde(default)]
    pub chart_type: Option<String>,
    /// Display mode for change types: "all" (stacked) or "switchable" (A/M/D toggle)
    #[serde(default)]
    pub change_type_mode: Option<String>,
    #[serde(flatten)]
    pub kind: ViewKind,
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
    #[serde(default)]
    pub analyzer_id: String,
    #[serde(default)]
    pub metric_key: Option<String>,
    #[serde(default)]
    pub category: Option<String>,
}

#[derive(Debug, Deserialize, serde::Serialize, Clone, utoipa::ToSchema)]
pub struct TopNParams {
    pub limit: u32,
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
        category: "maintainability"
        type: "top_n"
        source: { analyzer_id: "char_count", metric_key: "length" }
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
        assert_eq!(view.category, Some("maintainability".to_string()));

        match &view.kind {
            ViewKind::TopN { source, params } => {
                assert_eq!(source.analyzer_id, "char_count");
                assert_eq!(source.metric_key, Some("length".to_string()));
                assert_eq!(params.limit, 10);
            }
            _ => panic!("Expected TopN view"),
        }
    }
}
