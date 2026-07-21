use crate::{Analyzer, FileProcessor};
use async_trait::async_trait;
use codeprism_core::{
    ContentBlock, FinalizeFinding, FinalizeMatchResult, FinalizeMetric, FinalizeOccurrence,
    FinalizeOutput, IntermediateBlock, ScriptContentBlock,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};
use std::sync::mpsc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const IO_TIMEOUT: Duration = Duration::from_secs(30);

/// Message sent to the Python script's stdin.
///
/// The `action` field dispatches between the two protocol phases:
/// - `"extract"`  — per-file block extraction (fields: file_path, content)
/// - `"finalize"` — global aggregation (field: blocks)
#[derive(Serialize)]
struct ScriptMessage {
    action: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    file_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    blocks: Option<Vec<BlockData>>,
}

/// Data sent to the script's `finalize` phase for one intermediate block.
///
/// Field semantics (see `IntermediateBlock` doc table for details):
/// | field        | duplicate detection meaning            |
/// |-------------|---------------------------------------|
/// | group_key   | block_hash (SHA-256 of content)       |
/// | blob_data   | block_content (for display)           |
/// | int_data1   | block_size (lines / sentinel)         |
/// | int_data2   | line_start                            |
/// | int_data3   | line_end                              |
/// | str_data1   | change_type ("A","M","D")             |
/// | str_data2   | side ("0"=old, "1"=new in diff mode)  |
#[derive(Serialize)]
struct BlockData {
    file_path: String,
    group_key: String,
    blob_data: Option<String>,
    int_data1: Option<i64>,
    int_data2: Option<i64>,
    int_data3: Option<i64>,
    str_data1: Option<String>,
    str_data2: Option<String>,
}

#[derive(Deserialize)]
#[serde(untagged)]
enum ScriptFinalizeResponse {
    Generic(FinalizeOutput),
    Legacy(Vec<FinalizeMatchResult>),
}

struct ProcessHandle {
    child: Child,
    stdin: ChildStdin,
    stdout_reader: BufReader<ChildStdout>,
}

impl Drop for ProcessHandle {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

pub struct ScriptCrossFileAnalyzer {
    id: String,
    /// YAML-configured tag overrides (from `custom_cross_file_analyzers`).
    /// Applied on top of whatever the Python script defines as defaults.
    tag_overrides: HashMap<String, String>,
    interpreter: Arc<Mutex<Option<String>>>,
    process: Arc<Mutex<Option<ProcessHandle>>>,
}

impl ScriptCrossFileAnalyzer {
    /// Create a new cross-file analyzer.
    ///
    /// The Python script path is inferred from the analyzer `id` as
    /// `custom_analyzers/{id}.py`.
    ///
    /// Threshold logic (min_file_count, min_block_count, etc.) is owned
    /// by the Python script itself — the framework only passes through
    /// the blocks and writes whatever the script reports as matches.
    ///
    /// `tag_overrides` comes from the YAML config and overrides
    /// whatever default tags the Python script itself defines.
    pub fn new(id: &str, tag_overrides: HashMap<String, String>) -> Self {
        Self {
            id: id.to_string(),
            tag_overrides,
            interpreter: Arc::new(Mutex::new(None)),
            process: Arc::new(Mutex::new(None)),
        }
    }

    fn script_path(&self) -> String {
        format!("custom_analyzers/{}.py", self.id)
    }

    fn detect_python_interpreter() -> Result<String, String> {
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

        Err("Python interpreter not found".to_string())
    }

    fn ensure_process(&self) -> Result<(), String> {
        let mut guard = self.process.lock().unwrap();
        if guard.is_none() {
            let interpreter = {
                let mut interp_guard = self.interpreter.lock().unwrap();
                if interp_guard.is_none() {
                    *interp_guard = Some(Self::detect_python_interpreter()?);
                }
                interp_guard.clone().unwrap()
            };

            let mut child = Command::new(&interpreter)
                .arg(self.script_path())
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(Stdio::inherit())
                .spawn()
                .map_err(|e| {
                    format!(
                        "Failed to spawn cross-file analyzer '{}' with interpreter '{}': {}",
                        self.id, interpreter, e
                    )
                })?;

            let stdin = child.stdin.take().ok_or("Failed to capture stdin")?;
            let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;

            guard.replace(ProcessHandle {
                child,
                stdin,
                stdout_reader: BufReader::new(stdout),
            });
        }
        Ok(())
    }

