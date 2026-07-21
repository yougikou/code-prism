//! API Integration Tests for CodePrism Server
//!
//! These tests verify the server endpoints work correctly.

use codeprism_server::config::{
    AppConfig, ProjectAppConfig, SourceConfig, TechStackInfo, TopNParams, ViewConfig, ViewKind,
};
use codeprism_server::aggregation::{SumAggregator, ViewFilters};

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
                include_children: true,
                group_by: vec![],
                chart_type: Some("bar_row".to_string()),
                change_type_mode: None,
                width: 1,
                kind: ViewKind::TopN {
                    source: SourceConfig {
                        analyzer_id: vec!["char_count".to_string()],
                        tag_filters: std::collections::HashMap::new(),
                    },
                    params: TopNParams { order: Default::default() },
                },
                trend: false,
                detail_view: false,
            }],
            tech_stacks: vec![TechStackInfo { name: "Rust".to_string(), category: None }],
            columns: 4,
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
                tech_stacks: vec![TechStackInfo { name: "Rust".to_string(), category: None }],
                columns: 4,
            },
            ProjectAppConfig {
                name: "project_b".to_string(),
                views: vec![],
                tech_stacks: vec![TechStackInfo { name: "Python".to_string(), category: None }],
                columns: 4,
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
            tech_stacks: vec![TechStackInfo { name: "Rust".to_string(), category: None }],
            columns: 4,
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
        include_children: true,
        group_by: vec!["tech_stack".to_string()],
        chart_type: Some("pie".to_string()),
        change_type_mode: None,
        width: 1,
        kind: ViewKind::Sum {
            source: SourceConfig {
                analyzer_id: vec!["file_count".to_string()],
                tag_filters: std::collections::HashMap::new(),
            },
        },
        trend: false,
        detail_view: false,
    };

    let json = serde_json::to_string(&sum_view).expect("Failed to serialize");
    assert!(json.contains("\"type\":\"sum\""));
    assert!(json.contains("\"analyzer_id\":[\"file_count\"]"));
}

#[tokio::test]
async fn cross_file_metrics_group_by_finding_key() -> anyhow::Result<()> {
    let temp_dir = tempfile::tempdir()?;
    let db_path = temp_dir.path().join("findings.db");
    std::fs::File::create(&db_path)?;
    let db = codeprism_database::Db::new(&format!("sqlite:{}", db_path.display())).await?;
    db.migrate().await?;

    sqlx::query("INSERT INTO projects (name) VALUES ('test')")
        .execute(db.pool())
        .await?;
    sqlx::query(
        "INSERT INTO scans (project_id, commit_hash, scan_mode) VALUES (1, 'abc', 'SNAPSHOT')",
    )
    .execute(db.pool())
    .await?;
    for (path, finding_key) in [("a.rs", "finding-a"), ("b.rs", "finding-b")] {
        sqlx::query(
            "INSERT INTO metrics (scan_id, file_path, analyzer_id, finding_key, tags, value_after) \
             VALUES (1, ?, 'cross_aggregated', ?, '{\"metric\":\"finding_count\"}', 1)",
        )
        .bind(path)
        .bind(finding_key)
        .execute(db.pool())
        .await?;
    }

    let view = ViewConfig {
        id: "findings".to_string(),
        title: "Findings".to_string(),
        tech_stacks: vec![],
        include_children: false,
        group_by: vec!["finding_key".to_string()],
        chart_type: Some("bar_col".to_string()),
        change_type_mode: None,
        width: 1,
        kind: ViewKind::Sum {
            source: SourceConfig {
                analyzer_id: vec!["cross_aggregated".to_string()],
                tag_filters: std::collections::HashMap::from([(
                    "metric".to_string(),
                    "finding_count".to_string(),
                )]),
            },
        },
        trend: false,
        detail_view: true,
    };
    let filters = ViewFilters {
        tech_stack: None,
        category: None,
        metric_key: None,
        change_type: None,
        group_by: None,
    };

    let mut results = SumAggregator::execute(db.pool(), 1, &view, &filters).await?;
    results.sort_by(|left, right| left.label.cmp(&right.label));
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].label, "finding-a");
    assert_eq!(results[0].value, 1.0);
    assert_eq!(results[1].label, "finding-b");
    assert_eq!(results[1].value, 1.0);

    Ok(())
}
