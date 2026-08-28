use crate::Analyzer;
use codeprism_core::{AnalyzerRuntimeOutcome, MatchDetail, MetricEntry, TAG_CATEGORY, TAG_METRIC};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::{Arc, Mutex, mpsc};
use std::time::Duration;

const SCRIPT_IO_TIMEOUT: Duration = Duration::from_secs(30);
const MAX_SCRIPT_INPUT_BYTES: usize = 8 * 1024 * 1024;

#[derive(Serialize, Deserialize)]
struct ScriptInput {
    file_path: String,
    content: String,
    change_type: String,
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
    child: Arc<Mutex<Child>>,
    stdin: ChildStdin,
    stdout_reader: BufReader<ChildStdout>,
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        if let Ok(mut child) = self.child.lock() {
            let _ = child.kill();
            let _ = child.wait();
        }
    }
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
    current_change_type: Arc<Mutex<String>>,
    runtime_outcomes: Arc<Mutex<Vec<AnalyzerRuntimeOutcome>>>,
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
            current_change_type: Arc::new(Mutex::new(String::new())),
            runtime_outcomes: Arc::new(Mutex::new(Vec::new())),
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
                && output.success()
            {
                return Ok(cmd.to_string());
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
                child: Arc::new(Mutex::new(child)),
                stdin,
                stdout_reader: BufReader::new(stdout),
            });
        }
        Ok(())
    }

    fn record_runtime_outcome(
        &self,
        kind: &str,
        severity: &str,
        message: impl Into<String>,
        limit: Option<String>,
        observed: Option<String>,
    ) {
        self.runtime_outcomes
            .lock()
            .unwrap()
            .push(AnalyzerRuntimeOutcome {
                kind: kind.to_string(),
                severity: severity.to_string(),
                message: message.into(),
                limit,
                observed,
            });
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

    fn set_file_context(&self, change_type: &str, _scan_mode: &str) {
        *self.current_change_type.lock().unwrap() = change_type.to_string();
    }

    fn analyze(&self, file_path: &str, content: &str) -> Vec<MetricEntry> {
        if let Err(e) = self.ensure_process() {
            eprintln!("{}", e);
            self.record_runtime_outcome("startup_error", "high", e, None, None);
            return vec![];
        }

        // Reset matches cache for this file
        self.matches_cache.lock().unwrap().clear();

        // Move the process interaction to a worker so a misbehaving script
        // cannot block the scan indefinitely. On timeout the parent is killed
        // and the next file gets a fresh process.
        let mut guard = self.process.lock().unwrap();
        if let Some(mut handle) = guard.take() {
            // Prepare Input
            let change_type = self.current_change_type.lock().unwrap().clone();
            let input = ScriptInput {
                file_path: file_path.to_string(),
                content: content.to_string(),
                change_type,
            };

            // Serialize to single line JSON (no newlines usually in json compact)
            // But content might contain newlines which are escaped as \n.
            let mut json_input = match serde_json::to_string(&input) {
                Ok(s) => s,
                Err(error) => {
                    self.record_runtime_outcome(
                        "protocol_error",
                        "warning",
                        error.to_string(),
                        None,
                        None,
                    );
                    return vec![];
                }
            };
            json_input.push('\n');
            if json_input.len() > MAX_SCRIPT_INPUT_BYTES {
                eprintln!(
                    "Analyzer script input exceeds {} bytes",
                    MAX_SCRIPT_INPUT_BYTES
                );
                self.record_runtime_outcome(
                    "input_limit",
                    "high",
                    "Analyzer input exceeds the execution limit",
                    Some(format!("{} bytes", MAX_SCRIPT_INPUT_BYTES)),
                    Some(format!("{} bytes", json_input.len())),
                );
                return vec![];
            }

            let child = handle.child.clone();
            let (tx, rx) = mpsc::sync_channel(1);
            std::thread::spawn(move || {
                let result = (|| -> Result<(ProcessHandle, String), String> {
                    handle
                        .stdin
                        .write_all(json_input.as_bytes())
                        .map_err(|e| e.to_string())?;
                    handle.stdin.flush().map_err(|e| e.to_string())?;
                    let mut line = String::new();
                    match handle.stdout_reader.read_line(&mut line) {
                        Ok(0) => {
                            Err("Analyzer script process ended unexpectedly (EOF).".to_string())
                        }
                        Ok(_) => Ok((handle, line)),
                        Err(e) => Err(format!("Failed to read from analyzer: {}", e)),
                    }
                })();
                let _ = tx.send(result);
            });

            match rx.recv_timeout(SCRIPT_IO_TIMEOUT) {
                Ok(Ok((returned_handle, line))) => {
                    *guard = Some(returned_handle);
                    // Parse Output — try standard metric format first, then
                    // duplication block format (auto-detect at call time).
                    let raw_outputs: Vec<ScriptOutput> = match serde_json::from_str(&line) {
                        Ok(o) => o,
                        Err(first_err) => {
                            // Check if this is a duplication script (block output format).
                            // If so, return empty metrics silently — duplication analyzers
                            // are handled separately via the FileProcessor (cross-file) trait.
                            if serde_json::from_str::<Vec<codeprism_core::ScriptContentBlock>>(
                                &line,
                            )
                            .is_ok()
                            {
                                return vec![];
                            }
                            eprintln!("Failed to parse analyzer output: {}", first_err);
                            self.record_runtime_outcome(
                                "protocol_error",
                                "warning",
                                first_err.to_string(),
                                None,
                                None,
                            );
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
                Ok(Err(e)) => {
                    eprintln!("Analyzer script failed: {}", e);
                    self.record_runtime_outcome("execution_error", "warning", e, None, None);
                    return vec![];
                }
                Err(mpsc::RecvTimeoutError::Timeout) => {
                    eprintln!(
                        "Analyzer script timed out after {} seconds",
                        SCRIPT_IO_TIMEOUT.as_secs()
                    );
                    if let Ok(mut process) = child.lock() {
                        let _ = process.kill();
                    }
                    self.record_runtime_outcome(
                        "timeout",
                        "warning",
                        "Analyzer script exceeded its per-file execution limit",
                        Some(format!("{} seconds", SCRIPT_IO_TIMEOUT.as_secs())),
                        Some(format!("{} seconds", SCRIPT_IO_TIMEOUT.as_secs())),
                    );
                    return vec![];
                }
                Err(mpsc::RecvTimeoutError::Disconnected) => {
                    eprintln!("Analyzer script worker ended unexpectedly");
                    self.record_runtime_outcome(
                        "execution_error",
                        "warning",
                        "Analyzer script worker ended unexpectedly",
                        None,
                        None,
                    );
                    return vec![];
                }
            }
        }

        vec![]
    }

    fn extract_matches(&self, _path: &str, _content: &str) -> Vec<MatchDetail> {
        self.matches_cache.lock().unwrap().clone()
    }

    fn take_runtime_outcomes(&self) -> Vec<AnalyzerRuntimeOutcome> {
        std::mem::take(&mut *self.runtime_outcomes.lock().unwrap())
    }
}
