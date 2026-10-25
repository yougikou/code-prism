use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use codeprism_database::Db;
use codeprism_scanner::Scanner;

fn detect_python_interpreter() -> Option<&'static str> {
    let candidates: &[&str] = if cfg!(windows) {
        &["python", "python3", "py"]
    } else {
        &["python3", "python"]
    };

    candidates.iter().copied().find(|candidate| {
        std::process::Command::new(candidate)
            .arg("--version")
            .output()
            .is_ok_and(|output| output.status.success())
    })
}

#[derive(Parser)]
#[command(name = "codeprism")]
#[command(about = "Code Prism CLI", long_about = None)]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,

    /// Path to configuration file (default: codeprism.yaml)
    #[arg(long, global = true)]
    config: Option<String>,
}

#[derive(Copy, Clone, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum ScanMode {
    Snapshot,
    Diff,
}

#[derive(Subcommand)]
enum Commands {
    /// Initialize the database
    Init,
    /// Scan a directory or git repository
    Scan {
        /// Path to the project root (default: current directory)
        #[arg(default_value = ".")]
        path: String,

        /// Project name (optional, defaults to directory name)
        #[arg(short, long)]
        project: Option<String>,

        /// Scan mode: snapshot or diff
        #[arg(long, value_enum, default_value_t = ScanMode::Snapshot)]
        mode: ScanMode,

        /// Commit or Reference to scan (Snapshot mode)
        #[arg(long)]
        commit: Option<String>,

        /// Base commit for Diff mode
        #[arg(long)]
        base: Option<String>,

        /// Target commit for Diff mode (defaults to HEAD if not provided)
        #[arg(long)]
        target: Option<String>,
    },
    /// Generate a default configuration file
    InitConfig {
        /// Output path (default: codeprism.yaml)
        #[arg(default_value = "codeprism.yaml")]
        output: String,
    },
    /// Validate configuration file
    CheckConfig,
    /// Resume an interrupted permanent project deletion while the server is stopped
    ResumeProjectDeletion {
        operation_id: String,
        #[arg(long)]
        confirmation_name: String,
    },
    /// Run tests for custom analyzers
    TestAnalyzers,
    /// Start the API server
    Serve {
        /// Host address to listen on. Non-loopback addresses require CODEPRISM_API_TOKEN.
        #[arg(long, default_value = "127.0.0.1")]
        host: String,
        /// Port to listen on
        #[arg(long, default_value_t = 3000)]
        port: u16,
    },
}

