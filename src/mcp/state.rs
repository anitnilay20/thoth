//! Server state — manages open files for the MCP server.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use serde_json::Value;

use crate::error::Result;
use crate::file::FileType;
use crate::file::loaders::{DuckdbConnection, FileLoader, RecordSource, RecordWindow};

/// A single file opened by the MCP server, backed by the DuckDB engine.
///
/// The window makes repeated record reads cheap: tools like `sample_records`
/// and `get_schema` walk the head of a file, and only the first read of each
/// window costs a query.
pub struct OpenFile {
    pub path: PathBuf,
    pub file_type: FileType,
    engine: DuckdbConnection,
    window: RecordWindow,
    record_count: usize,
}

impl OpenFile {
    /// Open a file at the given path with automatic format detection.
    pub fn open(path: &Path) -> Result<Self> {
        let file_type = FileType::from_path(path);
        let engine = DuckdbConnection::open_path(path)?;
        let record_count = engine.len()?;
        Ok(Self {
            path: path.to_path_buf(),
            file_type,
            engine,
            window: RecordWindow::default(),
            record_count,
        })
    }

    /// Return the number of top-level records.
    pub fn record_count(&self) -> usize {
        self.record_count
    }

    /// The record at `index`, served from the current Arrow window.
    pub fn record(&mut self, index: usize) -> Result<Value> {
        self.window.record(&self.engine, index)?.ok_or_else(|| {
            crate::error::ThothError::DatabaseQueryError {
                query: format!("row {index}"),
                reason: format!("No record at index {index}"),
            }
        })
    }

    /// Records `[start, start + count)` — one query per window.
    pub fn records(&mut self, start: usize, count: usize) -> Result<Vec<Value>> {
        self.window.records(&self.engine, start, count)
    }

    /// Column names, in schema order.
    pub fn columns(&self) -> Result<Vec<String>> {
        self.engine.column_names()
    }

    /// Run a read-only statement against this file, returning at most
    /// `max_rows` rows. The alias is the file's stem.
    ///
    /// The bound is applied to the Arrow stream, not to the JSON: converting
    /// the whole of `SELECT * FROM sales` over a multi-gigabyte file and then
    /// keeping the first hundred rows builds millions of `serde_json::Value`s
    /// in the server process, while the state mutex is held.
    pub fn query(&self, sql: &str, max_rows: usize) -> Result<Vec<Value>> {
        read_only(sql)?;
        crate::file::loaders::batches_to_values(&self.engine.collect_bounded(sql, Some(max_rows))?)
    }

    /// The alias SQL should reference this file by.
    pub fn alias(&self) -> String {
        self.engine.primary_alias().unwrap_or_default()
    }

    /// The file's format, as the tools report it.
    ///
    /// Read from [`FileType`] directly. Going through `FileKind` lost the
    /// distinctions it does not carry: it maps Excel, Arrow and databases all
    /// to `Json`, and never produces `Ndjson` at all, so the `"ndjson"` arm
    /// was unreachable and half the formats reported as `"json"`.
    pub fn type_name(&self) -> &'static str {
        let ndjson = self
            .path
            .extension()
            .and_then(|e| e.to_str())
            .is_some_and(|e| e.eq_ignore_ascii_case("ndjson") || e.eq_ignore_ascii_case("jsonl"));
        match self.file_type {
            FileType::Json if ndjson => "ndjson",
            FileType::Json | FileType::Unknown => "json",
            FileType::Csv => "csv",
            FileType::Parquet => "parquet",
            FileType::Excel => "excel",
            FileType::Arrow => "arrow",
            FileType::Avro => "avro",
            FileType::DB => "database",
            FileType::Plugin => "plugin",
        }
    }
}