    fn reset_process(&self) {
        if let Ok(mut guard) = self.process.lock()
            && guard.is_some()
        {
            *guard = None;
        }
    }

    /// Internal: run the Python script's `extract` action and return ContentBlocks.
    ///
    fn extract_content_blocks(&self, file_path: &str, content: &str) -> Vec<ContentBlock> {
        if let Err(e) = self.ensure_process() {
            eprintln!("{}", e);
            return vec![];
        }

        let mut guard = self.process.lock().unwrap();
        let mut handle = match guard.take() {
            Some(h) => h,
            None => return vec![],
        };

        let msg = ScriptMessage {
            action: "extract".to_string(),
            file_path: Some(file_path.to_string()),
            content: Some(content.to_string()),
            blocks: None,
        };
        let mut json_input = match serde_json::to_string(&msg) {
            Ok(s) => s,
            Err(_) => {
                guard.replace(handle);
                return vec![];
            }
        };
        json_input.push('\n');

        let (tx, rx) = mpsc::channel();
        let aid = self.id.clone();
        std::thread::spawn(move || {
            let result = (|| -> Option<(ProcessHandle, Vec<ContentBlock>)> {
                let json_bytes = json_input.as_bytes();
                handle.stdin.write_all(json_bytes).ok()?;
                handle.stdin.flush().ok()?;

                let mut line = String::new();
                let n = handle.stdout_reader.read_line(&mut line).ok()?;
                if n == 0 {
                    return None;
                }

                let script_blocks: Vec<ScriptContentBlock> = serde_json::from_str(&line).ok()?;

                let blocks = script_blocks.into_iter().map(ContentBlock::from).collect();

                Some((handle, blocks))
            })();

            let _ = tx.send(result);
        });

        match rx.recv_timeout(IO_TIMEOUT) {
            Ok(Some((h, blocks))) => {
                guard.replace(h);
                return blocks;
            }
            Ok(None) => {
                eprintln!(
                    "Cross-file analyzer '{}' ended unexpectedly. Will restart.",
                    aid
                );
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                eprintln!(
                    "Cross-file analyzer '{}' timed out after {}s. Killing process.",
                    aid,
                    IO_TIMEOUT.as_secs()
                );
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                eprintln!("Cross-file analyzer '{}' thread crashed.", aid);
            }
        }

        vec![]
    }

