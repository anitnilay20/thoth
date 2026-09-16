//! DuckDB-backed file query engine (#148).
//!
//! One [`DuckdbConnection`] owns one in-memory DuckDB connection and the
//! aliases registered against it. Native formats are exposed as *views* over
//! DuckDB's own readers (`read_json_auto`, `read_csv_auto`, `read_parquet`),
//! so a multi-gigabyte file is never fully materialized — DuckDB scans only
//! what a query touches.
//!
//! Plugin-loaded formats have no DuckDB reader, so their records are pulled
//! through the file-loader plugin once and staged as NDJSON in a temp file
//! that DuckDB then reads like any other JSON source. That makes plugin
//! formats queryable with the same SQL as everything else.

use std::collections::HashMap;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use duckdb::{Connection, arrow::array::RecordBatch};
use tempfile::NamedTempFile;

use crate::error::{Result, ThothError};
use crate::file::FileType;
use crate::file::json_envelope::Collection;
use crate::file::loaders::FileLoader;

/// Records pulled per `get_range` call when staging a plugin-loaded file.
const STAGE_CHUNK: usize = 2048;

/// Alias the on-disk collection cache is attached under.
const CACHE_DB: &str = "thoth_cache";

/// Table inside that cache recording which file version it describes.
const STAMP_TABLE: &str = "__thoth_identity";

/// A file registered on the connection.
struct Source {
    alias: String,
    /// The file the user opened (not the staged copy, if any).
    path: PathBuf,
    /// Staged NDJSON for plugin-loaded formats. Held so the temp file
    /// outlives the view that reads it.
    staged: Option<NamedTempFile>,
}

impl Source {
    /// The file DuckDB actually scans.
    fn scan_path(&self) -> &Path {
        match &self.staged {
            Some(tmp) => tmp.path(),
            None => &self.path,
        }
    }
}

pub struct DuckdbConnection {
    conn: Mutex<Connection>,
    /// Every alias registered, in `open` order. The first is the primary one
    /// that `fetch` / `len` / `get` address.
    sources: Mutex<Vec<Source>>,
    /// `len()` is a `count(*)` over the whole file — worth caching.
    row_count: Mutex<Option<usize>>,
    /// Collections extracted from an envelope document, held so their files
    /// outlive the views reading them.
    staged: Mutex<HashMap<String, NamedTempFile>>,
}

impl DuckdbConnection {
    pub fn new() -> Result<Self> {
        Ok(Self {
            conn: Mutex::new(Connection::open_in_memory()?),
            sources: Mutex::new(Vec::new()),
            row_count: Mutex::new(None),
            staged: Mutex::new(HashMap::new()),
        })
    }

    /// Open a single file, deriving its alias from the file stem.
    pub fn open_path(path: &Path) -> Result<Self> {
        let db = Self::new()?;
        db.open(&path.to_string_lossy(), &alias_for(path))?;
        Ok(db)
    }

    /// The alias `fetch` / `len` / `get` operate on.
    pub fn primary_alias(&self) -> Option<String> {
        self.with_sources(|s| s.first().map(|s| s.alias.clone()))
    }

    /// The path of the primary source.
    pub fn primary_path(&self) -> Option<PathBuf> {
        self.with_sources(|s| s.first().map(|s| s.path.clone()))
    }

    fn with_sources<T>(&self, f: impl FnOnce(&[Source]) -> T) -> T {
        let guard = self.sources.lock().unwrap_or_else(|e| e.into_inner());
        f(&guard)
    }

    /// The quoted primary alias, or an error when nothing is open yet.
    fn primary(&self) -> Result<String> {
        self.primary_alias()
            .map(|a| quote_ident(&a))
            .ok_or_else(|| ThothError::DatabaseError {
                reason: "No file is open on this connection".to_string(),
            })
    }

