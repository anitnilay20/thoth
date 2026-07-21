mod json_array;
mod ndjson;
mod single;

pub use json_array::JsonArrayFile;
pub use ndjson::NdjsonFile;
pub use single::SingleValueFile;

use crate::error::Result;
use crate::file::detect_file_type::DetectedFileType;
use crate::plugin::wasm_file_viewer_loader::{DisplayMode, WasmFileViewerLoader};
use crate::plugin::wasm_loader::WasmFileLoader;
use serde_json::Value;
use std::path::Path;

/// Common trait for all lazy file loaders.
///
/// # Design Philosophy
/// - Loaders should perform minimal work during `open()` — just enough to index the file
/// - Actual parsing happens lazily on `get()` calls
/// - All read operations should be position-independent (safe for parallel access)
#[allow(dead_code)]
pub trait FileLoader {
    type Item;

    fn open(path: &Path) -> Result<Self>
    where
        Self: Sized;

    fn len(&self) -> usize;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn get(&mut self, idx: usize) -> Result<Self::Item>;

    fn raw_bytes(&self, idx: usize) -> Result<Vec<u8>>;
}

// ── Lightweight discriminant (Copy, stored in state/events) ───────────────────

/// A lightweight, `Copy` tag describing what kind of file is loaded.
/// Used in window state, toolbar events, and status bar display.
/// Does not hold any file handles.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FileKind {
    #[default]
    Ndjson,
    Json,
    Plugin,
    PluginTable,
}

impl From<DetectedFileType> for FileKind {
    fn from(val: DetectedFileType) -> Self {
        match val {
            DetectedFileType::Ndjson => FileKind::Ndjson,
            DetectedFileType::JsonArray | DetectedFileType::JsonObject => FileKind::Json,
        }
    }
}

// ── Fat loader enum (owns file handles) ───────────────────────────────────────

/// Unified file loader — dispatches to the right implementation and owns all
/// file handles. Add new formats here; callers only deal with this one type.
pub enum FileType {
    Ndjson(NdjsonFile),
    JsonArray(JsonArrayFile),
    Single(SingleValueFile),
    /// Loaded via a WASM plugin (file-loader only).
    Plugin(WasmFileLoader),
    /// Loaded via a WASM plugin that also controls rendering (file-loader + file-viewer).
    PluginWithViewer(WasmFileViewerLoader),
}

impl FileType {
    /// Returns the lightweight discriminant for this loader, suitable for
    /// storing in state or passing through events.
    pub fn kind(&self) -> FileKind {
        match self {
            FileType::Ndjson(_) => FileKind::Ndjson,
            FileType::JsonArray(_) | FileType::Single(_) => FileKind::Json,
            FileType::Plugin(_) => FileKind::Plugin,
            FileType::PluginWithViewer(_) => FileKind::PluginTable,
        }
    }

    /// Returns the number of top-level elements in the file.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        match self {
            FileType::Ndjson(f) => f.len(),
            FileType::JsonArray(f) => f.len(),
            FileType::Single(_) => 1,
            FileType::Plugin(f) => f.len(),
            FileType::PluginWithViewer(f) => f.len(),
        }
    }

    /// Get a parsed JSON value at the specified index.
    pub fn get(&mut self, idx: usize) -> Result<Value> {
        match self {
            FileType::Ndjson(f) => f.get(idx),
            FileType::JsonArray(f) => f.get(idx),
            FileType::Single(f) => f.get(idx),
            FileType::Plugin(f) => f.get(idx),
            FileType::PluginWithViewer(f) => f.get(idx),
        }
    }

    /// Read records `[start, start + count)` in one bulk, sequential pass.
    ///
    /// For plugin loaders this is a single WASM call (see the `get-range`
    /// export) — far cheaper than looping `get(idx)`, which is O(n²) for
    /// stream-parsed formats like CSV. Native loaders already offer O(1)
    /// random access, so they just loop `get`.
    pub fn get_range(&mut self, start: usize, count: usize) -> Result<Vec<Value>> {
        match self {
            FileType::Plugin(f) => f.get_range(start, count),
            FileType::PluginWithViewer(f) => f.get_range(start, count),
            other => {
                let end = start.saturating_add(count).min(other.len());
                let mut out = Vec::with_capacity(end.saturating_sub(start));
                for i in start..end {
                    out.push(other.get(i)?);
                }
                Ok(out)
            }
        }
    }

    /// Get the raw bytes for an element at the specified index.
    pub fn raw_slice(&self, idx: usize) -> Result<Vec<u8>> {
        match self {
            FileType::Ndjson(f) => f.raw_line(idx),
            FileType::JsonArray(f) => f.raw_element(idx),
            FileType::Single(f) => f.raw_all(),
            FileType::Plugin(f) => f.raw_bytes(idx),
            FileType::PluginWithViewer(f) => f.raw_bytes(idx),
        }
    }

    /// Ask the plugin how it wants its data displayed.
    /// Only available for PluginWithViewer loaders; defaults to Table.
    pub fn preferred_display(&mut self) -> DisplayMode {
        match self {
            FileType::PluginWithViewer(f) => f.preferred_display(),
            _ => DisplayMode::Table,
        }
    }

    /// Ask the plugin to render the given JSON record; returns the node_json string.
    /// Only available for PluginWithViewer loaders.
    pub fn render_record(&mut self, record_json: &str) -> Option<String> {
        match self {
            FileType::PluginWithViewer(f) => f.render_record(record_json).ok(),
            _ => None,
        }
    }

    /// Return plugin-supplied column headers. Only available for PluginWithViewer loaders.
    pub fn column_headers(&mut self) -> Option<Vec<String>> {
        match self {
            FileType::PluginWithViewer(f) => f.column_headers(),
            _ => None,
        }
    }
}

impl FileLoader for FileType {
    type Item = Value;

    fn open(path: &Path) -> Result<Self> {
        let (_detected, file_type) = load_file_auto(path)?;
        Ok(file_type)
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn get(&mut self, idx: usize) -> Result<Self::Item> {
        self.get(idx)
    }

    fn raw_bytes(&self, idx: usize) -> Result<Vec<u8>> {
        self.raw_slice(idx)
    }
}

/// Load a file with automatic format detection.
pub fn load_file_auto(path: &Path) -> Result<(DetectedFileType, FileType)> {
    use crate::file::detect_file_type::sniff_file_type;

    let detected = sniff_file_type(path)?;
    let file_type = match detected {
        DetectedFileType::Ndjson => FileType::Ndjson(NdjsonFile::open(path)?),
        DetectedFileType::JsonArray => FileType::JsonArray(JsonArrayFile::open(path)?),
        DetectedFileType::JsonObject => FileType::Single(SingleValueFile::open(path)?),
    };
    Ok((detected, file_type))
}
