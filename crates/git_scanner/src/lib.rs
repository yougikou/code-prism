use anyhow::{Context, Result};
use codeprism_analyzer::{
    Analyzer, CharCountAnalyzer, FileCountAnalyzer, RegexAnalyzer, ScriptAnalyzer, WasmAnalyzer,
};

use codeprism_database::Db;
use git2::{Delta, ObjectType, Repository, Tree};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::Path;

use tokio::sync::mpsc;

use codeprism_core::CodePrismConfig;
use indicatif::{ProgressBar, ProgressStyle};
use std::sync::{Arc, Mutex};

pub struct Scanner {
    db: Db,
    analyzers: HashMap<String, Box<dyn Analyzer>>,
    config: Arc<CodePrismConfig>,
    scan_job_id: Option<i64>,
    // Scan summary tracking (accumulated during scan lifecycle)
    analyzer_load_errors: Vec<String>,
    analyzer_exec_count: HashMap<String, u64>,
    analyzer_error_details: HashMap<String, Vec<String>>,
    // Track tag keys from this scan for auto-index creation
    seen_tag_keys: Mutex<HashSet<String>>,
}

// Event to decouple Git (Sync) from DB (Async)

#[derive(Debug)]
enum ScanEvent {
    Start {
        total: Option<usize>,
    },
    Ignored,
    FileFound {
        path: String,
        content: Option<String>,
        old_content: Option<String>,
        change_type: String,
        old_path: Option<String>,
        tech_stack: Option<String>,
    },
}

impl Scanner {
    pub fn new(db: Db) -> Self {
        // Try to load config from current directory, fallback to default
        let config = if let Ok(content) = std::fs::read_to_string("codeprism.yaml") {
            serde_yaml::from_str(&content).unwrap_or_default()
        } else {
            CodePrismConfig::default()
        };

        // Initialize with config defaults
        Scanner::with_config(db, config)
    }

    pub fn with_config(db: Db, config: CodePrismConfig) -> Self {
        let mut analyzers: HashMap<String, Box<dyn Analyzer>> = HashMap::new();
        let mut load_errors: Vec<String> = Vec::new();

        // 1. Built-in Analyzers
        let fc = FileCountAnalyzer::new();
        analyzers.insert(fc.id().to_string(), Box::new(fc));

        let cc = CharCountAnalyzer::new();
        analyzers.insert(cc.id().to_string(), Box::new(cc));

        // 2. Load Analyzers from all sources (Root + All Projects)
        // We collect all definitions first, though they might collide by name.
        // Last one wins if there's a name collision.
        let mut regex_defs = config.custom_regex_analyzers.clone();
        let mut impl_defs = config.custom_impl_analyzers.clone();
        let mut external_defs = config.external_analyzers.clone();

        for project in &config.projects {
            regex_defs.extend(project.custom_regex_analyzers.clone());
            impl_defs.extend(project.custom_impl_analyzers.clone());
            external_defs.extend(project.external_analyzers.clone());
        }

        // Apply Regex Analyzers
        for (name, def) in &regex_defs {
            let (pattern, tags, scan_mode, change_type) = match def {
                codeprism_core::CustomAnalyzerDef::Pattern(p) => {
                    let mut tags = std::collections::HashMap::new();
                    tags.insert(codeprism_core::TAG_METRIC.to_string(), "matches".to_string());
                    (p.clone(), tags, None, None)
                }
                codeprism_core::CustomAnalyzerDef::Config {
                    pattern,
                    scan_mode,
                    change_type,
                    ..
                } => (
                    pattern.clone(),
                    def.resolve_tags(),
                    scan_mode.clone(),
                    change_type.clone(),
                ),
            };

            match RegexAnalyzer::new(name, &pattern, tags, scan_mode, change_type)
            {
                Ok(ra) => {
                    analyzers.insert(ra.id().to_string(), Box::new(ra));
                }
                Err(e) => {
                    let msg = format!("Failed to compile regex analyzer '{}': {}", name, e);
                    eprintln!("{}", msg);
                    load_errors.push(msg);
                }
            }
        }

        // Apply External Wasm Analyzers
        for (name, path) in &external_defs {
            match WasmAnalyzer::new(name, path) {
                Ok(wa) => {
                    analyzers.insert(wa.id().to_string(), Box::new(wa));
                }
                Err(e) => {
                    let msg = format!(
                        "Failed to load wasm analyzer '{}' from '{}': {}",
                        name, path, e
                    );
                    eprintln!("{}", msg);
                    load_errors.push(msg);
                }
            }
        }

        // 4. Auto-discover Python Analyzers in 'custom_analyzers/'
        if let Ok(entries) = std::fs::read_dir("custom_analyzers") {
            for entry in entries.filter_map(Result::ok) {
                let path = entry.path();
                if path.is_file() {
                    if let Some(ext) = path.extension() {
                        if ext == "py" {
                            if let Some(stem) = path.file_stem().and_then(|s| s.to_str()) {
                                let analyzer_id = stem.to_string();
                                let full_path = path.to_string_lossy().to_string();

                                // Check for overrides (search root then projects)
                                let (tag_overrides, scan_mode, change_type) =
                                    if let Some(conf) = impl_defs.get(&analyzer_id) {
                                        (
                                            conf.resolve_tags(),
                                            conf.scan_mode.clone(),
                                            conf.change_type.clone(),
                                        )
                                    } else {
                                        (HashMap::new(), None, None)
                                    };

                                // Create ScriptAnalyzer
                                let sa = ScriptAnalyzer::new(
                                    &analyzer_id,
                                    &full_path,
                                    tag_overrides,
                                    scan_mode,
                                    change_type,
                                );
                                analyzers.insert(analyzer_id, Box::new(sa));
                                println!("Loaded custom analyzer: {}", stem);
                            }
                        }
                    }
                }
            }
        }

        Self {
            db,
            analyzers,
            config: Arc::new(config),
            scan_job_id: None,
            analyzer_load_errors: load_errors,
            analyzer_exec_count: HashMap::new(),
            analyzer_error_details: HashMap::new(),
            seen_tag_keys: Mutex::new(HashSet::new()),
        }
    }