    /// Run `sql` and collect every Arrow batch it produces.
    fn collect(&self, sql: &str) -> Result<Vec<RecordBatch>> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn.prepare(sql).map_err(|e| ThothError::DatabaseQueryError {
            query: sql.to_string(),
            reason: e.to_string(),
        })?;
        let batches = stmt
            .stream_arrow([])
            .map_err(|e| ThothError::DatabaseQueryError {
                query: sql.to_string(),
                reason: e.to_string(),
            })?
            .collect();
        Ok(batches)
    }

    fn execute(&self, sql: &str) -> Result<()> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.execute_batch(sql)
            .map_err(|e| ThothError::DatabaseQueryError {
                query: sql.to_string(),
                reason: e.to_string(),
            })
    }

    /// Register a native format as a view over the matching DuckDB reader.
    fn register_scan(&self, alias: &str, file_type: FileType, scan_path: &Path) -> Result<()> {
        let literal = quote_literal(&scan_path.to_string_lossy());
        let reader = match file_type {
            FileType::Json | FileType::Plugin | FileType::Unknown => {
                // DuckDB's default object limit is left alone deliberately. A
                // document too large to read as one value is an envelope, and
                // raising the limit only buys a multi-gigabyte parse before the
                // same failure -- see `json_envelope`.
                format!("read_json_auto({literal})")
            }
            FileType::Csv => format!("read_csv_auto({literal})"),
            FileType::Parquet => format!("read_parquet({literal})"),
            FileType::DB => return self.register_database(alias, scan_path),
        };
        self.execute(&format!(
            "CREATE OR REPLACE VIEW {} AS SELECT * FROM {reader}",
            quote_ident(alias)
        ))
    }

    /// Attach a SQLite/DuckDB database and point the alias at its first table.
    fn register_database(&self, alias: &str, path: &Path) -> Result<()> {
        if is_sqlite(path) {
            // Best effort — a sandboxed or offline host may not be able to
            // fetch the extension, in which case ATTACH reports the problem.
            let _ = self.execute("INSTALL sqlite; LOAD sqlite;");
        }
        let db_alias = format!("{alias}__db");
        self.execute(&format!(
            "ATTACH IF NOT EXISTS {} AS {} (READ_ONLY)",
            quote_literal(&path.to_string_lossy()),
            quote_ident(&db_alias)
        ))?;

        let table: String = {
            let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
            conn.query_row(
                "SELECT table_name FROM duckdb_tables() WHERE database_name = ? \
                 ORDER BY table_name LIMIT 1",
                [&db_alias],
                |row| row.get(0),
            )
            .map_err(|_| ThothError::InvalidFileType {
                path: path.to_path_buf(),
                expected: "a database containing at least one table".to_string(),
            })?
        };

        self.execute(&format!(
            "CREATE OR REPLACE VIEW {} AS SELECT * FROM {}.{}",
            quote_ident(alias),
            quote_ident(&db_alias),
            quote_ident(&table)
        ))
    }

    /// Attach a cache database, so a document's collections become tables
    /// without re-reading the document.
    ///
    /// Marker table aside, every table found is exposed as a plain view, so
    /// `SELECT * FROM users` works whether the collections were just ingested
    /// or restored from a previous session.
    pub fn attach_cache(&self, db_path: &Path) -> Result<Vec<String>> {
        self.execute(&format!(
            "ATTACH IF NOT EXISTS {} AS {}",
            quote_literal(&db_path.to_string_lossy()),
            quote_ident(CACHE_DB)
        ))?;

        let tables: Vec<String> = {
            let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
            let mut stmt = conn
                .prepare(
                    "SELECT table_name FROM duckdb_tables() \
                     WHERE database_name = ? ORDER BY table_name",
                )
                .map_err(|e| ThothError::DatabaseError {
                    reason: e.to_string(),
                })?;
            let rows = stmt
                .query_map([CACHE_DB], |row| row.get::<_, String>(0))
                .map_err(|e| ThothError::DatabaseError {
                    reason: e.to_string(),
                })?;
            rows.flatten().filter(|t| t != STAMP_TABLE).collect()
        };

        for table in &tables {
            self.execute(&format!(
                "CREATE OR REPLACE VIEW {} AS SELECT * FROM {}.{}",
                quote_ident(table),
                quote_ident(CACHE_DB),
                quote_ident(table)
            ))?;
        }
        let mut sources = self.sources.lock().unwrap_or_else(|e| e.into_inner());
        if sources.is_empty()
            && let Some(first) = tables.first()
        {
            sources.push(Source {
                alias: first.clone(),
                path: db_path.to_path_buf(),
                staged: None,
            });
        }
        Ok(tables)
    }

    /// The identity recorded in an attached cache, if it has one.
    pub fn cached_identity(&self) -> Option<(u64, i64, String)> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.query_row(
            &format!(
                "SELECT size, mtime, fingerprint FROM {}.{}",
                quote_ident(CACHE_DB),
                quote_ident(STAMP_TABLE)
            ),
            [],
            |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
        )
        .ok()
    }

    /// Record which file version an attached cache describes.
    pub fn record_identity(&self, size: u64, mtime: i64, fingerprint: &str) -> Result<()> {
        self.execute(&format!(
            "CREATE OR REPLACE TABLE {}.{} AS SELECT {} AS size, {} AS mtime, {} AS fingerprint",
            quote_ident(CACHE_DB),
            quote_ident(STAMP_TABLE),
            size,
            mtime,
            quote_literal(fingerprint)
        ))
    }

    /// Ingest one collection into the attached cache as a real table.
    ///
    /// The extracted JSON is temporary; what persists is columnar storage,
    /// which is both smaller than the text and far quicker to query, since a
    /// later read is a table scan rather than a re-parse.
    pub fn ingest_collection(&self, path: &Path, collection: &Collection) -> Result<()> {
        let alias = alias_for_name(&collection.name);
        let extracted = extract_range(path, collection.start, collection.end)?;
        self.execute(&format!(
            "CREATE OR REPLACE TABLE {}.{} AS SELECT * FROM read_json_auto({}, format='array')",
            quote_ident(CACHE_DB),
            quote_ident(&alias),
            quote_literal(&extracted.path().to_string_lossy())
        ))?;
        self.execute(&format!(
            "CREATE OR REPLACE VIEW {} AS SELECT * FROM {}.{}",
            quote_ident(&alias),
            quote_ident(CACHE_DB),
            quote_ident(&alias)
        ))?;

        let mut sources = self.sources.lock().unwrap_or_else(|e| e.into_inner());
        if sources.is_empty() {
            sources.push(Source {
                alias,
                path: path.to_path_buf(),
                staged: None,
            });
            *self.row_count.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }
        Ok(())
    }

    /// Register one collection of an envelope document as a queryable table.
    ///
    /// The collection's bytes are copied out to their own file and handed to
    /// DuckDB's JSON reader. That copy is the price of making a nested array
    /// queryable at all — DuckDB cannot read a byte range of a file, and
    /// parsing the enclosing document would cost the whole 2GB rather than this
    /// collection's share.
    ///
    /// Idempotent: a collection already staged is left alone, so this can be
    /// called freely before a query.
    pub fn stage_collection(&self, path: &Path, collection: &Collection) -> Result<()> {
        let alias = alias_for_name(&collection.name);
        {
            let staged = self.staged.lock().unwrap_or_else(|e| e.into_inner());
            if staged.contains_key(&alias) {
                return Ok(());
            }
        }

        let extracted = extract_range(path, collection.start, collection.end)?;
        // The value is a JSON array of records, which is exactly what the
        // `array` reader expects.
        self.execute(&format!(
            "CREATE OR REPLACE VIEW {} AS SELECT * FROM read_json_auto({}, format='array')",
            quote_ident(&alias),
            quote_literal(&extracted.path().to_string_lossy())
        ))?;

        self.staged
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(alias.clone(), extracted);

        // The first collection staged becomes the primary one, so `fetch` and
        // `len` address it without the caller naming it.
        let mut sources = self.sources.lock().unwrap_or_else(|e| e.into_inner());
        if sources.is_empty() {
            sources.push(Source {
                alias,
                path: path.to_path_buf(),
                staged: None,
            });
            *self.row_count.lock().unwrap_or_else(|e| e.into_inner()) = None;
        }
        Ok(())
    }

    /// Collections already staged, by the alias SQL refers to them by.
    pub fn staged_collections(&self) -> Vec<String> {
        let staged = self.staged.lock().unwrap_or_else(|e| e.into_inner());
        let mut names: Vec<String> = staged.keys().cloned().collect();
        names.sort();
        names
    }
}

