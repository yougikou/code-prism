use crate::Analyzer;
use codeprism_core::MetricEntry;
use serde::{Deserialize, Serialize};
use std::fs;
use std::sync::{Arc, RwLock};
use wasi_common::pipe::{ReadPipe, WritePipe};
use wasmtime::*;
use wasmtime_wasi::WasiCtxBuilder;

#[derive(Serialize, Deserialize)]
struct WasmInput {
    file_path: String,
    content: String,
}

#[derive(Serialize, Deserialize)]
struct WasmOutput {
    metric_key: String,
    value: f64,
    category: Option<String>,
}

pub struct WasmAnalyzer {
    id: String,
    engine: Engine,
    module: Module,
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
        })
    }
}

impl Analyzer for WasmAnalyzer {
    fn id(&self) -> &str {
        &self.id
    }

    fn analyze(&self, file_path: &str, content: &str) -> Vec<MetricEntry> {
        // Prepare Input JSON
        let input = WasmInput {
            file_path: file_path.to_string(),
            content: content.to_string(),
        };
        let json_bytes = match serde_json::to_vec(&input) {
            Ok(b) => b,
            Err(_) => return vec![],
        };

        // Output Buffer (Shared with Pipe)
        let stdout_buf = Arc::new(RwLock::new(Vec::new()));

        // Pipes (wasi-common 18.x)
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

        // Add WASI (Preview 1) support - sync
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

        // Call _start
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
                // if let Ok(s) = std::str::from_utf8(&result_bytes) { eprintln!("Raw: {}", s); }
                return vec![];
            }
        };

        raw_outputs
            .into_iter()
            .map(|o| MetricEntry {
                analyzer_id: self.id.clone(),
                metric_key: o.metric_key,
                category: o.category,
                value: o.value,
                scope: None,
                tech_stack: None,
            })
            .collect()
    }
}