    pub fn set_scan_job_id(&mut self, job_id: i64) {
        self.scan_job_id = Some(job_id);
    }

    async fn update_progress(&self, progress: u8, message: &str) {
        if let Some(job_id) = self.scan_job_id {
            sqlx::query(
                "UPDATE scan_jobs SET progress = ?, progress_message = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(progress as i32)
            .bind(message)
            .bind(job_id)
            .execute(self.db.pool())
            .await
            .ok();
        }
    }

    pub async fn scan_snapshot(
        &mut self,
        repo_path: &str,
        project_name: &str,
        commit_ref: Option<&str>,
    ) -> Result<i64> {
        let repo_path = repo_path.to_string();
        let commit_ref = commit_ref.map(|s| s.to_string());
        let project_name = project_name.to_string();

        // 1. Resolve Commit Info (Sync, can be done before channel)
        // We open repo briefly to get metadata
        let (commit_hash, branch_name) = {
            let repo = Repository::open(&repo_path).context("Failed to open git repository")?;
            Scanner::resolve_commit_info(&repo, commit_ref.as_deref())?
        };

        println!(
            "Snapshot Scanning: Project={}, Commit={}, Branch={}",
            project_name, commit_hash, branch_name
        );

        self.update_progress(2, "Resolving commit info").await;

        let project_id = self
            .get_or_create_project(&project_name, &repo_path)
            .await?;
        let scan_id = self
            .create_scan_record(project_id, &commit_hash, &branch_name, "SNAPSHOT", None)
            .await?;

        self.update_progress(5, "Creating scan records").await;

        // Resolve Project Config
        let project_config =
            Arc::new(self.config.get_project(&project_name).unwrap_or_else(|| {
                // If project not found specifically, create one from root settings or default
                println!(
                    "Warning: Project '{}' not found in config, using default settings.",
                    project_name
                );
                self.config.get_default_project()
            }));

        // 2. Spawn Blocking Git Walker
        self.update_progress(8, "Walking repository tree").await;
        let (tx, mut rx) = mpsc::channel(100);
        let repo_path_clone = repo_path.clone();
        let commit_hash_clone = commit_hash.clone();
        let project_config_clone = project_config.clone();

        tokio::task::spawn_blocking(move || {
            let res = (|| -> Result<()> {
                let repo = Repository::open(&repo_path_clone)?;
                let obj = repo.revparse_single(&commit_hash_clone)?;
                let commit = obj
                    .into_commit()
                    .map_err(|_| anyhow::anyhow!("Not a commit"))?;
                let tree = commit.tree()?;

                Scanner::walk_tree_sync(&repo, &tree, &tx, &project_config_clone)?;
                Ok(())
            })();
            if let Err(e) = res {
                eprintln!("Git Walk Error: {}", e);
            }
        });

        // 3. Process Events (Async DB)

        let pb = ProgressBar::new_spinner();
        pb.set_style(
            ProgressStyle::default_spinner()
                .template("{spinner:.green} [{elapsed_precise}] {msg}")
                .unwrap(),
        );
        pb.set_message("Initializing scan...");

        let mut processed_count = 0;
        let mut last_reported_progress = 0u8;
        let mut last_progress_update = std::time::Instant::now();

        while let Some(event) = rx.recv().await {
            match event {
                ScanEvent::Start { total: _ } => {
                    pb.set_message("Scanning files...");
                }
                ScanEvent::Ignored => {}
                ScanEvent::FileFound {
                    path,
                    content,
                    old_content: _,
                    change_type,
                    old_path,
                    tech_stack,
                } => {
                    processed_count += 1;
                    pb.set_message(format!("Scanned {} files: {}", processed_count, path));
                    pb.tick();

                    // Report progress using asymptotic formula (unknown total files)
                    let pct = 8u8.saturating_add(
                        (82.0 * (processed_count as f64 / (processed_count as f64 + 500.0))) as u8,
                    );
                    if pct != last_reported_progress
                        && last_progress_update.elapsed() >= std::time::Duration::from_millis(500)
                    {
                        self.update_progress(
                            pct.min(90),
                            &format!("Processing file {}: {}", processed_count, path),
                        )
                        .await;
                        last_reported_progress = pct;
                        last_progress_update = std::time::Instant::now();
                    }

                    // Run analyzers if content is available
                    let (file_metrics, file_matches) = if let Some(c) = content {
                        self.analyze_file_content(
                            &path,
                            &c,
                            "SNAPSHOT",
                            &change_type,
                            tech_stack.as_deref(),
                            &project_config,
                        )
                    } else {
                        (Vec::<codeprism_core::MetricEntry>::new(), Vec::<codeprism_core::MatchDetail>::new())
                    };

                    self.save_metrics(
                        scan_id,
                        &path,
                        &change_type,
                        old_path.as_deref(),
                        tech_stack.as_deref(),
                        file_metrics,
                        Vec::new(),
                    )
                    .await?;

                    self.save_matches(scan_id, file_matches).await?;
                }
            }
        }

        pb.finish_with_message(format!(
            "Snapshot Scan Complete. Scanned {} files.",
            processed_count
        ));

        self.update_progress(92, "Auto-creating indexes").await;
        // Auto-create expression indexes for newly seen tag keys
        self.ensure_tag_indexes().await;

        self.update_progress(95, "Saving scan summary").await;
        // Save scan summary
        if let Err(e) = self.save_scan_summary(scan_id, processed_count).await {
            eprintln!("Failed to save scan summary: {}", e);
        }

        self.update_progress(100, "Scan complete").await;
        println!("Snapshot Scan Complete. Scan ID: {}", scan_id);
        Ok(scan_id)
    }

    pub async fn scan_diff(
        &mut self,
        repo_path: &str,
        project_name: &str,
        base_ref: &str,
        target_ref: &str,
    ) -> Result<i64> {
        let repo_path = repo_path.to_string();
        let base_ref = base_ref.to_string();
        let target_ref = target_ref.to_string();
        let project_name = project_name.to_string();

        let (target_hash, target_branch, base_hash) = {
            let repo = Repository::open(&repo_path).context("Failed to open git repository")?;
            let (h, b) = Scanner::resolve_commit_info(&repo, Some(&target_ref))?;
            let (bh, _) = Scanner::resolve_commit_info(&repo, Some(&base_ref))?;
            (h, b, bh)
        };

        println!(
            "Diff Scanning: Project={} | {} .. {}",
            project_name, base_hash, target_hash
        );

        self.update_progress(2, "Resolving commit info").await;

        let project_id = self
            .get_or_create_project(&project_name, &repo_path)
            .await?;
        let scan_id = self
            .create_scan_record(
                project_id,
                &target_hash,
                &target_branch,
                "DIFF",
                Some(&base_hash),
            )
            .await?;

        self.update_progress(5, "Creating scan records").await;

        // Resolve Project Config
        let project_config =
            Arc::new(self.config.get_project(&project_name).unwrap_or_else(|| {
                println!(
                    "Warning: Project '{}' not found in config, using default settings.",
                    project_name
                );
                self.config.get_default_project()
            }));

        // Spawn Blocking Git Diff
        self.update_progress(8, "Computing diff").await;
        let (tx, mut rx) = mpsc::channel(100);
        let repo_path_clone = repo_path.clone();
        let project_config_clone = project_config.clone();

        tokio::task::spawn_blocking(move || {
            let res = (|| -> Result<()> {
                let repo = Repository::open(&repo_path_clone)?;

                let base_obj = repo.revparse_single(&base_hash)?;
                let base_tree = base_obj.peel_to_commit()?.tree()?;

                let target_obj = repo.revparse_single(&target_hash)?;
                let target_tree = target_obj.peel_to_commit()?.tree()?;

                Scanner::process_diff_sync(
                    &repo,
                    &base_tree,
                    &target_tree,
                    &tx,
                    &project_config_clone,
                )?;
                Ok(())
            })();
            if let Err(e) = res {
                eprintln!("Git Diff Error: {}", e);
            }
        });

        // Process Events
        // Process Events
        let pb = ProgressBar::new(0); // Optional length, will set on Start
        pb.set_style(
            ProgressStyle::default_bar()
                .template(
                    "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos}/{len} {msg}",
                )
                .unwrap()
                .progress_chars("#>-"),
        );
        pb.set_message("Calculating diff...");

        let mut processed_count = 0u64;
        let mut total_deltas = 0usize;
        let mut last_reported_progress = 0u8;
        let mut last_progress_update = std::time::Instant::now();

        while let Some(event) = rx.recv().await {
            match event {
                ScanEvent::Start { total: Some(t) } => {
                    total_deltas = t;
                    pb.set_length(t as u64);
                    pb.set_message("Processing changes...");
                    self.update_progress(10, &format!("Processing {} changes", t)).await;
                }
                ScanEvent::Start { total: None } => {
                    // Should not happen in diff mode usually
                }
                ScanEvent::Ignored => {
                    pb.inc(1);
                }
                ScanEvent::FileFound {
                    path,
                    content,
                    old_content,
                    change_type,
                    old_path,
                    tech_stack,
                } => {
                    pb.set_message(format!("Processing: {}", path));
                    pb.inc(1);
                    processed_count += 1;

                    if total_deltas > 0 {
                        let pct = 10u8.saturating_add(
                            (80 * processed_count as usize / total_deltas) as u8,
                        );
                        if pct != last_reported_progress
                            && last_progress_update.elapsed() >= std::time::Duration::from_millis(500)
                        {
                            self.update_progress(
                                pct.min(90),
                                &format!(
                                    "Processing change {}/{}: {}",
                                    processed_count, total_deltas, path
                                ),
                            )
                            .await;
                            last_reported_progress = pct;
                            last_progress_update = std::time::Instant::now();
                        }
                    }

                    // Diff Mode: Analyze both if available
                    let old_path_ref = old_path.as_deref().unwrap_or(&path);
                    let (old_metrics, old_matches) = if let Some(c) = old_content {
                        self.analyze_file_content(
                            old_path_ref,
                            &c,
                            "DIFF",
                            &change_type,
                            tech_stack.as_deref(),
                            &project_config,
                        )
                    } else {
                        (Vec::<codeprism_core::MetricEntry>::new(), Vec::<codeprism_core::MatchDetail>::new())
                    };

                    let (new_metrics, new_matches) = if let Some(c) = content {
                        self.analyze_file_content(
                            &path,
                            &c,
                            "DIFF",
                            &change_type,
                            tech_stack.as_deref(),
                            &project_config,
                        )
                    } else {
                        (Vec::<codeprism_core::MetricEntry>::new(), Vec::<codeprism_core::MatchDetail>::new())
                    };

                    self.save_metrics(
                        scan_id,
                        &path,
                        &change_type,
                        old_path.as_deref(),
                        tech_stack.as_deref(),
                        new_metrics,
                        old_metrics,
                    )
                    .await?;

                    // Save matches for both old and new content
                    self.save_matches(scan_id, old_matches).await?;
                    self.save_matches(scan_id, new_matches).await?;
                }
            }
        }
        pb.finish_with_message("Diff Scan Complete");

        self.update_progress(92, "Auto-creating indexes").await;
        // Auto-create expression indexes for newly seen tag keys
        self.ensure_tag_indexes().await;

        self.update_progress(95, "Saving scan summary").await;
        // Save scan summary
        if let Err(e) = self.save_scan_summary(scan_id, processed_count).await {
            eprintln!("Failed to save scan summary: {}", e);
        }

        self.update_progress(100, "Diff scan complete").await;
        println!("Diff Scan Complete. Scan ID: {}", scan_id);
        Ok(scan_id)
    }

    // --- Sync Git Logic (Runs in worker thread) ---

    fn resolve_commit_info(repo: &Repository, ref_name: Option<&str>) -> Result<(String, String)> {
        let obj = match ref_name {
            Some(r) => repo
                .revparse_single(r)
                .context(format!("Failed to find reference: {}", r))?,
            None => match repo.head() {
                Ok(h) => h.peel(ObjectType::Commit)?,
                Err(e) if e.code() == git2::ErrorCode::UnbornBranch => {
                    return Err(anyhow::anyhow!(
                        "The repository has no commits yet. snapshot mode requires at least one commit."
                    ));
                }
                Err(e) => return Err(e.into()),
            },
        };
        let commit = obj.peel_to_commit()?;
        let hash = commit.id().to_string();

        // Try branch name
        let branch = if ref_name.is_none() {
            repo.head()
                .ok()
                .and_then(|h| h.shorthand().map(|s| s.to_string()))
                .unwrap_or_else(|| "HEAD".to_string())
        } else {
            ref_name.unwrap().to_string()
        };
        Ok((hash, branch))
    }

    fn walk_tree_sync(
        repo: &Repository,
        tree: &Tree<'_>,
        tx: &mpsc::Sender<ScanEvent>,
        project_config: &codeprism_core::ProjectConfig,
    ) -> Result<()> {
        tree.walk(git2::TreeWalkMode::PreOrder, |root, entry| {
            if let Some(ObjectType::Blob) = entry.kind() {
                let filename = entry.name().unwrap_or("unknown");
                let path = format!("{}{}", root, filename);
                // Check Exclusions
                if project_config.is_excluded(&path) {
                    return git2::TreeWalkResult::Ok;
                }

                // Analyze?

                let mut content = None;
                if let Ok(blob) = entry.to_object(repo).and_then(|o| {
                    o.into_blob()
                        .map_err(|_| git2::Error::from_str("Not a blob"))
                }) {
                    if !blob.is_binary() {
                        if let Ok(c) = std::str::from_utf8(blob.content()) {
                            content = Some(c.to_string());
                        }
                    }
                }

                let tech_stack = project_config.get_tech_stack_for_file(&path);

                let _ = tx.blocking_send(ScanEvent::FileFound {
                    path,
                    content,
                    old_content: None,            // Snapshot has no old content
                    change_type: "A".to_string(), // In snapshot we treat as Add or just N
                    old_path: None,
                    tech_stack,
                });
            }
            git2::TreeWalkResult::Ok
        })?;
        Ok(())
    }

    fn process_diff_sync(
        repo: &Repository,
        base: &Tree<'_>,
        target: &Tree<'_>,
        tx: &mpsc::Sender<ScanEvent>,
        project_config: &codeprism_core::ProjectConfig,
    ) -> Result<()> {
        let mut diff = repo.diff_tree_to_tree(Some(base), Some(target), None)?;
        diff.find_similar(None)?;

        let delta_count = diff.deltas().len();

        let _ = tx.blocking_send(ScanEvent::Start {
            total: Some(delta_count),
        });

        for i in 0..delta_count {
            if let Some(delta) = diff.get_delta(i) {
                let file_path = delta
                    .new_file()
                    .path()
                    .unwrap_or(Path::new("unknown"))
                    .to_string_lossy()
                    .to_string();

                // Check Exclusions for the new file path (Target)
                if project_config.is_excluded(&file_path) {
                    let _ = tx.blocking_send(ScanEvent::Ignored);
                    continue;
                }

                let old_path_str = delta
                    .old_file()
                    .path()
                    .map(|p| p.to_string_lossy().to_string());

                let change_type = match delta.status() {
                    Delta::Added => "A",
                    Delta::Modified => "M",
                    Delta::Deleted => "D",
                    Delta::Renamed => "M", // Treat rename as Modified content-wise usually, or handled specially
                    _ => "U",
                };

                if change_type == "U" {
                    continue;
                }

                // Retrieve Content
                let mut new_content = None;
                let mut old_content = None;

                // New Content (Target) - for A, M, Renamed
                if change_type != "D" {
                    if let Ok(blob) = repo.find_blob(delta.new_file().id()) {
                        if !blob.is_binary() {
                            if let Ok(c) = std::str::from_utf8(blob.content()) {
                                new_content = Some(c.to_string());
                            }
                        }
                    }
                }

                // Old Content (Base) - for M, D, Renamed
                if change_type != "A" {
                    if let Ok(blob) = repo.find_blob(delta.old_file().id()) {
                        if !blob.is_binary() {
                            if let Ok(c) = std::str::from_utf8(blob.content()) {
                                old_content = Some(c.to_string());
                            }
                        }
                    }
                }

                let final_old_path =
                    if delta.status() == Delta::Renamed || delta.status() == Delta::Deleted {
                        old_path_str.clone()
                    } else {
                        None
                    };

                // For Deleted files, file_path in DB usually should reflect "where it was" or we just keep old_path.
                // In this schema, we put the path in `file_path`. If deleted, `file_path` is usually the old name in Git structure,
                // but `delta.new_file().path()` might be empty or same?
                // `delta.old_file().path()` is reliable for Deleted.
                let final_path = if change_type == "D" {
                    old_path_str.clone().unwrap_or(file_path)
                } else {
                    file_path
                };

                let tech_stack = project_config.get_tech_stack_for_file(&final_path);

                let _ = tx.blocking_send(ScanEvent::FileFound {
                    path: final_path,
                    content: new_content,
                    old_content,
                    change_type: change_type.to_string(),
                    old_path: final_old_path,
                    tech_stack,
                });
            }
        }
        Ok(())
    }

    /// Save a scan execution summary to the scan_summaries table.
    async fn save_scan_summary(&self, scan_id: i64, total_files: u64) -> anyhow::Result<()> {
        let total_errors: u64 = self
            .analyzer_error_details
            .values()
            .map(|v| v.len() as u64)
            .sum();

        let load_errors_json = serde_json::to_string(&self.analyzer_load_errors)?;

        let analyzer_stats: Vec<serde_json::Value> = self
            .analyzers
            .keys()
            .map(|id| {
                let files = self.analyzer_exec_count.get(id).copied().unwrap_or(0);
                let errors = self
                    .analyzer_error_details
                    .get(id)
                    .map(|v| v.len() as u64)
                    .unwrap_or(0);
                let details = self
                    .analyzer_error_details
                    .get(id)
                    .cloned()
                    .unwrap_or_default();
                serde_json::json!({
                    "analyzer_id": id,
                    "files_analyzed": files,
                    "execution_errors": errors,
                    "error_details": details,
                })
            })
            .collect();
        let analyzer_stats_json = serde_json::to_string(&analyzer_stats)?;

        let total_executions: u64 = self.analyzer_exec_count.values().sum();
        let executed_count = self
            .analyzer_exec_count
            .values()
            .filter(|&&c| c > 0)
            .count() as u64;

        sqlx::query(
            "INSERT INTO scan_summaries \
             (scan_id, total_files_scanned, total_analyzers_loaded, \
              total_analyzers_executed, total_analyzer_executions, total_errors, \
              load_errors, analyzer_stats) \
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(scan_id)
        .bind(total_files as i64)
        .bind(self.analyzers.len() as i64)
        .bind(executed_count as i64)
        .bind(total_executions as i64)
        .bind(total_errors as i64)
        .bind(&load_errors_json)
        .bind(&analyzer_stats_json)
        .execute(self.db.pool())
        .await?;

        Ok(())
    }

    // --- DB Helpers ---

    async fn get_or_create_project(&self, name: &str, path: &str) -> Result<i64> {
        let rec = sqlx::query_scalar::<_, i64>(
            "INSERT INTO projects (name, repo_path) VALUES (?, ?)
             ON CONFLICT(name) DO UPDATE SET repo_path = excluded.repo_path
             RETURNING id",
        )
        .bind(name)
        .bind(path)
        .fetch_one(self.db.pool())
        .await
        .context("Failed to get or create project")?;

        Ok(rec)
    }

    async fn create_scan_record(
        &self,
        project_id: i64,
        commit: &str,
        branch: &str,
        mode: &str,
        base: Option<&str>,
    ) -> Result<i64> {
        let rec = sqlx::query!(
            "INSERT INTO scans (project_id, commit_hash, branch_name, scan_mode, base_commit_hash) 
             VALUES (?, ?, ?, ?, ?) 
             RETURNING id",
            project_id,
            commit,
            branch,
            mode,
            base
        )
        .fetch_one(self.db.pool())
        .await
        .context("Failed to create scan record")?;
        Ok(rec.id.expect("ID"))
    }

    // record_file_change_with_old removed: merged into save_metrics

    fn analyze_file_content(
        &mut self,
        file_path: &str,
        content: &str,
        scan_mode: &str,
        change_type: &str,
        tech_stack_name: Option<&str>,
        project_config: &codeprism_core::ProjectConfig,
    ) -> (Vec<codeprism_core::MetricEntry>, Vec<codeprism_core::MatchDetail>) {
        let mut results: Vec<codeprism_core::MetricEntry> = Vec::new();
        let mut all_matches: Vec<codeprism_core::MatchDetail> = Vec::new();
        let mut analyzers_to_run: Vec<String> = vec!["file_count".to_string()];

        if let Some(ts_name) = tech_stack_name {
            if let Some(stack) = project_config
                .tech_stacks
                .iter()
                .find(|s| s.name == ts_name)
            {
                for a in &stack.analyzers {
                    if !analyzers_to_run.contains(a) {
                        analyzers_to_run.push(a.clone());
                    }
                }
            }
        }

        for analyzer_id in analyzers_to_run {
            if let Some(analyzer) = self.analyzers.get(&analyzer_id) {
                // Respect per-analyzer scan_mode filter (None = all modes)
                if let Some(mode) = analyzer.scan_mode() {
                    if mode != scan_mode {
                        continue;
                    }
                }
                // Respect per-analyzer change_type filter (None = all types)
                if let Some(ct) = analyzer.change_type() {
                    if ct != change_type {
                        continue;
                    }
                }

                // Isolate each analyzer with catch_unwind so a panic in one
                // (e.g. Wasm runtime crash, Script process deadlock) does not
                // crash the entire scan. Failed results are discarded and the
                // error is logged; remaining analyzers continue unaffected.
                let metrics_result = std::panic::catch_unwind(
                    std::panic::AssertUnwindSafe(|| analyzer.analyze(file_path, content)),
                );

                match metrics_result {
                    Ok(metrics) => {
                        *self.analyzer_exec_count.entry(analyzer_id.clone()).or_insert(0) += 1;
                        results.extend(metrics);
                    }
                    Err(panic_info) => {
                        let msg = panic_info
                            .downcast_ref::<&str>()
                            .map(|s| s.to_string())
                            .or_else(|| panic_info.downcast_ref::<String>().cloned())
                            .unwrap_or_else(|| "unknown panic".to_string());
                        eprintln!(
                            "Analyzer '{}' panicked while processing '{}': {}. \
                             Its results have been skipped.",
                            analyzer_id, file_path, msg,
                        );
                        self.analyzer_error_details
                            .entry(analyzer_id.clone())
                            .or_default()
                            .push(format!("{}: {}", file_path, msg));
                        // Skip match extraction if analyze panicked
                        continue;
                    }
                }

                // Extract matches (separate catch_unwind to isolate from analyze panics)
                let matches_result = std::panic::catch_unwind(
                    std::panic::AssertUnwindSafe(|| analyzer.extract_matches(file_path, content)),
                );

                match matches_result {
                    Ok(matches) => {
                        all_matches.extend(matches);
                    }
                    Err(panic_info) => {
                        let msg = panic_info
                            .downcast_ref::<&str>()
                            .map(|s| s.to_string())
                            .or_else(|| panic_info.downcast_ref::<String>().cloned())
                            .unwrap_or_else(|| "unknown panic".to_string());
                        eprintln!(
                            "Analyzer '{}' extract_matches panicked for '{}': {}",
                            analyzer_id, file_path, msg,
                        );
                    }
                }
            }
        }
        (results, all_matches)
    }

    async fn save_metrics(
        &self,
        scan_id: i64,
        file_path: &str,
        change_type: &str,
        old_path: Option<&str>,
        tech_stack: Option<&str>,
        new_metrics: Vec<codeprism_core::MetricEntry>,
        old_metrics: Vec<codeprism_core::MetricEntry>,
    ) -> Result<()> {
        // Track tag keys for auto-indexing
        for m in &old_metrics {
            for k in m.tags.keys() {
                self.seen_tag_keys.lock().unwrap().insert(k.clone());
            }
        }
        for m in &new_metrics {
            for k in m.tags.keys() {
                self.seen_tag_keys.lock().unwrap().insert(k.clone());
            }
        }

        // Deterministic JSON serialization for dedup hashing
        fn serialize_tags(tags: &HashMap<String, String>) -> String {
            let sorted: BTreeMap<_, _> = tags.iter().collect();
            serde_json::to_string(&sorted).unwrap_or_default()
        }

        #[derive(Hash, Eq, PartialEq)]
        struct MetricKey {
            analyzer: String,
            tags_json: String,
            scope: Option<String>,
        }

        // Map to store values: Key -> (ValueBefore, ValueAfter)
        let mut merged_data: HashMap<MetricKey, (Option<f64>, Option<f64>)> = HashMap::new();

        for m in &old_metrics {
            let key = MetricKey {
                analyzer: m.analyzer_id.clone(),
                tags_json: serialize_tags(&m.tags),
                scope: m.scope.clone(),
            };
            merged_data.entry(key).or_insert((None, None)).0 = Some(m.value);
        }

        for m in &new_metrics {
            let key = MetricKey {
                analyzer: m.analyzer_id.clone(),
                tags_json: serialize_tags(&m.tags),
                scope: m.scope.clone(),
            };
            merged_data.entry(key).or_insert((None, None)).1 = Some(m.value);
        }

        for (key, (val_before, val_after)) in merged_data {
            // Skip entries where both values are zero or NULL — redundant data
            let before_zero = val_before.map(|v| v == 0.0).unwrap_or(true);
            let after_zero = val_after.map(|v| v == 0.0).unwrap_or(true);
            if before_zero && after_zero {
                continue;
            }
            sqlx::query(
                "INSERT INTO metrics (scan_id, file_path, change_type, old_file_path, tech_stack, analyzer_id, tags, value_before, value_after, scope)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(scan_id)
            .bind(file_path)
            .bind(change_type)
            .bind(old_path)
            .bind(tech_stack)
            .bind(&key.analyzer)
            .bind(&key.tags_json)
            .bind(val_before)
            .bind(val_after)
            .bind(&key.scope)
            .execute(self.db.pool())
            .await?;
        }
        Ok(())
    }

    async fn save_matches(
        &self,
        scan_id: i64,
        matches: Vec<codeprism_core::MatchDetail>,
    ) -> Result<()> {
        for m in matches {
            sqlx::query(
                "INSERT INTO matches (scan_id, file_path, analyzer_id, line_number, column_start, column_end, matched_text, context_before, context_after)
                 VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
            )
            .bind(scan_id)
            .bind(&m.file_path)
            .bind(&m.analyzer_id)
            .bind(m.line_number as i64)
            .bind(m.column_start.map(|v| v as i64))
            .bind(m.column_end.map(|v| v as i64))
            .bind(&m.matched_text)
            .bind(&m.context_before)
            .bind(&m.context_after)
            .execute(self.db.pool())
            .await?;
        }
        Ok(())
    }

    /// Auto-create expression indexes for tag keys discovered during this scan.
    /// Uses CREATE INDEX IF NOT EXISTS so repeated calls are harmless.
    async fn ensure_tag_indexes(&self) {
        let keys = self.seen_tag_keys.lock().unwrap().clone();
        for key in keys {
            let idx_name: String = key
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '_')
                .collect();
            let idx_name = if idx_name.is_empty() {
                "idx_metrics_tag_unknown".to_string()
            } else {
                format!("idx_metrics_tag_{}", idx_name)
            };
            let escaped_key = key.replace('\\', "\\\\").replace('"', "\\\"");
            let sql = format!(
                "CREATE INDEX IF NOT EXISTS {idx_name} ON metrics(json_extract(tags, '$.\"{escaped_key}\"'))"
            );
            if let Err(e) = sqlx::query(&sql).execute(self.db.pool()).await {
                eprintln!("Warning: failed to create index for tag '{}': {}", key, e);
            }
        }
    }
}