/// Copy `[start, end)` of `path` into a file of its own.
///
/// Streamed rather than read whole: a collection can be hundreds of megabytes,
/// and there is no reason for it to be resident.
fn extract_range(path: &Path, start: u64, end: u64) -> Result<NamedTempFile> {
    let mut source = std::fs::File::open(path).map_err(|e| ThothError::FileReadError {
        path: path.to_path_buf(),
        reason: e.to_string(),
    })?;
    source
        .seek(SeekFrom::Start(start))
        .map_err(|e| ThothError::FileReadError {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;

    let mut out = tempfile::Builder::new()
        .suffix(".json")
        .tempfile()
        .map_err(|e| ThothError::FileWriteError {
            path: path.to_path_buf(),
            reason: e.to_string(),
        })?;

    let mut remaining = end.saturating_sub(start);
    let mut buf = vec![0u8; 1 << 20];
    while remaining > 0 {
        let want = (buf.len() as u64).min(remaining) as usize;
        let read = source
            .read(&mut buf[..want])
            .map_err(|e| ThothError::FileReadError {
                path: path.to_path_buf(),
                reason: e.to_string(),
            })?;
        if read == 0 {
            break;
        }
        out.write_all(&buf[..read])
            .map_err(|e| ThothError::FileWriteError {
                path: out.path().to_path_buf(),
                reason: e.to_string(),
            })?;
        remaining -= read as u64;
    }
    out.flush().map_err(|e| ThothError::FileWriteError {
        path: out.path().to_path_buf(),
        reason: e.to_string(),
    })?;
    Ok(out)
}

/// A collection's key as an SQL-safe identifier.
fn alias_for_name(name: &str) -> String {
    let sanitized: String = name
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    if sanitized.is_empty() {
        "collection".to_string()
    } else if sanitized.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{sanitized}")
    } else {
        sanitized
    }
}

impl FileLoader for DuckdbConnection {
    fn open(&self, path: &str, alias: &str) -> Result<()> {
        let path = Path::new(path);
        if !path.exists() {
            return Err(ThothError::FileNotFound {
                path: path.to_path_buf(),
            });
        }

        let file_type = FileType::from_path(path);
        // Formats DuckDB cannot read natively come through a plugin, staged as
        // NDJSON so DuckDB can scan them like any other JSON source.
        let staged = match file_type {
            FileType::Plugin | FileType::Unknown => Some(stage_via_plugin(path)?),
            _ => None,
        };

        let source = Source {
            alias: alias.to_string(),
            path: path.to_path_buf(),
            staged,
        };
        let scan_type = if source.staged.is_some() {
            FileType::Json
        } else {
            file_type
        };
        self.register_scan(alias, scan_type, source.scan_path())?;

        let mut sources = self.sources.lock().unwrap_or_else(|e| e.into_inner());
        // Re-opening the same alias replaces it rather than shadowing it.
        if let Some(existing) = sources.iter_mut().find(|s| s.alias == alias) {
            *existing = source;
        } else {
            sources.push(source);
        }
        *self.row_count.lock().unwrap_or_else(|e| e.into_inner()) = None;
        Ok(())
    }

    fn query(&self, query: &str) -> Result<Vec<RecordBatch>> {
        self.collect(query)
    }

    fn fetch(
        &self,
        filters: Vec<String>,
        offset: Option<usize>,
        limit: Option<usize>,
    ) -> Result<Vec<RecordBatch>> {
        let mut sql = format!("SELECT * FROM {}", self.primary()?);
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
        self.collect(&sql)
    }

    fn size(&self) -> Result<u128> {
        let path = self
            .with_sources(|s| s.first().map(|s| s.scan_path().to_path_buf()))
            .ok_or_else(|| ThothError::DatabaseError {
                reason: "No file is open on this connection".to_string(),
            })?;
        let meta = std::fs::metadata(&path).map_err(|e| ThothError::FileReadError {
            path: path.clone(),
            reason: e.to_string(),
        })?;
        Ok(meta.len() as u128)
    }

    fn len(&self) -> Result<usize> {
        if let Some(cached) = *self.row_count.lock().unwrap_or_else(|e| e.into_inner()) {
            return Ok(cached);
        }
        let alias = self.primary()?;
        let count: i64 = {
            let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
            conn.query_row(&format!("SELECT count(*) FROM {alias}"), [], |row| {
                row.get(0)
            })?
        };
        let count = count.max(0) as usize;
        *self.row_count.lock().unwrap_or_else(|e| e.into_inner()) = Some(count);
        Ok(count)
    }

    fn get(&self, index: usize) -> Result<RecordBatch> {
        let batches = self.fetch(Vec::new(), Some(index), Some(1))?;
        batches
            .into_iter()
            .find(|b| b.num_rows() > 0)
            .ok_or_else(|| ThothError::DatabaseQueryError {
                query: format!("row {index}"),
                reason: format!("No record at index {index}"),
            })
    }
}

/// Pull every record out of a file-loader plugin and write it as NDJSON.
///
/// This is the one place plugin formats cross into DuckDB. It is eager by
/// necessity — the WIT interface exposes indexed reads, not a scannable
/// stream — so the crossing happens once, at open time.
fn stage_via_plugin(path: &Path) -> Result<NamedTempFile> {
    use std::io::Write;

    let ext = path
        .extension()
        .map(|e| e.to_string_lossy().to_lowercase())
        .unwrap_or_default();

    let manager =
        crate::plugin::runtime::active_manager().ok_or_else(|| ThothError::InvalidFileType {
            path: path.to_path_buf(),
            expected: "a natively supported format (plugins are not loaded)".to_string(),
        })?;
    if manager.find_loader_for_extension(&ext).is_none() {
        return Err(ThothError::InvalidFileType {
            path: path.to_path_buf(),
            expected: format!(
                "a natively supported format, or an installed plugin for .{ext} files"
            ),
        });
    }

    let mut loader = manager.open_file(&ext, path)?;
    let total = loader.len();

    let mut tmp = NamedTempFile::new()?;
    let mut start = 0;
    while start < total {
        let count = STAGE_CHUNK.min(total - start);
        let chunk = loader.get_range(start, count)?;
        if chunk.is_empty() {
            break; // plugin ended early
        }
        let read = chunk.len();
        for value in chunk {
            writeln!(tmp, "{value}").map_err(|e| ThothError::FileWriteError {
                path: tmp.path().to_path_buf(),
                reason: e.to_string(),
            })?;
        }
        start += read;
    }
    tmp.flush().map_err(|e| ThothError::FileWriteError {
        path: tmp.path().to_path_buf(),
        reason: e.to_string(),
    })?;
    Ok(tmp)
}

/// Default alias for a path: its file stem, sanitized to an SQL-safe word.
pub fn alias_for(path: &Path) -> String {
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .unwrap_or_else(|| "data".to_string());
    let sanitized: String = stem
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '_' })
        .collect();
    if sanitized.starts_with(|c: char| c.is_ascii_digit()) {
        format!("_{sanitized}")
    } else if sanitized.is_empty() {
        "data".to_string()
    } else {
        sanitized
    }
}

