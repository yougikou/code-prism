use crate::Analyzer;
use codeprism_core::{MatchDetail, MetricEntry, TAG_CATEGORY, TAG_METRIC};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
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
    value: f64,
    /// New tag system — map of key-value tags
    #[serde(default)]
    tags: HashMap<String, String>,
    /// Old metric_key field (deprecated, merged into tags)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    metric_key: Option<String>,
    /// Old category field (deprecated, merged into tags)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    category: Option<String>,
    /// Optional per-match details
    #[serde(default)]
    matches: Option<Vec<MatchDetail>>,
}

impl ScriptOutput {
    fn resolve_tags(&self) -> HashMap<String, String> {
        let mut result = self.tags.clone();
        if let Some(mk) = &self.metric_key {
            result.insert(TAG_METRIC.to_string(), mk.clone());
        }
        if let Some(cat) = &self.category {
            result.insert(TAG_CATEGORY.to_string(), cat.clone());
        }
        result
    }
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
    tag_overrides: HashMap<String, String>,
    scan_mode: Option<String>,
    change_type: Option<String>,
    matches_cache: Arc<Mutex<Vec<MatchDetail>>>,
}

impl ScriptAnalyzer {
    pub fn new(
        id: &str,
        script_path: &str,
        tag_overrides: HashMap<String, String>,
        scan_mode: Option<String>,
        change_type: Option<String>,
    ) -> Self {
        Self {
            id: id.to_string(),
            script_path: script_path.to_string(),
            interpreter: Arc::new(Mutex::new(None)), // Will be detected on first use
            process: Arc::new(Mutex::new(None)),
            tag_overrides,
            scan_mode,
            change_type,
            matches_cache: Arc::new(Mutex::new(Vec::new())),
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

    fn scan_mode(&self) -> Option<&str> {
        self.scan_mode.as_deref()
    }

    fn change_type(&self) -> Option<&str> {
        self.change_type.as_deref()
    }

    fn analyze(&self, file_path: &str, content: &str) -> Vec<MetricEntry> {
        if let Err(e) = self.ensure_process() {
            eprintln!("{}", e);
            return vec![];
        }

        // Reset matches cache for this file
        self.matches_cache.lock().unwrap().clear();

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

                    // Collect all matches from all output entries into cache
                    let mut all_matches = Vec::new();
                    for o in &raw_outputs {
                        if let Some(ref matches) = o.matches {
                            for m in matches {
                                let mut detail = m.clone();
                                detail.analyzer_id = self.id.clone();
                                detail.file_path = file_path.to_string();
                                all_matches.push(detail);
                            }
                        }
                    }
                    *self.matches_cache.lock().unwrap() = all_matches;

                    return raw_outputs
                        .into_iter()
                        .map(|o| {
                            let mut tags = o.resolve_tags();
                            for (k, v) in &self.tag_overrides {
                                tags.insert(k.clone(), v.clone());
                            }
                            MetricEntry {
                                analyzer_id: self.id.clone(),
                                tags,
                                value: o.value,
                                scope: None,
                                tech_stack: None,
                            }
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

    fn extract_matches(&self, _path: &str, _content: &str) -> Vec<MatchDetail> {
        self.matches_cache.lock().unwrap().clone()
    }
}
