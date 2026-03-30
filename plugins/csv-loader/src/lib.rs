// CSV / TSV file loader plugin for Thoth.
// Compiled to WASM with: cargo component build --release
// Output copied to: assets/plugins/csv-loader/plugin.wasm

// cargo-component generates this file automatically from wit/thoth-plugin.wit.
// Do not call wit_bindgen::generate! — it is handled for you.
mod bindings;

use std::cell::RefCell;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;

use bindings::exports::thoth::plugin::{
    file_loader::Guest as FileLoaderGuest, plugin_lifecycle::Guest as LifecycleGuest,
    plugin_meta::Guest as MetaGuest,
};
use bindings::thoth::plugin::types::{Capability, PluginError};
use serde_json::{Map, Value};

struct CsvPlugin;

// ── per-instance state stored inside the WASM sandbox ────────────────────────

struct State {
    headers: Vec<String>,
    file: PathBuf,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

// ── plugin-meta ───────────────────────────────────────────────────────────────

impl MetaGuest for CsvPlugin {
    fn get_info() -> bindings::exports::thoth::plugin::plugin_meta::PluginInfo {
        bindings::exports::thoth::plugin::plugin_meta::PluginInfo {
            id: "com.thoth.csv-loader".to_string(),
            name: "CSV Loader".to_string(),
            version: "0.1.0".to_string(),
            description: "Load CSV and TSV files as tabular JSON records".to_string(),
            capabilities: vec![Capability::FileLoader],
            author: Some("Thoth contributors".to_string()),
            homepage: None,
        }
    }
}

// ── plugin-lifecycle ──────────────────────────────────────────────────────────

impl LifecycleGuest for CsvPlugin {
    fn on_load() {}

    fn on_close() {
        STATE.with(|s| *s.borrow_mut() = None);
    }
}

// ── file-loader ───────────────────────────────────────────────────────────────

impl FileLoaderGuest for CsvPlugin {
    fn supported_extensions() -> Vec<String> {
        vec!["csv".to_string(), "tsv".to_string()]
    }

    fn open(path: String) -> Result<u64, PluginError> {
        let file = std::fs::File::open(&path).map_err(|e| plugin_err(1, e.to_string()))?;
        let mut reader = BufReader::new(file);
        let mut buf = String::new();
        reader
            .read_line(&mut buf)
            .map_err(|err| plugin_err(1, err.to_string()))?;
        let headers: Vec<String> = buf.trim_end().split(',').map(String::from).collect();

        STATE.with(|s| {
            *s.borrow_mut() = Some(State {
                headers,
                file: PathBuf::from(path),
            })
        });
        Ok(reader.lines().count() as u64)
    }

    fn get(idx: u64) -> Result<String, PluginError> {
        STATE.with(|s| {
            let guard = s.borrow();
            let state = guard.as_ref().ok_or(plugin_err(2, "file not opened"))?;

            let file =
                std::fs::File::open(&state.file).map_err(|e| plugin_err(1, e.to_string()))?;
            let reader = BufReader::new(file);

            for (index, line_result) in reader.lines().enumerate() {
                // Skip the header line (index 0) and check if we've reached the desired record index.
                if index - 1 == idx as usize {
                    let line = line_result.map_err(|e| plugin_err(1, e.to_string()))?;
                    let values: Vec<String> =
                        line.trim_end().split(',').map(String::from).collect();
                    let obj: Map<String, Value> = state
                        .headers
                        .iter()
                        .zip(values.iter())
                        .map(|(k, v): (&String, &String)| (k.clone(), Value::String(v.clone())))
                        .collect();
                    return serde_json::to_string(&Value::Object(obj))
                        .map_err(|e| plugin_err(3, e.to_string()));
                }
            }

            Err(plugin_err(2, "record index out of bounds"))
        })
    }

    fn raw_bytes(idx: u64) -> Result<Vec<u8>, PluginError> {
        Self::get(idx).map(String::into_bytes)
    }
}

// Register all exports with the WASM component runtime
bindings::export!(CsvPlugin with_types_in bindings);

// ── helpers ───────────────────────────────────────────────────────────────────

fn plugin_err(code: u32, message: impl Into<String>) -> PluginError {
    PluginError {
        code,
        message: message.into(),
    }
}
