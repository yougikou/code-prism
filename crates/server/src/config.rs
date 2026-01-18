use anyhow::Result;
use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Deserialize)]
pub struct AppConfig {
    pub views: Vec<ViewConfig>,
}

#[derive(Debug, Deserialize)]
pub struct ViewConfig {
    pub id: String,
    pub tech_stack: String,
    #[serde(flatten)]
    pub kind: ViewKind,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type")]
pub enum ViewKind {
    #[serde(rename = "top_n")]
    TopN {
        source: SourceConfig,
        params: TopNParams,
    },
    #[serde(rename = "sum")]
    Sum { source: SourceConfig },
}

#[derive(Debug, Deserialize)]
pub struct SourceConfig {
    pub analyzer_id: String,
    pub metric_key: String,
}

#[derive(Debug, Deserialize)]
pub struct TopNParams {
    pub limit: u32,
}

impl AppConfig {
    pub fn load_from_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config = serde_yaml::from_str(&content)?;
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_load_config() {
        let yaml = r#"
views:
  - id: "top_file_size"
    tech_stack: "Gosu"
    type: "top_n"
    source: { analyzer_id: "char_count", metric_key: "length" }
    params: { limit: 10 }
"#;
        let config: AppConfig = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(config.views.len(), 1);
        let view = &config.views[0];
        assert_eq!(view.id, "top_file_size");
        assert_eq!(view.tech_stack, "Gosu");

        match &view.kind {
            ViewKind::TopN { source, params } => {
                assert_eq!(source.analyzer_id, "char_count");
                assert_eq!(source.metric_key, "length");
                assert_eq!(params.limit, 10);
            }
            ViewKind::Sum { .. } => panic!("Unexpected Sum view"),
        }
    }
}
