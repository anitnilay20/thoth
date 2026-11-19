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

use std::sync::Arc;

use crate::error::Result;
use crate::file::FileType;
use duckdb::arrow::array::RecordBatch;
use duckdb::arrow::datatypes::DataType;
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

    /// `SELECT * FROM <relation>` for a named relation. Overridable because a
    /// DuckDB connection maps display names to quoted aliases and attached
    /// database tables.
    fn relation_select_sql(&self, relation: &str) -> Result<String> {
        Ok(format!("SELECT * FROM {relation}"))
    }

    /// Rows `[offset, offset + limit)` of a named relation.
    ///
    /// The primary-alias methods cannot address a document's other collections
    /// or a database's other tables; the data bus needs one handle per
    /// relation, so this is the loader-level seam it publishes.
    fn relation_fetch(
        &self,
        relation: &str,
        filters: Vec<String>,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<RecordBatch>> {
        let mut sql = self.relation_select_sql(relation)?;
        if !filters.is_empty() {
            sql.push_str(" WHERE ");
            sql.push_str(
                &filters
                    .iter()
                    .map(|f| format!("({f})"))
                    .collect::<Vec<_>>()
                    .join(" AND "),
            );
        }
        if let Some(limit) = limit {
            sql.push_str(&format!(" LIMIT {limit}"));
        }
        if let Some(offset) = offset {
            // DuckDB requires a LIMIT before OFFSET.
            if limit.is_none() {
                sql.push_str(" LIMIT ALL");
            }
            sql.push_str(&format!(" OFFSET {offset}"));
        }
        self.query(&sql)
    }

    /// Row count of a named relation.
    fn relation_len(&self, relation: &str) -> Result<usize> {
        use duckdb::arrow::array::{Array, Int64Array};
        let sql = self.relation_select_sql(relation)?;
        let batches = self.query(&format!("SELECT count(*) AS n FROM ({sql}) t"))?;
        let Some(batch) = batches.first() else {
            return Ok(0);
        };
        let Some(count) = batch.column(0).as_any().downcast_ref::<Int64Array>() else {
            return Ok(0);
        };
        Ok(count.value(0).max(0) as usize)
    }
}

/// A [`FileLoader`] scoped to one named relation of another loader.
///
/// A document can hold several collections and a database several tables. The
/// registry gives each its own handle, so this redirects the primary-alias
/// methods (`fetch`/`len`/`get`) at the wrapped relation.
pub struct RelationLoader {
    inner: Arc<dyn FileLoader + Send + Sync>,
    relation: String,
}

impl RelationLoader {
    /// Wrap `inner` so its primary-alias methods address `relation`.
    pub fn new(inner: Arc<dyn FileLoader + Send + Sync>, relation: String) -> Self {
        Self { inner, relation }
    }
}

impl FileLoader for RelationLoader {
    fn query(&self, query: &str) -> Result<Vec<RecordBatch>> {
        self.inner.query(query)
    }

    fn fetch(
        &self,
        filters: Vec<String>,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<RecordBatch>> {
        self.inner
            .relation_fetch(&self.relation, filters, offset, limit)
    }

    fn size(&self) -> Result<u128> {
        self.inner.size()
    }

    fn len(&self) -> Result<usize> {
        self.inner.relation_len(&self.relation)
    }

    fn get(&self, index: usize) -> Result<RecordBatch> {
        self.fetch(Vec::new(), Some(index), Some(1))?
            .into_iter()
            .next()
            .ok_or_else(|| crate::error::ThothError::DatabaseQueryError {
                query: format!("relation {} row {index}", self.relation),
                reason: "No record at index".to_string(),
            })
    }

    fn open(&self, path: &str, alias: &str) -> Result<()> {
        self.inner.open(path, alias)
    }

    fn relation_select_sql(&self, relation: &str) -> Result<String> {
        self.inner.relation_select_sql(relation)
    }
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

    /// Column names and SQL-ish type hints of the primary alias, in schema
    /// order. Reads the Arrow schema only — the type hints are what the data
    /// bus, grid and chart axis pickers use to tell numbers from text.
    fn columns(&self) -> Result<Vec<(String, String)>> {
        let mut batches = self.fetch(Vec::new(), None, Some(0))?;
        if batches.iter().all(|batch| batch.num_columns() == 0) {
            // Some readers only publish a schema alongside real rows.
            batches = self.fetch(Vec::new(), None, Some(1))?;
        }
        let Some(batch) = batches.first() else {
            return Ok(Vec::new());
        };
        Ok(batch
            .schema()
            .fields()
            .iter()
            .map(|field| (field.name().clone(), arrow_type_hint(field.data_type())))
            .collect())
    }
}

impl<T: FileLoader + ?Sized> RecordSource for T {}

/// Map an Arrow type to the SQL-ish hint the SDK's `ColumnType::from_sql`
/// understands (alignment, colour, chart axis treatment).
fn arrow_type_hint(data_type: &DataType) -> String {
    let hint = match data_type {
        DataType::Boolean => "boolean",
        DataType::Int8
        | DataType::Int16
        | DataType::Int32
        | DataType::Int64
        | DataType::UInt8
        | DataType::UInt16
        | DataType::UInt32
        | DataType::UInt64 => "integer",
        DataType::Float16 | DataType::Float32 | DataType::Float64 => "float",
        DataType::Decimal32(..)
        | DataType::Decimal64(..)
        | DataType::Decimal128(..)
        | DataType::Decimal256(..) => "decimal",
        DataType::Date32 | DataType::Date64 => "date",
        DataType::Time32(..) | DataType::Time64(..) => "time",
        DataType::Timestamp(..) => "timestamp",
        DataType::Duration(..) | DataType::Interval(..) => "interval",
        DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View => "text",
        DataType::Binary
        | DataType::LargeBinary
        | DataType::BinaryView
        | DataType::FixedSizeBinary(..) => "binary",
        DataType::List(..)
        | DataType::LargeList(..)
        | DataType::ListView(..)
        | DataType::LargeListView(..)
        | DataType::FixedSizeList(..)
        | DataType::Struct(..)
        | DataType::Map(..)
        | DataType::Union(..)
        | DataType::Dictionary(..)
        | DataType::RunEndEncoded(..) => "json",
        DataType::Null => "null",
    };
    hint.to_string()
}

/// An in-memory [`FileLoader`] over Arrow record batches.
///
/// The plugin `dataset-bus.publish-arrow` path lands here: the host parses the
/// IPC stream once and then serves windows without another copy.
pub struct BatchLoader {
    batches: Vec<RecordBatch>,
}

impl BatchLoader {
    /// Wrap a batch sequence as a loader.
    pub fn new(batches: Vec<RecordBatch>) -> Self {
        Self { batches }
    }
}

impl FileLoader for BatchLoader {
    fn query(&self, query: &str) -> Result<Vec<RecordBatch>> {
        Err(crate::error::ThothError::DatabaseQueryError {
            query: query.to_string(),
            reason: "in-memory Arrow datasets are not queryable".to_string(),
        })
    }

