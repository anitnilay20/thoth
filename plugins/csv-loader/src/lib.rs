// CSV / TSV file loader plugin for Thoth.
// Compiled to WASM with: cargo component build --release
// Output copied to: assets/plugins/csv-loader/plugin.wasm

// cargo-component generates this file automatically from wit/thoth-plugin.wit.
// Do not call wit_bindgen::generate! — it is handled for you.
#[rustfmt::skip]
mod bindings;

use std::path::PathBuf;

use thoth_plugin_sdk::prelude::*;

use bindings::exports::thoth::plugin::{
    file_loader::Guest as FileLoaderGuest,
    file_viewer::{DisplayMode, Guest as FileViewerGuest},
    plugin_lifecycle::Guest as LifecycleGuest,
    plugin_settings::{Guest as SettingsGuest, SettingsOutput},
};
use bindings::thoth::plugin::types::PluginError;
use csv::ReaderBuilder;
use serde_json::{Map, Value};

#[derive(PluginMeta)]
#[plugin(
    id = "com.thoth.csv-loader",
    name = "CSV Loader",
    version = "0.1.0",
    description = "Load CSV and TSV files as tabular JSON records",
    capabilities = [FileLoader, FileViewer],
    author = "Thoth contributors",
)]
struct CsvPlugin;

// ── per-instance state stored inside the WASM sandbox ────────────────────────

struct State {
    headers: Vec<String>,
    file: PathBuf,
    /// Delimiter byte: b',' for CSV, b'\t' for TSV.
    delimiter: u8,
}

static STATE: PluginState<State> = PluginState::new();


// ── plugin-lifecycle ──────────────────────────────────────────────────────────

impl LifecycleGuest for CsvPlugin {
    fn on_load(_setting: String) {
        // No user-configurable settings — plugin is stateless from a
        // configuration perspective; runtime state lives in STATE above.
    }

    fn on_close() {
        STATE.reset();
    }

    fn on_setting_change(_setting: String) {
        // No action needed since this plugin is entirely stateless and has no settings.
    }
}

// ── file-loader ───────────────────────────────────────────────────────────────

impl FileLoaderGuest for CsvPlugin {
    fn supported_extensions() -> Vec<String> {
        vec!["csv".to_string(), "tsv".to_string()]
    }

    fn open(path: String) -> Result<u64, PluginError> {
        let delimiter = if path.ends_with(".tsv") { b'\t' } else { b',' };

        let mut rdr = ReaderBuilder::new()
            .delimiter(delimiter)
            .has_headers(true)
            .from_path(&path)
            .map_err(|e| plugin_err(1, e.to_string()))?;

        let headers: Vec<String> = rdr
            .headers()
            .map_err(|e| plugin_err(1, e.to_string()))?
            .iter()
            .map(str::to_owned)
            .collect();

        // Count records without loading them all into memory.
        let count = rdr.records().count() as u64;

        STATE.set(State {
            headers,
            file: PathBuf::from(path),
            delimiter,
        });

        Ok(count)
    }

    fn get(idx: u64) -> Result<String, PluginError> {
        STATE
            .try_with(|state| {
                let mut rdr = ReaderBuilder::new()
                    .delimiter(state.delimiter)
                    .has_headers(true)
                    .from_path(&state.file)
                    .map_err(|e| plugin_err(1, e.to_string()))?;

                let record = rdr
                    .records()
                    .nth(idx as usize)
                    .ok_or_else(|| plugin_err(2, "index out of range"))?
                    .map_err(|e| plugin_err(1, e.to_string()))?;

                let obj: Map<String, Value> = state
                    .headers
                    .iter()
                    .zip(record.iter())
                    .map(|(k, v)| (k.clone(), Value::String(v.to_owned())))
                    .collect();

                serde_json::to_string(&Value::Object(obj)).map_err(|e| plugin_err(3, e.to_string()))
            })
            .unwrap_or_else(|| Err(plugin_err(2, "file not opened")))
    }

    fn raw_bytes(idx: u64) -> Result<Vec<u8>, PluginError> {
        STATE
            .try_with(|state| {
                // Return the original (unparsed) CSV/TSV bytes for this record by
                // reading the ByteRecord at `idx` and reconstructing the delimited line.
                let mut rdr = ReaderBuilder::new()
                    .delimiter(state.delimiter)
                    .has_headers(true)
                    .from_path(&state.file)
                    .map_err(|e| plugin_err(1, e.to_string()))?;

                let record = rdr
                    .byte_records()
                    .nth(idx as usize)
                    .ok_or_else(|| plugin_err(2, "index out of range"))?
                    .map_err(|e| plugin_err(1, e.to_string()))?;

                let mut out = Vec::new();
                for (i, field) in record.iter().enumerate() {
                    if i > 0 {
                        out.push(state.delimiter);
                    }
                    out.extend_from_slice(field);
                }
                Ok(out)
            })
            .unwrap_or_else(|| Err(plugin_err(2, "file not opened")))
    }
}

// ── file-viewer ───────────────────────────────────────────────────────────────

impl FileViewerGuest for CsvPlugin {
    fn preferred_display() -> DisplayMode {
        DisplayMode::Table
    }

    fn column_headers() -> Option<Vec<String>> {
        STATE.try_with(|state| state.headers.clone())
    }

    fn render_record(
        _record_json: String,
    ) -> Result<bindings::exports::thoth::plugin::file_viewer::RenderOutput, PluginError> {
        // Not called in table mode — host renders cells directly from file-loader.get()
        Err(plugin_err(0, "not used in table mode"))
    }
}

// ── plugin-settings ───────────────────────────────────────────────────────────
// CSV loader has no user-configurable settings, so all methods are no-ops.

impl SettingsGuest for CsvPlugin {
    fn render_settings() -> Result<SettingsOutput, bindings::thoth::plugin::types::PluginError> {
        use thoth_plugin_sdk::ToNodeJson;
        use thoth_plugin_sdk::components::Typography;
        use thoth_plugin_sdk::render_node::RenderNode;

        let node = RenderNode::Text(
            Typography::builder()
                .text("This plugin has no settings.")
                .build(),
        );
        Ok(SettingsOutput {
            node_json: node.to_json().to_string(),
            height_hint: 0,
        })
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
