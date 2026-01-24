use crate::Analyzer;
use codeprism_core::MetricEntry;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex};

#[derive(Serialize, Deserialize)]
struct ScriptInput {
    file_path: String,
    content: String,
}

#[derive(Serialize, Deserialize)]
struct ScriptOutput {
    metric_key: String,
    value: f64,
    category: Option<String>,
}

struct ProcessHandle {
    _child: Child,
    stdin: ChildStdin,
    stdout_reader: BufReader<ChildStdout>,
}

pub struct ScriptAnalyzer {
    id: String,
    script_path: String,
    interpreter: Arc<Mutex<Option<String>>>, // Lazy-detected, wrapped for interior mutability
    process: Arc<Mutex<Option<ProcessHandle>>>,
    metric_key_override: Option<String>,
    category_override: Option<String>,
}

impl ScriptAnalyzer {
    pub fn new(
        id: &str,
        script_path: &str,
        metric_key_override: Option<String>,
        category_override: Option<String>,
    ) -> Self {
        Self {
            id: id.to_string(),
            script_path: script_path.to_string(),
            interpreter: Arc::new(Mutex::new(None)), // Will be detected on first use
            process: Arc::new(Mutex::new(None)),
            metric_key_override,
            category_override,
        }
    }

    /// Detect available Python interpreter - tries python3 first (macOS/Linux), then python (Windows)
    fn detect_python_interpreter() -> Result<String, String> {
        // Try python3 first (preferred on macOS/Linux), python on Windows
        let candidates = if cfg!(windows) {
            vec!["python", "python3", "py"]
        } else {
            vec!["python3", "python"]
        };

        for cmd in candidates {
            if let Ok(output) = Command::new(cmd)
                .arg("--version")
                .stdout(Stdio::null())
                .stderr(Stdio::null())
                .status()
            {
                if output.success() {
                    return Ok(cmd.to_string());
                }
            }
        }

        Err("Python interpreter not found. Please install Python 3 and ensure 'python3' or 'python' is in your PATH.".to_string())
    }

    fn ensure_process(&self) -> Result<(), String> {
        let mut guard = self.process.lock().unwrap();
        if guard.is_none() {
            // Detect interpreter if not already done (uses separate lock)
            let interpreter = {
                let mut interp_guard = self.interpreter.lock().unwrap();
                if interp_guard.is_none() {
                    *interp_guard = Some(Self::detect_python_interpreter()?);
                }
                interp_guard.clone().unwrap()
            };

            let mut child = Command::new(&interpreter)
                .arg(&self.script_path)
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .map_err(|e| {
                    format!(
                        "Failed to spawn analyzer '{}' with interpreter '{}': {}",
                        self.id, interpreter, e
                    )
                })?;

            let stdin = child.stdin.take().ok_or("Failed to capture stdin")?;
            let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;

            guard.replace(ProcessHandle {
                _child: child,
                stdin,
                stdout_reader: BufReader::new(stdout),
            });
        }
        Ok(())
    }
}

impl Analyzer for ScriptAnalyzer {
    fn id(&self) -> &str {
        &self.id
    }

    fn analyze(&self, file_path: &str, content: &str) -> Vec<MetricEntry> {
        if let Err(e) = self.ensure_process() {
            eprintln!("{}", e);
            return vec![];
        }

        // Lock the process for the duration of this analysis interaction
        let mut guard = self.process.lock().unwrap();
        if let Some(handle) = guard.as_mut() {
            // Prepare Input
            let input = ScriptInput {
                file_path: file_path.to_string(),
                content: content.to_string(),
            };

            // Serialize to single line JSON (no newlines usually in json compact)
            // But content might contain newlines which are escaped as \n.
            let mut json_input = match serde_json::to_string(&input) {
                Ok(s) => s,
                Err(_) => return vec![],
            };
            json_input.push('\n');

            // Write Input
            if let Err(e) = handle.stdin.write_all(json_input.as_bytes()) {
                eprintln!("Failed to write to analyzer script: {}", e);
                // If broken pipe, maybe restart? For now just fail.
                return vec![];
            }
            if let Err(e) = handle.stdin.flush() {
                eprintln!("Failed to flush to analyzer: {}", e);
                return vec![];
            }

            // Read Output
            let mut line = String::new();
            match handle.stdout_reader.read_line(&mut line) {
                Ok(0) => {
                    // EOF
                    eprintln!("Analyzer script process ended unexpectedly (EOF).");
                    return vec![];
                }
                Ok(_) => {
                    // Parse Output
                    let raw_outputs: Vec<ScriptOutput> = match serde_json::from_str(&line) {
                        Ok(o) => o,
                        Err(e) => {
                            eprintln!("Failed to parse analyzer output: {}", e);
                            return vec![];
                        }
                    };

                    return raw_outputs
                        .into_iter()
                        .map(|o| MetricEntry {
                            analyzer_id: self.id.clone(),
                            metric_key: self.metric_key_override.clone().unwrap_or(o.metric_key),
                            category: self.category_override.clone().or(o.category),
                            value: o.value,
                            scope: None,
                            tech_stack: None,
                        })
                        .collect();
                }
                Err(e) => {
                    eprintln!("Failed to read from analyzer: {}", e);
                    return vec![];
                }
            }
        }

        vec![]
    }
}
