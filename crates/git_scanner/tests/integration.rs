use codeprism_database::Db;
use codeprism_scanner::Scanner;
use git2::{Repository, Signature};
use sqlx::Row;
use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::path::Path;
use tempfile::TempDir;

#[tokio::test]
async fn test_git_scan_integration() -> anyhow::Result<()> {
    // 1. Setup temporary directory and database
    let temp_dir = TempDir::new()?;
    let db_path = temp_dir.path().join("test.db");
    let db_url = format!("sqlite:{}", db_path.to_string_lossy());

    // Setup dummy config
    let config = codeprism_core::CodePrismConfig {
        projects: vec![], // No projects in multi-project format, using legacy fields
        tech_stacks: vec![codeprism_core::TechStack {
            name: "Text".to_string(),
            extensions: vec!["txt".to_string()],
            analyzers: vec!["file_count".to_string()],
            paths: vec!["**/*.txt".to_string()],
            excludes: vec![],
        }],
        global_excludes: vec!["**/exclude_this/**".to_string()],

        database_url: None,
        custom_regex_analyzers: HashMap::new(),
        custom_impl_analyzers: HashMap::new(),
        external_analyzers: HashMap::new(),
        aggregation_views: indexmap::IndexMap::new(),
    };

    File::create(&db_path)?; // Create DB file

    let db = Db::new(&db_url).await?;
    db.migrate().await?;

    // 2. Setup a dummy git repo
    let repo_path = temp_dir.path().join("repo");
    std::fs::create_dir(&repo_path)?;
    let repo = Repository::init(&repo_path)?;

    let sig = Signature::now("Test User", "test@example.com")?;

    // -- COMMIT 1: Add file1.txt --
    let file1_path = repo_path.join("file1.txt");
    {
        let mut file = File::create(&file1_path)?;
        writeln!(file, "Hello World")?;
    }

    let mut index = repo.index()?;
    index.add_path(Path::new("file1.txt"))?;
    index.write()?;
    let tree_id = index.write_tree()?;
    let tree1 = repo.find_tree(tree_id)?;
    let commit1_oid = repo.commit(Some("HEAD"), &sig, &sig, "Initial commit", &tree1, &[])?;
    let commit1_hash = commit1_oid.to_string();

    // -- COMMIT 2: Modify file1.txt, Add file2.txt --
    {
        let mut file = File::create(&file1_path)?; // Overwrite
        writeln!(file, "Hello Modified")?;
    }
    let file2_path = repo_path.join("file2.txt");
    {
        let mut file = File::create(&file2_path)?;
        writeln!(file, "New File")?;
    }
    // Modify file1.txt

    index.add_path(Path::new("file1.txt"))?;
    index.add_path(Path::new("file2.txt"))?;
    index.write()?;
    let tree_id2 = index.write_tree()?;
    let tree2 = repo.find_tree(tree_id2)?;
    let parent = repo.find_commit(commit1_oid)?;
    let commit2_oid = repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        "Second commit",
        &tree2,
        &[&parent],
    )?;
    let commit2_hash = commit2_oid.to_string();

    // 3. Test SNAPSHOT Scan (on Commit 2)
    // Use with_config to inject test configuration
    let scanner = Scanner::with_config(db.clone(), config.clone());
    scanner
        .scan_snapshot(
            repo_path.to_str().unwrap(),
            "test_project",
            Some(&commit2_hash),
        )
        .await?;

    // Verify Audit Trail
    let scan_rec =
        sqlx::query("SELECT commit_hash, scan_mode FROM scans WHERE scan_mode = 'SNAPSHOT'")
            .fetch_one(db.pool())
            .await?;
    assert_eq!(scan_rec.get::<String, _>("commit_hash"), commit2_hash);

    // Verify Counts (file1 and file2 should be found)
    let metrics_count: i64 = sqlx::query("SELECT COUNT(*) FROM metrics")
        .fetch_one(db.pool())
        .await?
        .get(0);
    assert!(metrics_count > 0, "Should have metrics for snapshot");

    // 4. Test DIFF Scan (Commit 1 -> Commit 2)
    // Expect: file1.txt (Modified), file2.txt (Added)
    scanner
        .scan_diff(
            repo_path.to_str().unwrap(),
            "test_project",
            &commit1_hash,
            &commit2_hash,
        )
        .await?;

    let diff_scan = sqlx::query("SELECT id FROM scans WHERE scan_mode = 'DIFF'")
        .fetch_one(db.pool())
        .await?;
    let diff_scan_id: i64 = diff_scan.get("id");

    // Check File Changes
    // Check File Changes
    // We expect: file1.txt: M, file2.txt: A
    let changes: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT DISTINCT file_path, change_type FROM metrics WHERE scan_id = ? ORDER BY file_path",
    )
    .bind(diff_scan_id)
    .fetch_all(db.pool())
    .await?;

    // We expect 2 files changed
    assert_eq!(changes.len(), 2);

    // Check for file1.txt
    let f1 = changes
        .iter()
        .find(|(p, _)| p == "file1.txt")
        .expect("file1.txt missing");
    assert_eq!(f1.1.as_deref(), Some("M"), "file1.txt should be Modified");

    // Check for file2.txt
    let f2 = changes
        .iter()
        .find(|(p, _)| p == "file2.txt")
        .expect("file2.txt missing");
    assert_eq!(f2.1.as_deref(), Some("A"), "file2.txt should be Added");

    // Verify Tech Stack
    let f1_stack: Option<String> = sqlx::query_scalar(
        "SELECT tech_stack FROM metrics WHERE scan_id = ? AND file_path = 'file1.txt' LIMIT 1",
    )
    .bind(diff_scan_id)
    .fetch_one(db.pool())
    .await?;
    assert_eq!(
        f1_stack.as_deref(),
        Some("Text"),
        "file1.txt should have Text stack"
    );

    // Verify Values for Modify (file1.txt)
    // Verify Values for Modify (file1.txt)
    let f1_metrics = sqlx::query("SELECT value_before, value_after FROM metrics WHERE scan_id = ? AND file_path = 'file1.txt' AND metric_key = 'count'")

        .bind(diff_scan_id)
        .fetch_one(db.pool())
        .await?;
    // For Modify, both should be present (1.0)
    assert_eq!(f1_metrics.get::<Option<f64>, _>("value_before"), Some(1.0));
    assert_eq!(f1_metrics.get::<Option<f64>, _>("value_after"), Some(1.0));

    // Verify Values for Add (file2.txt)
    // Verify Values for Add (file2.txt)
    let f2_metrics = sqlx::query("SELECT value_before, value_after FROM metrics WHERE scan_id = ? AND file_path = 'file2.txt' AND metric_key = 'count'")

        .bind(diff_scan_id)
        .fetch_one(db.pool())
        .await?;
    // For Add, before is null, after is 1.0
    assert_eq!(f2_metrics.get::<Option<f64>, _>("value_before"), None);
    assert_eq!(f2_metrics.get::<Option<f64>, _>("value_after"), Some(1.0));

    // 5. Test DIFF Scan (Commit 2 -> Commit 3 [Delete file2.txt])
    // Create Commit 3: Delete file2.txt
    std::fs::remove_file(&file2_path)?;
    let mut index = repo.index()?;
    index.remove_path(Path::new("file2.txt"))?;
    index.write()?;
    let tree_id3 = index.write_tree()?;
    let tree3 = repo.find_tree(tree_id3)?;
    let parent2 = repo.find_commit(commit2_oid)?;
    let commit3_oid = repo.commit(
        Some("HEAD"),
        &sig,
        &sig,
        "Third commit (Delete)",
        &tree3,
        &[&parent2],
    )?;
    let commit3_hash = commit3_oid.to_string();

    scanner
        .scan_diff(
            repo_path.to_str().unwrap(),
            "test_project",
            &commit2_hash,
            &commit3_hash,
        )
        .await?;

    let diff_scan_delete =
        sqlx::query("SELECT id FROM scans WHERE scan_mode = 'DIFF' AND commit_hash = ?")
            .bind(&commit3_hash)
            .fetch_one(db.pool())
            .await?;
    let diff_scan_delete_id: i64 = diff_scan_delete.get("id");

    // Verify Deletion Metrics
    // file2.txt should have D, value_before=Some, value_after=None
    let delete_metric = sqlx::query("SELECT change_type, value_before, value_after, tech_stack FROM metrics WHERE scan_id = ? AND file_path = 'file2.txt' AND metric_key = 'count'")

        .bind(diff_scan_delete_id)
        .fetch_one(db.pool())
        .await?;

    assert_eq!(
        delete_metric
            .get::<Option<String>, _>("change_type")
            .as_deref(),
        Some("D")
    );
    assert!(
        delete_metric
            .get::<Option<f64>, _>("value_before")
            .is_some()
    );
    assert!(delete_metric.get::<Option<f64>, _>("value_after").is_none());
    // Tech Stack might be null on delete depending on implementation since we don't have content to analyze extension easily if we don't look at old path?
    // Wait, we do look at old path. Let's check if tech_stack is populated.
    // In delete, we process `delta.old_file()`. path comes from old file. Extension should work.
    let ts: Option<String> = delete_metric.get("tech_stack");
    assert_eq!(ts.as_deref(), Some("Text"));

    Ok(())
}
