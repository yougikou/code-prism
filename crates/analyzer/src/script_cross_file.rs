use crate::{Analyzer, FileProcessor};
use async_trait::async_trait;
use codeprism_core::{
    ContentBlock, FinalizeMatchResult, FinalizeOccurrence, IntermediateBlock,
    ScriptContentBlock, TAG_CATEGORY, TAG_METRIC,
};
use serde::Serialize;
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

/// Default tags captured from the Python script's own output.
/// These serve as the fallback when no YAML overrides are configured.
struct ScriptDefaultTags {
    metric_key: Option<String>,
    category: Option<String>,
    tags: Option<HashMap<String, String>>,
}

pub struct ScriptCrossFileAnalyzer {
    id: String,
    /// YAML-configured tag overrides (from `custom_cross_file_analyzers`).
    /// Applied on top of whatever the Python script defines as defaults.
    tag_overrides: HashMap<String, String>,
    /// Script's own default tags, captured from the first script invocation.
    script_default_tags: Arc<Mutex<Option<ScriptDefaultTags>>>,
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
            script_default_tags: Arc::new(Mutex::new(None)),
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
            {
                if output.success() {
                    return Ok(cmd.to_string());
                }
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
                .arg(&self.script_path())
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
        if let Ok(mut guard) = self.process.lock() {
            if guard.is_some() {
                *guard = None;
            }
        }
    }

    /// Internal: run the Python script's `extract` action and return ContentBlocks.
    ///
    /// On the first invocation, also captures the script's own default tags
    /// (metric_key, category, tags) for use in finalize().
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
        let script_default_tags = self.script_default_tags.clone();
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

                let script_blocks: Vec<ScriptContentBlock> =
                    serde_json::from_str(&line).ok()?;

                // Capture script's default tags from the first block on first call
                if let Some(first) = script_blocks.first() {
                    let mut dst = script_default_tags.lock().unwrap();
                    if dst.is_none() {
                        *dst = Some(ScriptDefaultTags {
                            metric_key: first.metric_key.clone(),
                            category: first.category.clone(),
                            tags: first.tags.clone(),
                        });
                    }
                }

                let blocks = script_blocks
                    .into_iter()
                    .map(ContentBlock::from)
                    .collect();

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
                eprintln!("Cross-file analyzer '{}' thread crashed during finalize.", aid);
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
                analyzer_id: String::new(),  // filled by pipeline
                file_path: String::new(),    // filled by pipeline
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