    fn fetch(
        &self,
        _filters: Vec<String>,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<RecordBatch>> {
        let start = offset.unwrap_or(0);
        let end = limit.map_or(usize::MAX, |limit| start.saturating_add(limit));
        let mut out = Vec::new();
        let mut seen = 0usize;
        for batch in &self.batches {
            let rows = batch.num_rows();
            let batch_start = seen;
            let batch_end = seen + rows;
            seen = batch_end;
            if batch_end <= start {
                continue;
            }
            if batch_start >= end {
                break;
            }
            let from = start.saturating_sub(batch_start);
            let to = (end - batch_start).min(rows);
            if from < to {
                out.push(batch.slice(from, to - from));
            }
        }
        Ok(out)
    }

    fn size(&self) -> Result<u128> {
        Ok(0)
    }

    fn len(&self) -> Result<usize> {
        Ok(self.batches.iter().map(RecordBatch::num_rows).sum())
    }

    fn get(&self, index: usize) -> Result<RecordBatch> {
        self.fetch(Vec::new(), Some(index), Some(1))?
            .into_iter()
            .next()
            .ok_or_else(|| crate::error::ThothError::DatabaseQueryError {
                query: format!("row {index}"),
                reason: "No record at index".to_string(),
            })
    }

    fn open(&self, _path: &str, _alias: &str) -> Result<()> {
        Err(crate::error::ThothError::DatabaseError {
            reason: "in-memory Arrow datasets cannot open files".to_string(),
        })
    }
}

/// Parse an Arrow IPC stream into record batches.
pub fn parse_ipc(bytes: &[u8]) -> Result<Vec<RecordBatch>> {
    use duckdb::arrow::ipc::reader::StreamReader;

    let invalid = |e: &dyn std::fmt::Display| crate::error::ThothError::Unknown {
        message: format!("invalid Arrow IPC: {e}"),
    };
    let reader =
        StreamReader::try_new(std::io::Cursor::new(bytes), None).map_err(|e| invalid(&e))?;
    reader
        .collect::<std::result::Result<Vec<_>, _>>()
        .map_err(|e| invalid(&e))
}

/// Encode record batches as an Arrow IPC stream.
pub fn encode_ipc(batches: &[RecordBatch]) -> Result<Vec<u8>> {
    use duckdb::arrow::ipc::writer::StreamWriter;

    let Some(first) = batches.first() else {
        return Ok(Vec::new());
    };
    let schema = first.schema();
    let invalid = |e: &dyn std::fmt::Display| crate::error::ThothError::Unknown {
        message: format!("failed to encode Arrow IPC: {e}"),
    };

    let mut bytes = Vec::new();
    {
        let mut writer =
            StreamWriter::try_new(&mut bytes, schema.as_ref()).map_err(|e| invalid(&e))?;
        for batch in batches {
            writer.write(batch).map_err(|e| invalid(&e))?;
        }
        writer.finish().map_err(|e| invalid(&e))?;
    }
    Ok(bytes)
}

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
            FileType::Json
            | FileType::Csv
            | FileType::Parquet
            | FileType::Excel
            | FileType::Arrow
            | FileType::Avro
            | FileType::DB => FileKind::Json,
            FileType::Plugin => FileKind::Plugin,
            FileType::Unknown => FileKind::Json,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::arrow_type_hint;
    use duckdb::arrow::datatypes::{DataType, Field, TimeUnit};
    use std::sync::Arc;

    #[test]
    fn arrow_types_map_to_sql_hints() {
        let cases = [
            (DataType::Boolean, "boolean"),
            (DataType::Int64, "integer"),
            (DataType::UInt32, "integer"),
            (DataType::Float64, "float"),
            (DataType::Decimal128(10, 2), "decimal"),
            (DataType::Utf8, "text"),
            (DataType::LargeUtf8, "text"),
            (DataType::Date32, "date"),
            (
                DataType::Timestamp(TimeUnit::Millisecond, None),
                "timestamp",
            ),
            (DataType::Time64(TimeUnit::Microsecond), "time"),
            (DataType::Duration(TimeUnit::Second), "interval"),
            (DataType::Binary, "binary"),
            (DataType::Null, "null"),
        ];
        for (data_type, expected) in cases {
            assert_eq!(arrow_type_hint(&data_type), expected, "{data_type:?}");
        }

        let list = DataType::List(Arc::new(Field::new("item", DataType::Int64, true)));
        assert_eq!(arrow_type_hint(&list), "json");
    }
}