    /// Run the Python script's `finalize` action and return the raw response line.
    ///
    /// All intermediate blocks are sent to the script, which groups them by
    /// hash, applies its own thresholds, and returns filtered match groups.
    fn send_finalize(&self, blocks: Vec<BlockData>) -> Result<String, String> {
        if let Err(e) = self.ensure_process() {
            eprintln!("{}", e);
            return Err(e);
        }

        let mut guard = self.process.lock().unwrap();
        let mut handle = match guard.take() {
            Some(h) => h,
            None => return Err("Process handle not available".to_string()),
        };

        let msg = ScriptMessage {
            action: "finalize".to_string(),
            file_path: None,
            content: None,
            blocks: Some(blocks),
        };
        let mut json_input = match serde_json::to_string(&msg) {
            Ok(s) => s,
            Err(e) => {
                guard.replace(handle);
                return Err(format!("Serialization error: {}", e));
            }
        };
        json_input.push('\n');

        let (tx, rx) = mpsc::channel();
        let aid = self.id.clone();

        std::thread::spawn(move || {
            let result = (|| -> Option<(ProcessHandle, String)> {
                handle.stdin.write_all(json_input.as_bytes()).ok()?;
                handle.stdin.flush().ok()?;

                let mut line = String::new();
                let n = handle.stdout_reader.read_line(&mut line).ok()?;
                if n == 0 {
                    return None;
                }
                Some((handle, line))
            })();

            let _ = tx.send(result);
        });

        match rx.recv_timeout(IO_TIMEOUT) {
            Ok(Some((h, line))) => {
                guard.replace(h);
                Ok(line)
            }
            Ok(None) => {
                eprintln!(
                    "Cross-file analyzer '{}' ended unexpectedly during finalize.",
                    aid
                );
                Err("Script ended unexpectedly".to_string())
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {
                eprintln!(
                    "Cross-file analyzer '{}' timed out during finalize ({}s).",
                    aid,
                    IO_TIMEOUT.as_secs()
                );
                Err(format!("Script timed out after {}s", IO_TIMEOUT.as_secs()))
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                eprintln!(
                    "Cross-file analyzer '{}' thread crashed during finalize.",
                    aid
                );
                Err("Script thread crashed".to_string())
            }
        }
    }
}

impl Drop for ScriptCrossFileAnalyzer {
    fn drop(&mut self) {
        self.reset_process();
    }
}

// ── Analyzer impl (returns empty — cross-file analysis produces no per-file metrics) ──

impl Analyzer for ScriptCrossFileAnalyzer {
    fn id(&self) -> &str {
        &self.id
    }

    fn analyze(&self, _path: &str, _content: &str) -> Vec<codeprism_core::MetricEntry> {
        vec![] // Cross-file analysis produces results only in finalize()
    }

    fn as_file_processor(&self) -> Option<&dyn FileProcessor> {
        Some(self)
    }
}

// ── FileProcessor impl ──

#[async_trait]
impl FileProcessor for ScriptCrossFileAnalyzer {
    fn extract_blocks(&self, file_path: &str, content: &str) -> Vec<IntermediateBlock> {
        let content_blocks = self.extract_content_blocks(file_path, content);
        content_blocks
            .into_iter()
            .map(|cb| IntermediateBlock {
                analyzer_id: String::new(), // filled by pipeline
                file_path: String::new(),   // filled by pipeline
                group_key: cb.block_hash,
                blob_data: Some(cb.block_content),
                int_data1: Some(cb.block_size as i64),
                int_data2: Some(cb.line_start as i64),
                int_data3: Some(cb.line_end as i64),
                str_data1: None, // filled by pipeline (change_type)
                str_data2: None, // filled by pipeline (side)
            })
            .collect()
    }

    async fn finalize(&self, blocks: Vec<IntermediateBlock>) -> anyhow::Result<FinalizeOutput> {
        if blocks.is_empty() {
            return Ok(FinalizeOutput::default());
        }
        let payload = blocks
            .into_iter()
            .map(|block| BlockData {
                file_path: block.file_path,
                group_key: block.group_key,
                blob_data: block.blob_data,
                int_data1: block.int_data1,
                int_data2: block.int_data2,
                int_data3: block.int_data3,
                str_data1: block.str_data1,
                str_data2: block.str_data2,
            })
            .collect();
        let response = self.send_finalize(payload).map_err(|error| {
            self.reset_process();
            anyhow::anyhow!("transient finalize failure for '{}': {}", self.id, error)
        })?;
        match serde_json::from_str::<ScriptFinalizeResponse>(&response).map_err(|error| {
            anyhow::anyhow!("Invalid finalize response from '{}': {}", self.id, error)
        })? {
            ScriptFinalizeResponse::Generic(mut output) => {
                for finding in &mut output.findings {
                    finding.tags.extend(self.tag_overrides.clone());
                }
                Ok(output)
            }
            ScriptFinalizeResponse::Legacy(results) => {
                eprintln!(
                    "Cross-file analyzer '{}' uses the deprecated duplication finalize protocol",
                    self.id
                );
                Ok(legacy_output(results, &self.tag_overrides))
            }
        }
    }