fn quote_ident(ident: &str) -> String {
    format!("\"{}\"", ident.replace('"', "\"\""))
}

fn quote_literal(value: &str) -> String {
    format!("'{}'", value.replace('\'', "''"))
}

/// SQLite files start with a fixed 16-byte header; DuckDB files do not.
fn is_sqlite(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut head = [0u8; 16];
    file.read_exact(&mut head).is_ok() && &head == b"SQLite format 3\0"
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file::loaders::RecordSource;
    use std::io::Write;

    fn ndjson_file(lines: &str) -> NamedTempFile {
        let mut tmp = tempfile::Builder::new().suffix(".ndjson").tempfile().unwrap();
        tmp.write_all(lines.as_bytes()).unwrap();
        tmp.flush().unwrap();
        tmp
    }

    fn csv_file(contents: &str) -> NamedTempFile {
        let mut tmp = tempfile::Builder::new().suffix(".csv").tempfile().unwrap();
        tmp.write_all(contents.as_bytes()).unwrap();
        tmp.flush().unwrap();
        tmp
    }

    #[test]
    fn open_then_query_share_one_connection() {
        let file = ndjson_file("{\"a\":1}\n{\"a\":2}\n");
        let db = DuckdbConnection::new().unwrap();
        db.open(&file.path().to_string_lossy(), "rows").unwrap();

        // The alias registered by `open` must still exist for `query`.
        let batches = db.query("SELECT sum(a) AS total FROM rows").unwrap();
        assert_eq!(batches.iter().map(|b| b.num_rows()).sum::<usize>(), 1);
    }

    #[test]
    fn len_counts_rows_and_caches() {
        let file = ndjson_file("{\"a\":1}\n{\"a\":2}\n{\"a\":3}\n");
        let db = DuckdbConnection::open_path(file.path()).unwrap();
        assert_eq!(db.len().unwrap(), 3);
        assert_eq!(db.len().unwrap(), 3);
        assert!(!db.is_empty().unwrap());
    }

    #[test]
    fn fetch_applies_filters_limit_and_offset() {
        let file = ndjson_file("{\"a\":1}\n{\"a\":2}\n{\"a\":3}\n{\"a\":4}\n");
        let db = DuckdbConnection::open_path(file.path()).unwrap();

        let rows: usize = db
            .fetch(vec!["a > 1".to_string()], None, None)
            .unwrap()
            .iter()
            .map(|b| b.num_rows())
            .sum();
        assert_eq!(rows, 3);

        // OFFSET without LIMIT must still be valid SQL.
        let rows: usize = db
            .fetch(Vec::new(), Some(2), None)
            .unwrap()
            .iter()
            .map(|b| b.num_rows())
            .sum();
        assert_eq!(rows, 2);
    }

    #[test]
    fn records_round_trip_through_arrow() {
        let file = ndjson_file("{\"a\":1,\"b\":\"x\"}\n{\"a\":2,\"b\":\"y\"}\n");
        let db = DuckdbConnection::open_path(file.path()).unwrap();

        let first = db.record(0).unwrap();
        assert_eq!(first["a"], 1);
        assert_eq!(first["b"], "x");

        let all = db.record_range(0, 10).unwrap();
        assert_eq!(all.len(), 2);
        assert_eq!(all[1]["b"], "y");

        let bytes = db.raw_bytes(1).unwrap();
        assert!(String::from_utf8(bytes).unwrap().contains("\"y\""));
    }

    #[test]
    fn csv_is_read_natively_and_exposed_as_records() {
        let file = csv_file("name,age\nada,36\nlinus,54\n");
        let db = DuckdbConnection::open_path(file.path()).unwrap();

        assert_eq!(db.len().unwrap(), 2);
        assert_eq!(db.column_names().unwrap(), vec!["name", "age"]);
        assert_eq!(db.record(0).unwrap()["name"], "ada");
    }

    #[test]
    fn get_out_of_range_is_an_error() {
        let file = ndjson_file("{\"a\":1}\n");
        let db = DuckdbConnection::open_path(file.path()).unwrap();
        assert!(db.get(5).is_err());
    }

    #[test]
    fn multiple_aliases_can_be_joined() {
        let a = ndjson_file("{\"id\":1,\"v\":\"a\"}\n");
        let b = ndjson_file("{\"id\":1,\"w\":\"b\"}\n");
        let db = DuckdbConnection::new().unwrap();
        db.open(&a.path().to_string_lossy(), "a").unwrap();
        db.open(&b.path().to_string_lossy(), "b").unwrap();

        let rows: usize = db
            .query("SELECT * FROM a JOIN b USING(id)")
            .unwrap()
            .iter()
            .map(|batch| batch.num_rows())
            .sum();
        assert_eq!(rows, 1);
        // The first alias opened stays primary.
        assert_eq!(db.primary_alias().as_deref(), Some("a"));
    }

    #[test]
    fn alias_defaults_to_a_sql_safe_stem() {
        assert_eq!(alias_for(Path::new("/tmp/sales-2024.parquet")), "sales_2024");
        assert_eq!(alias_for(Path::new("/tmp/2024.csv")), "_2024");
    }

    #[test]
    fn opening_a_missing_file_reports_not_found() {
        let db = DuckdbConnection::new().unwrap();
        assert!(matches!(
            db.open("/definitely/not/here.json", "x"),
            Err(ThothError::FileNotFound { .. })
        ));
    }

    // ── Envelope collections ────────────────────────────────────────────────

    fn envelope_doc(body: &str) -> NamedTempFile {
        let mut tmp = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
        tmp.write_all(body.as_bytes()).unwrap();
        tmp.flush().unwrap();
        tmp
    }

    #[test]
    fn a_collection_inside_an_envelope_becomes_a_table() {
        use crate::file::json_envelope::JsonEnvelope;

        let file = envelope_doc(
            r#"{"meta":{"v":1},"users":[{"id":1,"name":"ada"},{"id":2,"name":"linus"}]}"#,
        );
        let env = JsonEnvelope::scan(file.path()).unwrap().unwrap();
        let users = env.get("users").unwrap();

        let db = DuckdbConnection::new().unwrap();
        db.stage_collection(file.path(), users).unwrap();

        assert_eq!(db.staged_collections(), ["users"]);
        // It is a real table: the enclosing document is not involved.
        let rows = batch_rows_of(&db.query("SELECT name FROM users ORDER BY id").unwrap());
        assert_eq!(rows, 2);
        assert_eq!(db.len().unwrap(), 2, "the first staged collection is primary");
    }

    #[test]
    fn collections_from_one_document_can_be_joined() {
        // The whole point: `users JOIN transactions` as ordinary SQL, over a
        // document DuckDB cannot read as a table at all.
        use crate::file::json_envelope::JsonEnvelope;

        let file = envelope_doc(
            r#"{"users":[{"id":1,"name":"ada"},{"id":2,"name":"linus"}],
                "transactions":[{"user_id":1,"amount":10},{"user_id":1,"amount":5},
                                {"user_id":2,"amount":7}]}"#,
        );
        let env = JsonEnvelope::scan(file.path()).unwrap().unwrap();

        let db = DuckdbConnection::new().unwrap();
        for c in env.queryable() {
            db.stage_collection(file.path(), c).unwrap();
        }
        assert_eq!(db.staged_collections(), ["transactions", "users"]);

        let batches = db
            .query(
                "SELECT u.name, sum(t.amount) AS total \
                 FROM users u JOIN transactions t ON t.user_id = u.id \
                 GROUP BY u.name ORDER BY total DESC",
            )
            .unwrap();
        let rows = crate::file::loaders::batches_to_values(&batches).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["name"], "ada");
        assert_eq!(rows[0]["total"], 15);
        assert_eq!(rows[1]["name"], "linus");
        assert_eq!(rows[1]["total"], 7);
    }

    #[test]
    fn staging_the_same_collection_twice_is_a_no_op() {
        use crate::file::json_envelope::JsonEnvelope;

        let file = envelope_doc(r#"{"rows":[{"a":1}]}"#);
        let env = JsonEnvelope::scan(file.path()).unwrap().unwrap();
        let rows = env.get("rows").unwrap();

        let db = DuckdbConnection::new().unwrap();
        db.stage_collection(file.path(), rows).unwrap();
        db.stage_collection(file.path(), rows).unwrap();
        assert_eq!(db.staged_collections().len(), 1);
    }

    #[test]
    fn a_key_that_is_not_an_identifier_still_gets_a_usable_alias() {
        use crate::file::json_envelope::JsonEnvelope;

        let file = envelope_doc(r#"{"user events-2024":[{"a":1}]}"#);
        let env = JsonEnvelope::scan(file.path()).unwrap().unwrap();
        let c = env.get("user events-2024").unwrap();

        let db = DuckdbConnection::new().unwrap();
        db.stage_collection(file.path(), c).unwrap();
        assert_eq!(db.staged_collections(), ["user_events_2024"]);
        assert_eq!(
            batch_rows_of(&db.query("SELECT * FROM user_events_2024").unwrap()),
            1
        );
    }

    fn batch_rows_of(batches: &[RecordBatch]) -> usize {
        batches.iter().map(|b| b.num_rows()).sum()
    }
}