    async fn finalize(
        &self,
        scan_id: i64,
        pool: &sqlx::Pool<sqlx::Sqlite>,
    ) -> anyhow::Result<()> {
        // 1. Fetch intermediate blocks for this analyzer
        let rows = sqlx::query_as::<_, (String, String, Option<String>, Option<i64>, Option<i64>, Option<i64>, Option<String>, Option<String>)>(
            "SELECT file_path, group_key, blob_data, int_data1, int_data2, int_data3, str_data1, str_data2 \
             FROM intermediate_blocks WHERE scan_id = ? AND analyzer_id = ? ORDER BY group_key"
        )
        .bind(scan_id)
        .bind(&self.id)
        .fetch_all(pool)
        .await?;

        if rows.is_empty() {
            return Ok(());
        }

        // 2. Build block data for Python script
        let blocks: Vec<BlockData> = rows
            .iter()
            .map(
                |(fp, gk, bd, i1, i2, i3, s1, s2)| BlockData {
                    file_path: fp.clone(),
                    group_key: gk.clone(),
                    blob_data: bd.clone(),
                    int_data1: *i1,
                    int_data2: *i2,
                    int_data3: *i3,
                    str_data1: s1.clone(),
                    str_data2: s2.clone(),
                },
            )
            .collect();

        // 3. Send to Python script for aggregation
        let response = self.send_finalize(blocks).map_err(|e| {
            anyhow::anyhow!("Finalize failed for '{}': {}", self.id, e)
        })?;

        // 4. Parse results — the script owns threshold logic and returns only
        //    groups that passed (e.g. min_file_count, min_block_count, etc.)
        let match_results: Vec<FinalizeMatchResult> = serde_json::from_str(&response)
            .map_err(|e| {
                anyhow::anyhow!(
                    "Failed to parse finalize response from '{}': {}",
                    self.id,
                    e
                )
            })?;

        if match_results.is_empty() {
            // Clean up intermediate blocks even with no matches
            sqlx::query(
                "DELETE FROM intermediate_blocks WHERE scan_id = ? AND analyzer_id = ?",
            )
            .bind(scan_id)
            .bind(&self.id)
            .execute(pool)
            .await?;
            return Ok(());
        }

        // 5. Write matches and metrics in a transaction
        let mut tx = pool.begin().await?;

        for result in &match_results {
            let content_hash = codeprism_core::hash_match_content(&result.block_content);
            let (content_id, stored_content): (i64, String) = sqlx::query_as(
                "INSERT INTO match_contents (content_hash, content, content_bytes, line_count) \
                 VALUES (?, ?, ?, ?) \
                 ON CONFLICT(content_hash) DO UPDATE SET content_hash = excluded.content_hash \
                 RETURNING id, content",
            )
            .bind(&content_hash)
            .bind(&result.block_content)
            .bind(result.block_content.len() as i64)
            .bind(result.block_content.lines().count() as i64)
            .fetch_one(&mut *tx)
            .await?;
            if stored_content != result.block_content {
                anyhow::bail!("SHA-256 collision while storing cross-file finding content");
            }

            let aggregated_analyzer_id = format!("{}_aggregated", self.id);

            // Every occurrence keeps its own location while sharing one content row.
            for occurrence in &result.occurrences {
                let side = occurrence
                    .side
                    .as_deref()
                    .and_then(|value| value.parse::<i64>().ok());
                sqlx::query(
                    "INSERT INTO matches (scan_id, file_path, analyzer_id, content_id, finding_key, \
                     line_start, line_end, column_start, column_end, side, change_type, \
                     context_before, context_after) \
                     VALUES (?, ?, ?, ?, ?, ?, ?, NULL, NULL, ?, ?, NULL, NULL)",
                )
                .bind(scan_id)
                .bind(&occurrence.file_path)
                .bind(&aggregated_analyzer_id)
                .bind(content_id)
                .bind(&result.block_hash)
                .bind(occurrence.line_start as i64)
                .bind(occurrence.line_end as i64)
                .bind(side)
                .bind(occurrence.change_type.as_deref().unwrap_or("A"))
                .execute(&mut *tx)
                .await?;
            }

            // — Per-file metrics: group occurrences by file_path for side aggregation —
            let mut file_groups: BTreeMap<&str, Vec<&FinalizeOccurrence>> = BTreeMap::new();
            for occ in &result.occurrences {
                file_groups
                    .entry(occ.file_path.as_str())
                    .or_default()
                    .push(occ);
            }

            let finding_has_before = result
                .occurrences
                .iter()
                .any(|occurrence| occurrence.side.as_deref() == Some("0"));
            let finding_has_after = result.occurrences.iter().any(|occurrence| {
                occurrence.side.is_none() || occurrence.side.as_deref() == Some("1")
            });

            for (file_index, (file_path, file_entries)) in file_groups.iter().enumerate() {
                let first = file_entries.first().unwrap();
                let line_start = first.line_start;
                let line_end = first.line_end;
                let change_type = first.change_type.clone();

                // Snapshot vs diff mode: side="0" (old), side="1" (new)
                let has_side_0 = file_entries.iter().any(|o| o.side.as_deref() == Some("0"));
                let has_side_1 = file_entries.iter().any(|o| o.side.as_deref() == Some("1"));

                let (value_before, value_after) = if !has_side_0 && !has_side_1 {
                    (0.0, 1.0)
                } else {
                    let vb = if has_side_0 { 1.0 } else { 0.0 };
                    let va = if has_side_1 { 1.0 } else { 0.0 };
                    (vb, va)
                };

                if value_before == 0.0 && value_after == 0.0 {
                    continue;
                }

                // — Tag merging: script defaults → YAML overrides → auto fields —
                let mut tags = HashMap::new();
                {
                    let sd = self.script_default_tags.lock().unwrap();
                    if let Some(dt) = sd.as_ref() {
                        if let Some(mk) = &dt.metric_key {
                            tags.insert(TAG_METRIC.to_string(), mk.clone());
                        }
                        if let Some(cat) = &dt.category {
                            tags.insert(TAG_CATEGORY.to_string(), cat.clone());
                        }
                        if let Some(st) = &dt.tags {
                            tags.extend(st.clone());
                        }
                    }
                }
                for (k, v) in &self.tag_overrides {
                    tags.insert(k.clone(), v.clone());
                }

                let effective_change_type = change_type.as_deref().unwrap_or("A");
                let before_occurrences = file_entries
                    .iter()
                    .filter(|occurrence| occurrence.side.as_deref() == Some("0"))
                    .count() as f64;
                let after_occurrences = file_entries
                    .iter()
                    .filter(|occurrence| {
                        occurrence.side.is_none() || occurrence.side.as_deref() == Some("1")
                    })
                    .count() as f64;
                let before_lines = file_entries
                    .iter()
                    .filter(|occurrence| occurrence.side.as_deref() == Some("0"))
                    .map(|occurrence| (occurrence.line_end - occurrence.line_start + 1).max(0) as f64)
                    .sum::<f64>();
                let after_lines = file_entries
                    .iter()
                    .filter(|occurrence| {
                        occurrence.side.is_none() || occurrence.side.as_deref() == Some("1")
                    })
                    .map(|occurrence| (occurrence.line_end - occurrence.line_start + 1).max(0) as f64)
                    .sum::<f64>();

                let base_metric = tags
                    .get(TAG_METRIC)
                    .cloned()
                    .unwrap_or_else(|| "finding_count".to_string());
                let mut metric_values = BTreeMap::new();
                metric_values.insert(base_metric, (value_before, value_after));
                metric_values.insert(
                    "occurrence_count".to_string(),
                    (before_occurrences, after_occurrences),
                );
                metric_values.insert(
                    "affected_file_count".to_string(),
                    (value_before, value_after),
                );
                metric_values.insert(
                    "affected_line_count".to_string(),
                    (before_lines, after_lines),
                );
                if file_index == 0 {
                    metric_values.insert(
                        "finding_count".to_string(),
                        (
                            if finding_has_before { 1.0 } else { 0.0 },
                            if finding_has_after { 1.0 } else { 0.0 },
                        ),
                    );
                }

                for (metric_name, (metric_before, metric_after)) in metric_values {
                    let mut metric_tags = tags.clone();
                    metric_tags.insert(TAG_METRIC.to_string(), metric_name);
                    let tags_json = {
                        let sorted: BTreeMap<_, _> = metric_tags.iter().collect();
                        serde_json::to_string(&sorted).unwrap_or_default()
                    };
                    sqlx::query(
                        "INSERT INTO metrics (scan_id, file_path, change_type, tech_stack, \
                         analyzer_id, content_id, finding_key, tags, value_before, value_after, scope) \
                         VALUES (?, ?, ?, NULL, ?, ?, ?, ?, ?, ?, ?)",
                    )
                    .bind(scan_id)
                    .bind(file_path)
                    .bind(effective_change_type)
                    .bind(&aggregated_analyzer_id)
                    .bind(content_id)
                    .bind(&result.block_hash)
                    .bind(&tags_json)
                    .bind(metric_before)
                    .bind(metric_after)
                    .bind(Some(format!("{}:{}-{}", self.id, line_start, line_end)))
                    .execute(&mut *tx)
                    .await?;
                }
            }
        }

        tx.commit().await?;

        // 6. Clean up intermediate blocks
        sqlx::query("DELETE FROM intermediate_blocks WHERE scan_id = ? AND analyzer_id = ?")
            .bind(scan_id)
            .bind(&self.id)
            .execute(pool)
            .await?;

        Ok(())
    }
}
