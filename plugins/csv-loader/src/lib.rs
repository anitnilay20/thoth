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
use csv::{ByteRecord, Position, ReaderBuilder, StringRecord};
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
    /// Byte position of the start of each data record (header excluded),
    /// captured once during `open()`. Lets `get(idx)` seek straight to a
    /// record — O(1) random access instead of re-parsing from row 0.
    positions: Vec<Position>,
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

        // Single sequential pass: record the byte position at the start of each
        // data record (only the ~24-byte Position, not the row itself), so
        // random access later is a seek rather than a re-parse from row 0.
        let mut positions = Vec::new();
        let mut record = ByteRecord::new();
        loop {
            let pos = rdr.position().clone();
            match rdr.read_byte_record(&mut record) {
                Ok(true) => positions.push(pos),
                Ok(false) => break,
                Err(e) => return Err(plugin_err(1, e.to_string())),
            }
        }

        let count = positions.len() as u64;
        STATE.set(State {
            headers,
            file: PathBuf::from(path),
            delimiter,
            positions,
        });

        Ok(count)
    }

    fn get(idx: u64) -> Result<String, PluginError> {
        STATE
            .try_with(|state| {
                let row = usize::try_from(idx).map_err(|_| plugin_err(2, "index out of range"))?;
                let mut rdr = reader_seeked(state, row)?;
                let mut record = StringRecord::new();
                if !rdr
                    .read_record(&mut record)
                    .map_err(|e| plugin_err(1, e.to_string()))?
                {
                    return Err(plugin_err(2, "index out of range"));
                }
                record_to_json(&state.headers, &record)
            })
            .unwrap_or_else(|| Err(plugin_err(2, "file not opened")))
    }

    fn get_range(start: u64, count: u64) -> Result<Vec<String>, PluginError> {
        STATE
            .try_with(|state| {
                let start =
                    usize::try_from(start).map_err(|_| plugin_err(2, "start out of range"))?;
                let count = usize::try_from(count).unwrap_or(usize::MAX);
                if start >= state.positions.len() {
                    return Ok(Vec::new());
                }

                // One seek to `start`, then a single sequential pass of `count`
                // records — O(count), no re-parse from row 0.
                let mut rdr = reader_seeked(state, start)?;
                let mut out = Vec::with_capacity(count.min(4096));
                let mut record = StringRecord::new();
                while out.len() < count {
                    if !rdr
                        .read_record(&mut record)
                        .map_err(|e| plugin_err(1, e.to_string()))?
                    {
                        break;
                    }
                    out.push(record_to_json(&state.headers, &record)?);
                }
                Ok(out)
            })
            .unwrap_or_else(|| Err(plugin_err(2, "file not opened")))
    }

    fn raw_bytes(idx: u64) -> Result<Vec<u8>, PluginError> {
        STATE
            .try_with(|state| {
                // Return the original (unparsed) CSV/TSV bytes for this record by
                // seeking to the record at `idx` and reconstructing the delimited line.
                let row = usize::try_from(idx).map_err(|_| plugin_err(2, "index out of range"))?;
                let mut rdr = reader_seeked(state, row)?;
                let mut record = ByteRecord::new();
                if !rdr
                    .read_byte_record(&mut record)
                    .map_err(|e| plugin_err(1, e.to_string()))?
                {
                    return Err(plugin_err(2, "index out of range"));
                }

                // Re-serialize through the CSV writer so fields containing the
                // delimiter, quotes, or newlines stay properly escaped.
                let mut wtr = csv::WriterBuilder::new()
                    .delimiter(state.delimiter)
                    .from_writer(Vec::new());
                wtr.write_byte_record(&record)
                    .map_err(|e| plugin_err(1, e.to_string()))?;
                let mut out = wtr.into_inner().map_err(|e| plugin_err(1, e.to_string()))?;
                // Drop the trailing line terminator to keep this API newline-free.
                while out.last() == Some(&b'\n') || out.last() == Some(&b'\r') {
                    out.pop();
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
        use thoth_plugin_sdk::components::Typography;
        use thoth_plugin_sdk::render_node::RenderNode;
        use thoth_plugin_sdk::ToNodeJson;

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

/// Open a fresh reader and seek to the start of data record `row`. Uses the
/// position index from `open()`, so this is O(1) rather than a scan from row 0.
/// The reader is built with `has_headers(false)` — we seek straight to a data
/// record, so there is no header line to skip.
fn reader_seeked(state: &State, row: usize) -> Result<csv::Reader<std::fs::File>, PluginError> {
    let pos = state
        .positions
        .get(row)
        .cloned()
        .ok_or_else(|| plugin_err(2, "index out of range"))?;
    let mut rdr = ReaderBuilder::new()
        .delimiter(state.delimiter)
        .has_headers(false)
        .from_path(&state.file)
        .map_err(|e| plugin_err(1, e.to_string()))?;
    rdr.seek(pos).map_err(|e| plugin_err(1, e.to_string()))?;
    Ok(rdr)
}

/// Zip a record against the header row into a JSON object string.
fn record_to_json(headers: &[String], record: &StringRecord) -> Result<String, PluginError> {
    let obj: Map<String, Value> = headers
        .iter()
        .zip(record.iter())
        .map(|(k, v)| (k.clone(), Value::String(v.to_owned())))
        .collect();
    serde_json::to_string(&Value::Object(obj)).map_err(|e| plugin_err(3, e.to_string()))
}