/// Reject anything that is not a read.
///
/// The engine a tool runs against has the user's own file system and network
/// reach: `COPY … TO`, `ATTACH`, `INSTALL` and `CREATE` all work, and an MCP
/// client is not the user. Disabling extension autoload closes one door of
/// several, so the statement itself is checked — one statement, and it must be
/// a `SELECT`, a `WITH … SELECT`, a `DESCRIBE`, a `SHOW` or an `EXPLAIN`.
fn read_only(sql: &str) -> Result<()> {
    let refuse = |reason: &str| {
        Err(crate::error::ThothError::DatabaseQueryError {
            query: sql.to_string(),
            reason: reason.to_string(),
        })
    };

    // Strip comments and string literals before looking at the shape of it, so
    // neither a `--` nor a quoted `;` can hide a second statement.
    let mut bare = String::with_capacity(sql.len());
    let mut chars = sql.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\'' | '"' => {
                while let Some(end) = chars.next() {
                    if end == c {
                        if chars.peek() == Some(&c) {
                            chars.next();
                        } else {
                            break;
                        }
                    }
                }
                bare.push(' ');
            }
            '-' if chars.peek() == Some(&'-') => {
                for rest in chars.by_ref() {
                    if rest == '\n' {
                        break;
                    }
                }
                bare.push(' ');
            }
            '/' if chars.peek() == Some(&'*') => {
                chars.next();
                let mut last = ' ';
                for rest in chars.by_ref() {
                    if last == '*' && rest == '/' {
                        break;
                    }
                    last = rest;
                }
                bare.push(' ');
            }
            _ => bare.push(c),
        }
    }

    // One statement. A trailing `;` is ordinary; a second statement is not.
    if bare.trim_end().trim_end_matches(';').contains(';') {
        return refuse("only one statement at a time");
    }
    let first = bare
        .split_whitespace()
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    const READS: [&str; 5] = ["select", "with", "describe", "show", "explain"];
    if !READS.contains(&first.as_str()) {
        return refuse(&format!(
            "only read-only statements are allowed here ({}), not '{first}'",
            READS.join(", ")
        ));
    }
    Ok(())
}

/// Thread-safe shared state for the MCP server.
///
/// Keyed by a user-chosen handle (defaults to the file path string).
#[derive(Clone, Default)]
pub struct ServerState {
    inner: Arc<Mutex<ServerStateInner>>,
}

#[derive(Default)]
struct ServerStateInner {
    files: HashMap<String, OpenFile>,
    next_id: u64,
}

impl ServerState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Open a file and return its handle.
    pub fn open_file(&self, path: &Path) -> Result<(String, FileInfo)> {
        let open = OpenFile::open(path)?;
        let info = FileInfo {
            handle: String::new(), // filled below
            path: path.display().to_string(),
            file_type: open.type_name().to_string(),
            record_count: open.record_count(),
            alias: open.alias(),
        };

        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        inner.next_id += 1;
        let handle = format!("file_{}", inner.next_id);
        let info = FileInfo {
            handle: handle.clone(),
            ..info
        };
        inner.files.insert(handle.clone(), open);
        Ok((handle, info))
    }

    /// Close a file by handle. Returns true if the file was found and removed.
    pub fn close_file(&self, handle: &str) -> bool {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        inner.files.remove(handle).is_some()
    }

    /// Run a closure with mutable access to an open file.
    pub fn with_file<F, T>(&self, handle: &str, f: F) -> Option<T>
    where
        F: FnOnce(&mut OpenFile) -> T,
    {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        inner.files.get_mut(handle).map(f)
    }

    /// Run a closure with mutable access to an open file, returning two values atomically.
    // TODO(#53): used again once the search tool returns.
    #[allow(dead_code)]
    pub fn with_file_read2<F, A, B>(&self, handle: &str, f: F) -> Option<(A, B)>
    where
        F: FnOnce(&mut OpenFile) -> (A, B),
    {
        let mut inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        inner.files.get_mut(handle).map(f)
    }

    /// Get info about an open file.
    pub fn file_info(&self, handle: &str) -> Option<FileInfo> {
        let inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        inner.files.get(handle).map(|f| FileInfo {
            handle: handle.to_string(),
            path: f.path.display().to_string(),
            file_type: f.type_name().to_string(),
            record_count: f.record_count(),
            alias: f.alias(),
        })
    }

    /// List all open file handles.
    #[allow(dead_code)]
    pub fn list_handles(&self) -> Vec<String> {
        let inner = self
            .inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner());
        inner.files.keys().cloned().collect()
    }
}

/// Serializable file metadata returned by several tools.
#[derive(Debug, Clone, serde::Serialize)]
pub struct FileInfo {
    pub handle: String,
    pub path: String,
    pub file_type: String,
    pub record_count: usize,
    /// The name SQL should reference this file by (see the query_file tool).
    pub alias: String,
}
