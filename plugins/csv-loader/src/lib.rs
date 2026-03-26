// CSV / TSV file loader plugin for Thoth.
// Compiled to WASM with: cargo component build --release
// Output copied to: assets/plugins/csv-loader/plugin.wasm

// cargo-component generates this file automatically from wit/thoth-plugin.wit.
// Do not call wit_bindgen::generate! — it is handled for you.
mod bindings;

use std::cell::RefCell;

use bindings::exports::thoth::plugin::{
    file_loader::Guest as FileLoaderGuest,
    plugin_lifecycle::Guest as LifecycleGuest,
    plugin_meta::Guest as MetaGuest,
};
use bindings::thoth::plugin::types::{Capability, PluginError};
use csv::ReaderBuilder;
use serde_json::{Map, Value};

struct CsvPlugin;

// ── per-instance state stored inside the WASM sandbox ────────────────────────

struct State {
    headers: Vec<String>,
    records: Vec<Vec<String>>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

// ── plugin-meta ───────────────────────────────────────────────────────────────

impl MetaGuest for CsvPlugin {
    fn get_info() -> bindings::exports::thoth::plugin::plugin_meta::PluginInfo {
        bindings::exports::thoth::plugin::plugin_meta::PluginInfo {
            id:           "com.thoth.csv-loader".to_string(),
            name:         "CSV Loader".to_string(),
            version:      "0.1.0".to_string(),
            description:  "Load CSV and TSV files as tabular JSON records".to_string(),
            capabilities: vec![Capability::FileLoader],
            author:       Some("Thoth contributors".to_string()),
            homepage:     None,
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
        let delimiter = if path.ends_with(".tsv") { b'\t' } else { b',' };

        let mut reader = ReaderBuilder::new()
            .delimiter(delimiter)
            .from_path(&path)
            .map_err(|e: csv::Error| plugin_err(1, e.to_string()))?;

        let headers: Vec<String> = reader
            .headers()
            .map_err(|e: csv::Error| plugin_err(1, e.to_string()))?
            .iter()
            .map(str::to_owned)
            .collect();

        let records: Vec<Vec<String>> = reader
            .records()
            .map(|r: Result<csv::StringRecord, csv::Error>| {
                r.map(|row: csv::StringRecord| row.iter().map(str::to_owned).collect())
                    .map_err(|e| plugin_err(1, e.to_string()))
            })
            .collect::<Result<_, _>>()?;

        let count = records.len() as u64;
        STATE.with(|s| *s.borrow_mut() = Some(State { headers, records }));
        Ok(count)
    }

    fn get(idx: u64) -> Result<String, PluginError> {
        STATE.with(|s| {
            let guard = s.borrow();
            let state = guard.as_ref().ok_or(plugin_err(2, "file not opened"))?;
            let row = state
                .records
                .get(idx as usize)
                .ok_or(plugin_err(2, "index out of range"))?;

            let obj: Map<String, Value> = state
                .headers
                .iter()
                .zip(row.iter())
                .map(|(k, v): (&String, &String)| (k.clone(), Value::String(v.clone())))
                .collect();

            serde_json::to_string(&Value::Object(obj)).map_err(|e| plugin_err(3, e.to_string()))
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
    PluginError { code, message: message.into() }
}
