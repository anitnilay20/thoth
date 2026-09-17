//! The file-loading interface (#147 / #148).
//!
//! Every locally-opened file is reached through [`FileLoader`]. The single
//! implementation is [`duck_db::DuckdbConnection`] — DuckDB is the host-side
//! query engine, so native formats (JSON/CSV/Parquet/…) are scanned directly
//! and plugin-loaded formats are ingested into it first. That keeps one SQL
//! surface for filtering (#53), sorting (#54) and aggregation (#55).
//!
//! [`FileLoader`] is the tabular half of the interface — it speaks Arrow.
//! [`RecordSource`] is the record half, used by the JSON tree viewer, the
//! search engine and the MCP tools, which all think in terms of one JSON
//! value per row.

pub mod arrow_rows;
pub mod arrow_tree;
pub mod duck_db;
pub mod text_index;

use crate::error::Result;
use crate::file::FileType;
use duckdb::arrow::array::RecordBatch;
use serde_json::Value;

pub use arrow_rows::{RecordWindow, batch_columns, batch_rows, batches_to_values};
pub use arrow_tree::{ArrowNode, NodeKind};
pub use duck_db::DuckdbConnection;
pub use text_index::TextIndex;

/// Tabular access to an opened file, in Arrow.
pub trait FileLoader {
    /// Run arbitrary SQL against the connection. Aliases registered by
    /// [`FileLoader::open`] are referenced by name.
    fn query(&self, query: &str) -> Result<Vec<RecordBatch>>;

    /// `SELECT *` over the primary alias with optional `WHERE` predicates
    /// (combined with `AND`), `LIMIT` and `OFFSET`.
    fn fetch(
        &self,
        filters: Vec<String>,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<RecordBatch>>;

    /// Size in bytes of the backing file.
    fn size(&self) -> Result<u128>;

    /// Number of rows in the primary alias.
    fn len(&self) -> Result<usize>;

    /// Whether the primary alias has no rows.
    fn is_empty(&self) -> Result<bool> {
        Ok(self.len()? == 0)
    }

    /// A single row by zero-based index.
    fn get(&self, index: usize) -> Result<RecordBatch>;

    /// Register `path` under `alias`. The first alias opened becomes the
    /// primary one that `fetch` / `len` / `get` operate on.
    fn open(&self, path: &str, alias: &str) -> Result<()>;
}

/// Record-oriented access, derived from Arrow.
///
/// Every method here is a thin conversion over [`FileLoader::fetch`] — the
/// read itself is always Arrow, and JSON is materialized only for the
/// consumers that think in records (tree viewer, search, MCP). Blanket-
/// implemented, so any `FileLoader` (including `dyn FileLoader`) gets it.
///
/// Prefer [`RecordSource::record_range`] over a loop of
/// [`RecordSource::record`] — each call is one query — or hold a
/// [`RecordWindow`] when reads are repeated, which caches the Arrow window.
pub trait RecordSource: FileLoader {
    /// Rows `[start, start + count)` as JSON values.
    fn record_range(&self, start: usize, count: usize) -> Result<Vec<Value>> {
        if count == 0 {
            return Ok(Vec::new());
        }
        batches_to_values(&self.fetch(Vec::new(), Some(start), Some(count))?)
    }

    /// The row at `index` as a JSON value.
    fn record(&self, index: usize) -> Result<Value> {
        self.record_range(index, 1)?
            .into_iter()
            .next()
            .ok_or_else(|| crate::error::ThothError::DatabaseQueryError {
                query: format!("row {index}"),
                reason: format!("No record at index {index}"),
            })
    }

    /// The row at `index` serialized to JSON bytes. Used by the search engine,
    /// which scans raw bytes before it parses anything.
    fn raw_bytes(&self, index: usize) -> Result<Vec<u8>> {
        Ok(serde_json::to_vec(&self.record(index)?)?)
    }

    /// Column names of the primary alias, in schema order. Reads the schema
    /// only — `LIMIT 0` fetches no data.
    fn column_names(&self) -> Result<Vec<String>> {
        let schema_only = batch_columns(&self.fetch(Vec::new(), None, Some(0))?);
        if !schema_only.is_empty() {
            return Ok(schema_only);
        }
        // Some readers only publish a schema alongside real rows.
        Ok(batch_columns(&self.fetch(Vec::new(), None, Some(1))?))
    }
}

impl<T: FileLoader + ?Sized> RecordSource for T {}

/// A lightweight, `Copy` tag describing what kind of file a tab holds.
///
/// Stored in window state, toolbar events and the status bar; unlike
/// [`FileType`] it also records that a *plugin* supplies the renderer, which
/// only the host knows.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FileKind {
    #[default]
    Ndjson,
    Json,
    /// Loaded through a file-loader plugin; rendered by the built-in viewer.
    Plugin,
    /// Loaded through a plugin that also supplies its own tabular renderer.
    PluginTable,
}

impl From<FileType> for FileKind {
    fn from(value: FileType) -> Self {
        match value {
            // Native formats all surface as JSON records via `to_json`, so the
            // built-in tree viewer renders them. A dedicated table viewer
            // lands with the query editor (#149).
            FileType::Json | FileType::Csv | FileType::Parquet | FileType::DB => FileKind::Json,
            FileType::Plugin => FileKind::Plugin,
            FileType::Unknown => FileKind::Json,
        }
    }
}