    fn reset(&self) {
        self.reset_process();
    }

    fn is_transient_error(&self, error: &anyhow::Error) -> bool {
        error.to_string().starts_with("transient finalize failure")
    }
}

fn legacy_output(
    results: Vec<FinalizeMatchResult>,
    tag_overrides: &HashMap<String, String>,
) -> FinalizeOutput {
    let findings = results
        .into_iter()
        .map(|result| {
            let mut file_groups: BTreeMap<String, Vec<&FinalizeOccurrence>> = BTreeMap::new();
            for occurrence in &result.occurrences {
                file_groups
                    .entry(occurrence.file_path.clone())
                    .or_default()
                    .push(occurrence);
            }
            let mut metrics = Vec::new();
            let finding_before = result
                .occurrences
                .iter()
                .any(|occurrence| occurrence.side.as_deref() == Some("0"));
            let finding_after = result
                .occurrences
                .iter()
                .any(|occurrence| occurrence.side.as_deref() != Some("0"));
            for (index, (file_path, occurrences)) in file_groups.into_iter().enumerate() {
                let before = occurrences
                    .iter()
                    .filter(|o| o.side.as_deref() == Some("0"))
                    .count() as f64;
                let after = occurrences
                    .iter()
                    .filter(|o| o.side.as_deref() != Some("0"))
                    .count() as f64;
                let before_lines = occurrences
                    .iter()
                    .filter(|o| o.side.as_deref() == Some("0"))
                    .map(|o| (o.line_end - o.line_start + 1).max(0) as f64)
                    .sum();
                let after_lines = occurrences
                    .iter()
                    .filter(|o| o.side.as_deref() != Some("0"))
                    .map(|o| (o.line_end - o.line_start + 1).max(0) as f64)
                    .sum();
                let mut values = BTreeMap::from([
                    ("occurrence_count", (before, after)),
                    (
                        "affected_file_count",
                        ((before > 0.0) as u8 as f64, (after > 0.0) as u8 as f64),
                    ),
                    ("affected_line_count", (before_lines, after_lines)),
                ]);
                if index == 0 {
                    values.insert(
                        "finding_count",
                        (finding_before as u8 as f64, finding_after as u8 as f64),
                    );
                }
                for (metric_key, (value_before, value_after)) in values {
                    metrics.push(FinalizeMetric {
                        metric_key: metric_key.into(),
                        file_path: file_path.clone(),
                        value_before,
                        value_after,
                        change_type: occurrences.first().and_then(|o| o.change_type.clone()),
                        scope: None,
                        tags: HashMap::new(),
                    });
                }
            }
            FinalizeFinding {
                finding_key: result.block_hash,
                content: Some(result.block_content),
                occurrences: result.occurrences,
                tags: tag_overrides.clone(),
                metrics,
            }
        })
        .collect();
    FinalizeOutput { findings }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_generic_and_legacy_finalize_protocols() {
        let generic = r#"{"findings":[{"finding_key":"key","metrics":[],"occurrences":[]}]}"#;
        assert!(matches!(
            serde_json::from_str::<ScriptFinalizeResponse>(generic).unwrap(),
            ScriptFinalizeResponse::Generic(_)
        ));

        let legacy =
            r#"[{"block_hash":"hash","block_content":"body","block_size":1,"occurrences":[]}]"#;
        let parsed = serde_json::from_str::<ScriptFinalizeResponse>(legacy).unwrap();
        assert!(matches!(parsed, ScriptFinalizeResponse::Legacy(_)));

        let legacy = vec![FinalizeMatchResult {
            block_hash: "hash".into(),
            block_content: "body".into(),
            block_size: 1,
            occurrences: vec![FinalizeOccurrence {
                file_path: "a.rs".into(),
                line_start: 1,
                line_end: 2,
                change_type: Some("A".into()),
                side: None,
            }],
        }];
        assert_eq!(
            legacy_output(legacy, &HashMap::new()).findings[0]
                .metrics
                .len(),
            4
        );
    }
}