/// Resolve the database URL, making relative paths relative to the config file directory.
/// This ensures `scan` and `serve` use the same database when sharing a config file.
fn resolve_db_url(config: &codeprism_core::CodePrismConfig, config_path: &str) -> String {
    let url = config
        .database_url
        .clone()
        .unwrap_or_else(|| "sqlite:codeprism.db".to_string());

    // If it's a sqlite URL with a relative path, resolve against the config file's directory
    if let Some(path) = url.strip_prefix("sqlite:") {
        let db_path = std::path::Path::new(path);
        if db_path.is_relative()
            && let Some(config_dir) = std::path::Path::new(config_path).parent()
        {
            let abs_path = config_dir.join(path);
            return format!("sqlite:{}", abs_path.display());
        }
    }
    url
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    // DB connection

    match &cli.command {
        Commands::Init => {
            println!("Initializing Code Prism...");
            // Try to load config if exists, otherwise use default
            let config_path = cli.config.as_deref().unwrap_or("codeprism.yaml");
            let db_url = if std::path::Path::new(config_path).exists() {
                match codeprism_core::CodePrismConfig::load_from_file(config_path) {
                    Ok(c) => resolve_db_url(&c, config_path),
                    Err(_) => "sqlite:codeprism.db".to_string(),
                }
            } else {
                "sqlite:codeprism.db".to_string()
            };

            let db = Db::new(&db_url).await?;
            db.migrate().await?;
            println!("Database initialized at codeprism.db");

            // Also generate config file if it doesn't exist
            if !std::path::Path::new(config_path).exists() {
                let content = codeprism_core::CodePrismConfig::generate_template();
                std::fs::write(config_path, content)?;
                println!("Configuration template created at {}", config_path);
            }
        }
        Commands::InitConfig { output } => {
            let content = codeprism_core::CodePrismConfig::generate_template();
            if std::path::Path::new(output).exists() {
                println!("File '{}' already exists. Aborting.", output);
                std::process::exit(1);
            }
            std::fs::write(output, content)?;
            println!("Configuration template created at {}", output);
        }
        Commands::CheckConfig => {
            let config_path = cli.config.as_deref().unwrap_or("codeprism.yaml");
            match codeprism_core::CodePrismConfig::load_from_file(config_path) {
                Ok(config) => match config.validate() {
                    Ok(_) => println!("Configuration '{}' is valid.", config_path),
                    Err(e) => {
                        eprintln!("Configuration Error: {}", e);
                        std::process::exit(1);
                    }
                },
                Err(e) => {
                    eprintln!("Failed to load config: {}", e);
                    std::process::exit(1);
                }
            }
        }
        Commands::ResumeProjectDeletion {
            operation_id,
            confirmation_name,
        } => {
            let config_path = cli.config.as_deref().ok_or_else(|| {
                anyhow::anyhow!("--config is required for resume-project-deletion")
            })?;
            resume_project_deletion(config_path, operation_id, confirmation_name).await?;
        }
        Commands::Scan {
            path,
            project,
            mode,
            commit,
            base,
            target,
        } => {
            let config_path = cli.config.as_deref().unwrap_or("codeprism.yaml");
            let _process_lock = if std::path::Path::new(config_path).exists() {
                Some(codeprism_server::process_lock::ProcessLock::acquire(
                    config_path,
                )?)
            } else {
                None
            };
            let config = if std::path::Path::new(config_path).exists() {
                codeprism_core::CodePrismConfig::load_from_file(config_path)?
            } else {
                // If user didn't specify --config and default doesn't exist, use default values
                if cli.config.is_none() {
                    println!("No config file found at default location, using internal defaults.");
                    codeprism_core::CodePrismConfig::default()
                } else {
                    eprintln!("Config file '{}' not found.", config_path);
                    std::process::exit(1);
                }
            };

            let db_url = resolve_db_url(&config, config_path);
            let db = Db::new(&db_url).await?;

            let p_path = std::path::Path::new(path).canonicalize()?;
            let default_name = p_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown_project")
                .to_string();

            let project_name = project.as_ref().unwrap_or(&default_name);
            if config
                .get_project(project_name)
                .is_some_and(|project| project.archived)
            {
                anyhow::bail!("Project '{project_name}' is archived and cannot be scanned");
            }
            let config_dir = std::path::Path::new(config_path)
                .parent()
                .filter(|path| !path.as_os_str().is_empty());
            let mut scanner = Scanner::with_config_dir(db, config, config_dir);
            let abs_path_str = p_path.to_string_lossy().to_string();

            match mode {
                ScanMode::Snapshot => {
                    scanner
                        .scan_snapshot(&abs_path_str, project_name, commit.as_deref())
                        .await?;
                }
                ScanMode::Diff => {
                    if let Some(base_ref) = base {
                        let target_ref = target.as_deref().unwrap_or("HEAD");
                        scanner
                            .scan_diff(&abs_path_str, project_name, base_ref, target_ref)
                            .await?;
                    } else {
                        eprintln!("Error: --base <REF> is required for diff mode.");
                        std::process::exit(1);
                    }
                }
            }
        }
        Commands::TestAnalyzers => {
            println!("Testing custom Python analyzers...");
            let python = detect_python_interpreter().ok_or_else(|| {
                anyhow::anyhow!(
                    "Python 3 interpreter not found; expected python3, python, or py in PATH"
                )
            })?;
            let config_path = cli.config.as_deref().unwrap_or("codeprism.yaml");
            let config_dir = std::path::Path::new(config_path)
                .parent()
                .filter(|path| !path.as_os_str().is_empty());
            let dir = codeprism_scanner::analyzer_dir_with_config(config_dir)?;
            anyhow::ensure!(
                dir.is_dir(),
                "Analyzer directory not found: {}",
                dir.display()
            );

            let mut failures = 0usize;
            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.filter_map(Result::ok) {
                    let path = entry.path();
                    if path.is_file() && path.extension().is_some_and(|e| e == "py") {
                        let name = path
                            .file_stem()
                            .map(|s| s.to_string_lossy())
                            .unwrap_or_default();
                        if name.starts_with('_') {
                            continue;
                        }
                        println!("\nExample: {}", name);
                        println!("---------------------------------------------------");

                        let status = std::process::Command::new(python)
                            .arg(&path)
                            .arg("test")
                            .status();

                        match status {
                            Ok(s) => {
                                if s.success() {
                                    println!("{} [PASS]", name);
                                } else {
                                    println!("{} [FAIL] (Exit code: {:?})", name, s.code());
                                    failures += 1;
                                }
                            }
                            Err(e) => {
                                println!("Failed to execute test: {}", e);
                                failures += 1;
                            }
                        }
                    }
                }
            }
            if failures > 0 {
                anyhow::bail!("{} custom analyzer test(s) failed", failures);
            }
        }
        Commands::Serve { host, port } => {
            // Load Config & DB
            let config_path = cli.config.as_deref().unwrap_or("codeprism.yaml");
            let config = if std::path::Path::new(config_path).exists() {
                codeprism_core::CodePrismConfig::load_from_file(config_path)?
            } else if cli.config.is_none() {
                println!("No config file found, using defaults.");
                codeprism_core::CodePrismConfig::default()
            } else {
                eprintln!("Config '{}' not found.", config_path);
                std::process::exit(1);
            };
            let db_url = resolve_db_url(&config, config_path);
            let db = Db::new(&db_url).await?;
            db.migrate().await?;

            println!("Starting server...");
            codeprism_server::run_server(db, config, config_path.to_string(), host.clone(), *port)
                .await?;
        }
    }

    Ok(())
}

