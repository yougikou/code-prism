use anyhow::Result;
use clap::{Parser, Subcommand, ValueEnum};
use codeprism_database::Db;
use codeprism_scanner::Scanner;

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
    /// Run tests for custom analyzers
    /// Run tests for custom analyzers
    TestAnalyzers,
    /// Start the API server
    Serve {
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
        if db_path.is_relative() {
            if let Some(config_dir) = std::path::Path::new(config_path).parent() {
                let abs_path = config_dir.join(path);
                return format!("sqlite:{}", abs_path.display());
            }
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
        Commands::Scan {
            path,
            project,
            mode,
            commit,
            base,
            target,
        } => {
            let config_path = cli.config.as_deref().unwrap_or("codeprism.yaml");
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

            let mut scanner = Scanner::with_config(db, config);

            let p_path = std::path::Path::new(path).canonicalize()?;
            let default_name = p_path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("unknown_project")
                .to_string();

            let project_name = project.as_ref().unwrap_or(&default_name);
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
            println!("Testing custom Python analyzers in 'custom_analyzers/'...");
            let dir = std::path::Path::new("custom_analyzers");
            if !dir.exists() {
                println!("Directory 'custom_analyzers' not found.");
                return Ok(());
            }

            if let Ok(entries) = std::fs::read_dir(dir) {
                for entry in entries.filter_map(Result::ok) {
                    let path = entry.path();
                    if path.is_file() && path.extension().map_or(false, |e| e == "py") {
                        let name = path
                            .file_stem()
                            .map(|s| s.to_string_lossy())
                            .unwrap_or_default();
                        println!("\nExample: {}", name);
                        println!("---------------------------------------------------");

                        let status = std::process::Command::new("python")
                            .arg(&path)
                            .arg("test")
                            .status();

                        match status {
                            Ok(s) => {
                                if s.success() {
                                    println!("{} [PASS]", name);
                                } else {
                                    println!("{} [FAIL] (Exit code: {:?})", name, s.code());
                                }
                            }
                            Err(e) => println!("Failed to execute test: {}", e),
                        }
                    }
                }
            }
        }
        Commands::Serve { port } => {
            // Load Config & DB
            let config_path = cli.config.as_deref().unwrap_or("codeprism.yaml");
            let config = if std::path::Path::new(config_path).exists() {
                codeprism_core::CodePrismConfig::load_from_file(config_path)?
            } else {
                if cli.config.is_none() {
                    println!("No config file found, using defaults.");
                    codeprism_core::CodePrismConfig::default()
                } else {
                    eprintln!("Config '{}' not found.", config_path);
                    std::process::exit(1);
                }
            };
            let db_url = resolve_db_url(&config, config_path);
            let db = Db::new(&db_url).await?;
            db.migrate().await?;

            println!("Starting server...");
            codeprism_server::run_server(db, config, config_path.to_string(), *port).await?;
        }
    }

    Ok(())
}
