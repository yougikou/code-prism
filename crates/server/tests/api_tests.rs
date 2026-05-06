//! API Integration Tests for CodePrism Server
//!
//! These tests verify the server endpoints work correctly.

use codeprism_server::config::{
    AppConfig, ProjectAppConfig, SourceConfig, TopNParams, ViewConfig, ViewKind,
};

/// Test that AppConfig can be serialized to JSON correctly
#[test]
fn test_app_config_serialization() {
    let config = AppConfig {
        projects: vec![ProjectAppConfig {
            name: "test_project".to_string(),
            views: vec![ViewConfig {
                id: "test_view".to_string(),
                title: "Test View".to_string(),
                tech_stacks: vec!["Rust".to_string()],
                category: None,
                include_children: true,
                group_by: vec![],
                chart_type: Some("bar_row".to_string()),
                change_type_mode: None,
                width: 1,
                kind: ViewKind::TopN {
                    source: SourceConfig {
                        analyzer_id: "char_count".to_string(),
                        metric_key: Some("char_count".to_string()),
                        category: None,
                    },
                    params: TopNParams { limit: 10, order: Default::default() },
                },
            }],
            tech_stacks: vec!["Rust".to_string()],
        }],
    };

    let json = serde_json::to_string(&config).expect("Failed to serialize config");

    // Verify JSON structure
    let parsed: serde_json::Value = serde_json::from_str(&json).expect("Failed to parse JSON");
    assert!(parsed["projects"].is_array());
    assert_eq!(parsed["projects"][0]["name"], "test_project");
    assert_eq!(parsed["projects"][0]["views"][0]["id"], "test_view");
}

/// Test app config helper methods
#[test]
fn test_app_config_helpers() {
    let config = AppConfig {
        projects: vec![
            ProjectAppConfig {
                name: "project_a".to_string(),
                views: vec![],
                tech_stacks: vec!["Rust".to_string()],
            },
            ProjectAppConfig {
                name: "project_b".to_string(),
                views: vec![],
                tech_stacks: vec!["Python".to_string()],
            },
        ],
    };

    // Test get_project_names
    let names = config.get_project_names();
    assert_eq!(names, vec!["project_a", "project_b"]);

    // Test get_project
    let project = config
        .get_project("project_a")
        .expect("Project should exist");
    assert_eq!(project.name, "project_a");

    // Test get_default_project
    let default = config.get_default_project().expect("Default should exist");
    assert_eq!(default.name, "project_a");

    // Test is_multi_project
    assert!(config.is_multi_project());
}

/// Test single project mode
#[test]
fn test_single_project_mode() {
    let config = AppConfig {
        projects: vec![ProjectAppConfig {
            name: "only_project".to_string(),
            views: vec![],
            tech_stacks: vec!["Rust".to_string()],
        }],
    };

    assert!(!config.is_multi_project());
    assert_eq!(config.get_project_names().len(), 1);
}

/// Test ViewKind serialization for different aggregation types
#[test]
fn test_view_kind_serialization() {
    let sum_view = ViewConfig {
        id: "sum_view".to_string(),
        title: "Sum View".to_string(),
        tech_stacks: vec![],
        category: None,
        include_children: true,
        group_by: vec!["tech_stack".to_string()],
        chart_type: Some("pie".to_string()),
        change_type_mode: None,
        width: 1,
        kind: ViewKind::Sum {
            source: SourceConfig {
                analyzer_id: "".to_string(),
                metric_key: Some("file_count".to_string()),
                category: None,
            },
        },
    };

    let json = serde_json::to_string(&sum_view).expect("Failed to serialize");
    assert!(json.contains("\"type\":\"sum\""));
    assert!(json.contains("\"metric_key\":\"file_count\""));
}