async fn resume_project_deletion(
    config_path: &str,
    operation_id: &str,
    confirmation_name: &str,
) -> Result<()> {
    let _lock = codeprism_server::process_lock::ProcessLock::acquire(config_path)?;
    let canonical_config = std::fs::canonicalize(config_path)?;
    let config = codeprism_core::CodePrismConfig::load_from_file(config_path)?;
    let expected_db_url = config
        .database_url
        .clone()
        .unwrap_or_else(|| "sqlite:codeprism.db".to_string());
    let db_url = resolve_db_url(&config, config_path);
    let db = Db::new(&db_url).await?;
    db.migrate().await?;
    let operation = sqlx::query_as::<_, (String, String, String, String, String, String, String, String, String)>(
        "SELECT project_name, config_path, database_path, cache_path, directories_json, registrations_json, config_before, config_after, stage FROM project_deletion_operations WHERE operation_id = ?",
    ).bind(operation_id).fetch_optional(db.pool()).await?
        .ok_or_else(|| anyhow::anyhow!("deletion operation not found: {operation_id}"))?;
    let (
        project_name,
        operation_config,
        operation_db,
        cache_path,
        directories_json,
        registrations_json,
        config_before,
        config_after,
        stage,
    ) = operation;
    anyhow::ensure!(
        project_name == confirmation_name,
        "confirmation name does not match the recorded project"
    );
    anyhow::ensure!(
        std::fs::canonicalize(operation_config)? == canonical_config,
        "operation belongs to another configuration file"
    );
    anyhow::ensure!(
        operation_db == expected_db_url,
        "operation belongs to another database configuration"
    );
    anyhow::ensure!(
        stage != "completed",
        "deletion operation is already complete"
    );

    let current_config = std::fs::read_to_string(config_path)?;
    anyhow::ensure!(
        current_config == config_before || current_config == config_after,
        "configuration changed after the deletion operation; refusing to overwrite it"
    );

    #[derive(serde::Deserialize)]
    struct Directory {
        repo_id: String,
        path: String,
    }
    let directories: Vec<Directory> = serde_json::from_str(&directories_json)?;
    for directory in &directories {
        let path = std::path::Path::new(&directory.path);
        if !path.exists() {
            continue;
        }
        let parent = path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("invalid recorded repository path"))?;
        anyhow::ensure!(
            path.file_name().and_then(|name| name.to_str()) == Some(directory.repo_id.as_str()),
            "recorded repository path no longer matches its ID"
        );
        let cache = codeprism_server::git_cache::GitCache::new(parent.to_path_buf());
        let repo = cache.get(&directory.repo_id).ok_or_else(|| {
            anyhow::anyhow!(
                "repository {} cannot be revalidated; refusing to delete it",
                directory.repo_id
            )
        })?;
        anyhow::ensure!(
            repo.project_name.as_deref() == Some(project_name.as_str()),
            "repository {} is no longer registered to the recorded project",
            directory.repo_id
        );
        anyhow::ensure!(
            cache.can_remove_files(&directory.repo_id, &repo),
            "repository {} no longer passes managed path checks",
            directory.repo_id
        );
        anyhow::ensure!(
            !cache.list_all().iter().any(|(id, other)| {
                id != &directory.repo_id
                    && std::fs::canonicalize(&other.path).ok() == std::fs::canonicalize(path).ok()
            }),
            "repository {} is now shared by another registration",
            directory.repo_id
        );
        anyhow::ensure!(
            !config.get_projects().iter().any(|project| {
                project.name != project_name
                    && project
                        .repo_path
                        .as_deref()
                        .and_then(|other| std::fs::canonicalize(other).ok())
                        == std::fs::canonicalize(path).ok()
            }),
            "repository {} is now referenced by another project",
            directory.repo_id
        );
        let git = git2::Repository::open(path)?;
        let mut status_options = git2::StatusOptions::new();
        status_options
            .include_untracked(true)
            .recurse_untracked_dirs(true);
        anyhow::ensure!(
            git.statuses(Some(&mut status_options))?.is_empty(),
            "repository {} has uncommitted or untracked files",
            directory.repo_id
        );
        let head = git.head().ok();
        let tracking = head
            .as_ref()
            .and_then(|head| head.shorthand())
            .and_then(|name| git.find_branch(name, git2::BranchType::Local).ok())
            .and_then(|branch| branch.upstream().ok())
            .and_then(|branch| branch.get().target());
        let local = head.and_then(|head| head.target());
        let (Some(local), Some(remote)) = (local, tracking) else {
            anyhow::bail!(
                "repository {} has no verifiable upstream",
                directory.repo_id
            );
        };
        anyhow::ensure!(
            git.graph_ahead_behind(local, remote)?.0 == 0,
            "repository {} contains unpushed commits",
            directory.repo_id
        );
        std::fs::remove_dir_all(path)?;
        sqlx::query("UPDATE project_deletion_operations SET stage = 'files_removed', updated_at = CURRENT_TIMESTAMP WHERE operation_id = ?")
            .bind(operation_id).execute(db.pool()).await?;
    }

    if current_config == config_before {
        codeprism_server::config_persistence::write_atomic(
            std::path::Path::new(config_path),
            config_after.as_bytes(),
        )?;
    }
    let mut tx = db.pool().begin().await?;
    let project_ids: Vec<i64> = sqlx::query_scalar("SELECT id FROM projects WHERE name = ?")
        .bind(&project_name)
        .fetch_all(&mut *tx)
        .await?;
    for id in project_ids {
        for table in [
            "scan_summaries",
            "matches",
            "metrics",
            "file_changes",
            "intermediate_blocks",
        ] {
            let query = format!(
                "DELETE FROM {table} WHERE scan_id IN (SELECT id FROM scans WHERE project_id = ?)"
            );
            sqlx::query(&query).bind(id).execute(&mut *tx).await?;
        }
        sqlx::query("DELETE FROM scans WHERE project_id = ?")
            .bind(id)
            .execute(&mut *tx)
            .await?;
    }
    sqlx::query("DELETE FROM scan_jobs WHERE project_name = ?")
        .bind(&project_name)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM projects WHERE name = ?")
        .bind(&project_name)
        .execute(&mut *tx)
        .await?;
    sqlx::query("DELETE FROM match_contents WHERE NOT EXISTS (SELECT 1 FROM matches WHERE matches.content_id = match_contents.id) AND NOT EXISTS (SELECT 1 FROM metrics WHERE metrics.content_id = match_contents.id)")
        .execute(&mut *tx).await?;
    tx.commit().await?;
    let cache_path = std::path::PathBuf::from(cache_path);
    let cache = codeprism_server::git_cache::GitCache::new(
        cache_path
            .parent()
            .ok_or_else(|| anyhow::anyhow!("invalid cache path in operation record"))?
            .to_path_buf(),
    );
    let registration_ids: Vec<String> = serde_json::from_str(&registrations_json)?;
    for repo_id in registration_ids {
        cache.remove(&repo_id)?;
    }
    sqlx::query("UPDATE project_deletion_operations SET stage = 'completed', last_error = NULL, updated_at = CURRENT_TIMESTAMP WHERE operation_id = ?")
        .bind(operation_id).execute(db.pool()).await?;
    println!("Resumed deletion operation {operation_id} for '{project_name}'");
    Ok(())
}
