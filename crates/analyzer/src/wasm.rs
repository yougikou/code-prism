use crate::Analyzer;
use codeprism_core::{MatchDetail, MetricEntry};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fs;
use std::sync::{Arc, Mutex, RwLock};
use wasi_common::pipe::{ReadPipe, WritePipe};
use wasmtime::*;
use wasmtime_wasi::WasiCtxBuilder;

#[derive(Serialize, Deserialize)]
struct WasmInput {
    file_path: String,
    content: String,
    change_type: String,
}

#[derive(Serialize, Deserialize)]
struct WasmOutput {
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

impl WasmOutput {
    fn resolve_tags(&self) -> HashMap<String, String> {
        let mut result = self.tags.clone();
        if let Some(mk) = &self.metric_key {
            result.insert(codeprism_core::TAG_METRIC.to_string(), mk.clone());
        }
        if let Some(cat) = &self.category {
            result.insert(codeprism_core::TAG_CATEGORY.to_string(), cat.clone());
        }
        result
    }
}

pub struct WasmAnalyzer {
    id: String,
    engine: Engine,
    module: Module,
    matches_cache: Arc<Mutex<Vec<MatchDetail>>>,
    current_change_type: Arc<Mutex<String>>,
}

impl WasmAnalyzer {
    pub fn new(id: &str, wasm_path: &str) -> anyhow::Result<Self> {
        let engine = Engine::default();
        let wasm_bytes = fs::read(wasm_path)?;
        let module = Module::new(&engine, &wasm_bytes)?;

        Ok(Self {
            id: id.to_string(),
            engine,
            module,
            matches_cache: Arc::new(Mutex::new(Vec::new())),
            current_change_type: Arc::new(Mutex::new(String::new())),
        })
    }
}

impl Analyzer for WasmAnalyzer {
    fn id(&self) -> &str {
        &self.id
    }

    fn analyze(&self, file_path: &str, content: &str) -> Vec<MetricEntry> {
        // Reset matches cache for this file
        self.matches_cache.lock().unwrap().clear();

        // Prepare Input JSON
        let change_type = self.current_change_type.lock().unwrap().clone();
        let input = WasmInput {
            file_path: file_path.to_string(),
            content: content.to_string(),
            change_type,
        };
        let json_bytes = match serde_json::to_vec(&input) {
            Ok(b) => b,
            Err(_) => return vec![],
        };

        // Output Buffer (Shared with Pipe)
        let stdout_buf = Arc::new(RwLock::new(Vec::new()));

        // Pipes
        let stdin = ReadPipe::from(json_bytes);
        let stdout = WritePipe::from_shared(stdout_buf.clone());

        let wasi = WasiCtxBuilder::new()
            .stdin(Box::new(stdin))
            .stdout(Box::new(stdout))
            .arg("analyzer.wasm")
            .unwrap()
            .inherit_stderr()
            .build();

        let mut store = Store::new(&self.engine, wasi);
        let mut linker = Linker::new(&self.engine);

        if let Err(e) = wasmtime_wasi::add_to_linker(&mut linker, |s| s) {
            eprintln!("Failed to link WASI: {}", e);
            return vec![];
        }

        let instance = match linker.instantiate(&mut store, &self.module) {
            Ok(i) => i,
            Err(e) => {
                eprintln!("Wasm instantiation failed: {}", e);
                return vec![];
            }
        };

        let start_func = instance.get_typed_func::<(), ()>(&mut store, "_start");

        if let Ok(func) = start_func {
            if let Err(e) = func.call(&mut store, ()) {
                eprintln!("WASI execution failed: {}", e);
            }
        } else {
            eprintln!("_start function missing");
            return vec![];
        }

        // Read Output
        let result_bytes = {
            let buf = stdout_buf.read().unwrap();
            buf.clone()
        };

        if result_bytes.is_empty() {
            return vec![];
        }

        // Parse Output
        let raw_outputs: Vec<WasmOutput> = match serde_json::from_slice(&result_bytes) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("Failed to parse Wasm output JSON: {}", e);
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

        raw_outputs
            .into_iter()
            .map(|o| MetricEntry {
                analyzer_id: self.id.clone(),
                tags: o.resolve_tags(),
                value: o.value,
                scope: None,
                tech_stack: None,
            })
            .collect()
    }

    fn extract_matches(&self, _path: &str, _content: &str) -> Vec<MatchDetail> {
        self.matches_cache.lock().unwrap().clone()
    }

    fn set_file_context(&self, change_type: &str, _scan_mode: &str) {
        *self.current_change_type.lock().unwrap() = change_type.to_string();
    }
}
