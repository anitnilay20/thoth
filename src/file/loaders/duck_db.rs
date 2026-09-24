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

/// Table inside that cache recording the document's top-level layout, so a
/// later open knows what the document contains without rescanning it — and
/// knows about keys that were never ingested.
const LAYOUT_TABLE: &str = "__thoth_layout";

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
    /// The envelope document behind a cache, so a query naming a collection
    /// that was never staged can stage it and carry on.
    document: Mutex<Option<PathBuf>>,
}

impl DuckdbConnection {
    pub fn new() -> Result<Self> {
        let conn = Connection::open_in_memory()?;
        // Before anything can trigger one: DuckDB otherwise fetches a reader
        // on first use, from a worker thread, without telling anyone.
        crate::file::extensions::disable_autoload(&conn)?;
        Ok(Self {
            conn: Mutex::new(conn),
            sources: Mutex::new(Vec::new()),
            row_count: Mutex::new(None),
            staged: Mutex::new(HashMap::new()),
            document: Mutex::new(None),
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
        let mut stmt = conn
            .prepare(sql)
            .map_err(|e| ThothError::DatabaseQueryError {
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

    /// Which of DuckDB's readers can read this file, if any.
    ///
    /// Each candidate is probed with `LIMIT 1`, not a scan: the readers infer
    /// their schema from a sample, so a probe costs a sample and not the
    /// document. It still parses, so this belongs on a worker — it is reached
    /// only from `IndexJob`.
    ///
    /// JSON is tried first because it is the stricter of the two: almost any
    /// line-oriented text parses *as CSV*, so CSV also has to yield more than
    /// one column to count. A single column is the sniffer failing to find a
    /// delimiter, which is to say the file is prose.
    fn sniff_reader(&self, path: &Path) -> Option<FileType> {
        let literal = quote_literal(&path.to_string_lossy());
        if self
            .collect(&format!("SELECT * FROM read_json_auto({literal}) LIMIT 1"))
            .is_ok()
        {
            return Some(FileType::Json);
        }
        let csv = self
            .collect(&format!("SELECT * FROM read_csv_auto({literal}) LIMIT 1"))
            .ok()?;
        let columns = csv.first().map(|b| b.num_columns()).unwrap_or(0);
        (columns > 1).then_some(FileType::Csv)
    }

    /// Load an optional reader if the user has already installed it.
    ///
    /// Silent on absence: not having it is the ordinary state, and the
    /// reader call that follows is what turns it into something the user can
    /// act on — DuckDB's own error names the extension that would fix it.
    fn load_extension(&self, name: &str) {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        crate::file::extensions::load(&conn, name);
    }

    /// Whether this connection can read `file_type` right now, or is missing
    /// the optional extension for it.
    pub fn missing_extension(
        &self,
        file_type: FileType,
    ) -> Option<crate::file::extensions::Extension> {
        let needed = crate::file::extensions::required_for(file_type)?;
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        (!crate::file::extensions::is_installed(&conn, &needed.name)).then_some(needed)
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
            // Formats whose reader is an optional extension. `LOAD` only —
            // never `INSTALL`: fetching one is the user's decision, and the
            // failure below is what asks them. See `file::extensions`.
            FileType::Excel => {
                self.load_extension("excel");
                format!("read_xlsx({literal})")
            }
            FileType::Arrow => {
                self.load_extension("arrow");
                format!("read_arrow({literal})")
            }
            FileType::DB => return self.register_database(alias, scan_path),
        };
        self.execute(&format!(
            "CREATE OR REPLACE VIEW {} AS SELECT * FROM {reader}",
            quote_ident(alias)
        ))
    }

    /// Every table inside the attached database, in name order.
    ///
    /// Empty for a file that is not a database. A database opens on one of its
    /// tables — it has to open on *something* — and without this the rest were
    /// reachable only by writing SQL, which is not a viewer.
    ///
    /// Names only: counting rows in each would be a scan per table, and this
    /// is read while drawing.
    pub fn database_tables(&self) -> Vec<String> {
        let Some(alias) = self.primary_alias() else {
            return Vec::new();
        };
        let db_alias = format!("{alias}__db");
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let Ok(mut stmt) = conn.prepare(
            "SELECT table_name FROM duckdb_tables() WHERE database_name = ? ORDER BY table_name",
        ) else {
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map([&db_alias], |row| row.get::<_, String>(0)) else {
            return Vec::new();
        };
        rows.flatten().collect()
    }

    /// Point the primary alias at another table of the attached database.
    ///
    /// Returns its row count, which is only counted once the user has actually
    /// asked for the table — never for all of them at once.
    pub fn show_database_table(&self, table: &str) -> Result<usize> {
        let alias = self
            .primary_alias()
            .ok_or_else(|| ThothError::DatabaseError {
                reason: "No file is open on this connection".to_string(),
            })?;
        let db_alias = format!("{alias}__db");
        self.execute(&format!(
            "CREATE OR REPLACE VIEW {} AS SELECT * FROM {}.{}",
            quote_ident(&alias),
            quote_ident(&db_alias),
            quote_ident(table)
        ))?;
        *self.row_count.lock().unwrap_or_else(|e| e.into_inner()) = None;
        self.row_count_of(&alias)
    }

    /// Attach a SQLite/DuckDB database and point the alias at its first table.
    fn register_database(&self, alias: &str, path: &Path) -> Result<()> {
        if is_sqlite(path) {
            // `LOAD` only, for the same reason as the readers above: the
            // ATTACH below reports it when the extension is not there, and
            // that error is what the offer is built from.
            self.load_extension("sqlite");
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

    /// Record a document's top-level layout in the attached cache.
    ///
    /// Every key is stored, not just the queryable ones: a document's objects
    /// and scalars are part of what it contains, and a viewer that silently
    /// omits them misrepresents the file.
    pub fn record_layout(&self, document: &Path, collections: &[Collection]) -> Result<()> {
        self.execute(&format!(
            "CREATE OR REPLACE TABLE {}.{} \
             (name VARCHAR, kind VARCHAR, byte_start BIGINT, byte_end BIGINT, source VARCHAR)",
            quote_ident(CACHE_DB),
            quote_ident(LAYOUT_TABLE)
        ))?;
        let source = quote_literal(&document.to_string_lossy());
        for c in collections {
            self.execute(&format!(
                "INSERT INTO {}.{} VALUES ({}, {}, {}, {}, {source})",
                quote_ident(CACHE_DB),
                quote_ident(LAYOUT_TABLE),
                quote_literal(&c.name),
                quote_literal(c.kind.as_str()),
                c.start,
                c.end
            ))?;
        }
        *self.document.lock().unwrap_or_else(|e| e.into_inner()) = Some(document.to_path_buf());
        Ok(())
    }

    /// The layout recorded in an attached cache.
    pub fn cached_layout(&self) -> Vec<Collection> {
        {
            // Remember where the document lives, so an unstaged collection can
            // still be reached.
            let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
            if let Ok(source) = conn.query_row(
                &format!(
                    "SELECT source FROM {}.{} LIMIT 1",
                    quote_ident(CACHE_DB),
                    quote_ident(LAYOUT_TABLE)
                ),
                [],
                |row| row.get::<_, String>(0),
            ) {
                drop(conn);
                *self.document.lock().unwrap_or_else(|e| e.into_inner()) =
                    Some(PathBuf::from(source));
            }
        }
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let sql = format!(
            "SELECT name, kind, byte_start, byte_end FROM {}.{} ORDER BY byte_start",
            quote_ident(CACHE_DB),
            quote_ident(LAYOUT_TABLE)
        );
        let Ok(mut stmt) = conn.prepare(&sql) else {
            return Vec::new();
        };
        let Ok(rows) = stmt.query_map([], |row| {
            Ok(Collection {
                name: row.get::<_, String>(0)?,
                kind: crate::file::json_envelope::ValueKind::parse(&row.get::<_, String>(1)?),
                start: row.get::<_, i64>(2)? as u64,
                end: row.get::<_, i64>(3)? as u64,
            })
        }) else {
            return Vec::new();
        };
        rows.flatten().collect()
    }

    /// Stage whichever collection an error says is missing.
    ///
    /// Returns whether anything was staged, so the caller knows to retry.
    fn stage_missing(&self, error: &str) -> bool {
        let Some(document) = self
            .document
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .clone()
        else {
            return false;
        };
        let layout = self.cached_layout();
        if layout.is_empty() {
            return false;
        }

        // The error names the table it could not find; match it against the
        // layout rather than trying to parse SQL.
        for collection in layout.iter().filter(|c| c.is_queryable()) {
            let alias = alias_for_name(&collection.name);
            if !error.contains(&alias) || self.has_relation(&alias) {
                continue;
            }
            if self.ingest_collection(&document, collection).is_ok() {
                return true;
            }
        }
        false
    }

    /// Whether a relation of this name is registered.
    pub fn has_relation(&self, alias: &str) -> bool {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        conn.query_row(
            &format!("SELECT 1 FROM {} LIMIT 0", quote_ident(alias)),
            [],
            |_| Ok(()),
        )
        .is_ok()
            || conn
                .prepare(&format!("SELECT * FROM {} LIMIT 0", quote_ident(alias)))
                .is_ok()
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

    /// Point `fetch` / `len` / `get` at a different registered relation.
    ///
    /// A document with several collections has several tables on one
    /// connection; this chooses which one the tab is currently showing without
    /// disturbing the others, so a query can still join across them.
    pub fn set_primary(&self, alias: &str) -> Result<()> {
        let rows = self.row_count_of(alias)?;
        let mut sources = self.sources.lock().unwrap_or_else(|e| e.into_inner());
        let entry = Source {
            alias: alias.to_string(),
            path: sources
                .first()
                .map(|s| s.path.clone())
                .unwrap_or_else(|| PathBuf::from(alias)),
            staged: None,
        };
        if sources.is_empty() {
            sources.push(entry);
        } else {
            sources[0] = entry;
        }
        *self.row_count.lock().unwrap_or_else(|e| e.into_inner()) = Some(rows);
        Ok(())
    }

    /// Rows in a named relation.
    pub fn row_count_of(&self, alias: &str) -> Result<usize> {
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let count: i64 = conn.query_row(
            &format!("SELECT count(*) FROM {}", quote_ident(alias)),
            [],
            |row| row.get(0),
        )?;
        Ok(count.max(0) as usize)
    }

    /// Define `alias` as a view over `sql`, and report how many rows it has.
    ///
    /// A query result is a relation, not a copy: the view is what
    /// [`set_primary`](Self::set_primary) is then pointed at, so the grid pages
    /// through the result the same way it pages through a table and a query
    /// over a large file costs one window rather than the whole result set.
    ///
    /// The view is temporary — it belongs to this session, not to the cache
    /// database, which holds the file's collections and nothing derived.
    pub fn define_view(&self, alias: &str, sql: &str) -> Result<usize> {
        let statement = format!(
            "CREATE OR REPLACE TEMP VIEW {} AS {sql}",
            quote_ident(alias)
        );
        // A query may be the first thing to name a collection, so an unknown
        // table is staged and the definition retried — the same courtesy
        // `query` extends, for the same reason.
        if let Err(error) = self.execute(&statement) {
            if !self.stage_missing(&error.to_string()) {
                return Err(error);
            }
            self.execute(&statement)?;
        }
        self.row_count_of(alias)
    }

    /// The columns of a relation and the SQL type of each, in order.
    ///
    /// The query builder offers operators by type — ordering comparisons on
    /// numbers and dates, substring ones on text — so it needs the schema, not
    /// just the names.
    pub fn column_types(&self, alias: &str) -> Result<Vec<(String, String)>> {
        let sql = format!("DESCRIBE {}", quote_ident(alias));
        let conn = self.conn.lock().unwrap_or_else(|e| e.into_inner());
        let mut stmt = conn
            .prepare(&sql)
            .map_err(|e| ThothError::DatabaseQueryError {
                query: sql.clone(),
                reason: e.to_string(),
            })?;
        let rows = stmt
            .query_map([], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .map_err(|e| ThothError::DatabaseQueryError {
                query: sql.clone(),
                reason: e.to_string(),
            })?;
        let mut columns = Vec::new();
        for row in rows {
            columns.push(row.map_err(|e| ThothError::DatabaseQueryError {
                query: sql.clone(),
                reason: e.to_string(),
            })?);
        }
        Ok(columns)
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
pub fn alias_for_name(name: &str) -> String {
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
        // An unnamed format is not necessarily an unreadable one: a `.log` of
        // NDJSON and a `.dat` of CSV are both things DuckDB reads perfectly
        // well, and the extension is the only reason to think otherwise. So
        // ask DuckDB first and only fall back to a plugin when it declines.
        // Formats DuckDB cannot read come through a plugin, staged as NDJSON
        // so it can scan them like any other JSON source.
        let (scan_type, staged) = match file_type {
            FileType::Unknown => match self.sniff_reader(path) {
                Some(readable) => (readable, None),
                None => (FileType::Json, Some(stage_via_plugin(path)?)),
            },
            FileType::Plugin => (FileType::Json, Some(stage_via_plugin(path)?)),
            named => (named, None),
        };

        let source = Source {
            alias: alias.to_string(),
            path: path.to_path_buf(),
            staged,
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
        match self.collect(query) {
            Ok(batches) => Ok(batches),
            // A collection this document holds but has not staged yet reads as
            // a missing table. Staging it and retrying keeps lazy ingest an
            // implementation detail rather than something SQL has to know
            // about.
            Err(error) => match self.stage_missing(&error.to_string()) {
                true => self.collect(query),
                false => Err(error),
            },
        }
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
pub(crate) fn is_sqlite(path: &Path) -> bool {
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
        let mut tmp = tempfile::Builder::new()
            .suffix(".ndjson")
            .tempfile()
            .unwrap();
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

    /// A file whose extension says nothing about its contents.
    fn unnamed_file(suffix: &str, contents: &str) -> NamedTempFile {
        let mut tmp = tempfile::Builder::new().suffix(suffix).tempfile().unwrap();
        tmp.write_all(contents.as_bytes()).unwrap();
        tmp.flush().unwrap();
        tmp
    }

    #[test]
    fn an_unnamed_format_opens_through_duckdb_when_duckdb_can_read_it() {
        // The rule: the extension decides nothing. A `.log` of NDJSON is a
        // table, and so is a `.dat` of CSV.
        let db = DuckdbConnection::new().unwrap();

        let ndjson = unnamed_file(".log", "{\"a\":1,\"b\":\"x\"}\n{\"a\":2,\"b\":\"y\"}\n");
        assert_eq!(db.sniff_reader(ndjson.path()), Some(FileType::Json));

        let csv = unnamed_file(".dat", "a,b,c\n1,2,3\n4,5,6\n");
        assert_eq!(db.sniff_reader(csv.path()), Some(FileType::Csv));
    }

    #[test]
    fn prose_is_not_a_one_column_table() {
        // DuckDB's CSV sniffer will read almost any line-oriented text, so the
        // probe only accepts CSV that found a delimiter. Without the guard a
        // log file becomes a grid of its own lines, which is worse than text.
        let db = DuckdbConnection::new().unwrap();
        let prose = unnamed_file(".log", "starting worker\nshard-02 ready\nall done\n");
        assert_eq!(db.sniff_reader(prose.path()), None);
    }

    #[test]
    fn an_unnamed_format_duckdb_cannot_read_declines() {
        // No reader claims it, so `open` falls through to a plugin and then to
        // the text index — which is what makes any text file openable.
        let db = DuckdbConnection::new().unwrap();
        let binaryish = unnamed_file(".bin", "\u{1}\u{2}\u{3}not data at all\u{0}\u{4}");
        assert_eq!(db.sniff_reader(binaryish.path()), None);
    }

    // ── The format matrix ───────────────────────────────────────────────────
    //
    // One test per format the engine claims, each opened the way the app opens
    // it — `DuckdbConnection::open_path`, not a hand-written reader call — so
    // a format that stops being wired up fails here rather than in the app.
    //
    // The binary fixtures are generated by DuckDB itself rather than committed:
    // a checked-in `.parquet` is a blob nobody can review, and one written by
    // the same version that reads it cannot drift out of step.

    /// Rows and columns `open_path` yields for a file, as the app would see it.
    fn open_and_count(path: &Path) -> Result<(usize, usize)> {
        let db = DuckdbConnection::open_path(path)?;
        let alias = db.primary_alias().expect("an alias");
        let batches = db.query(&format!("SELECT * FROM {}", quote_ident(&alias)))?;
        let rows = batches.iter().map(|b| b.num_rows()).sum();
        let cols = batches.first().map(|b| b.num_columns()).unwrap_or(0);
        Ok((rows, cols))
    }

    /// Write `contents` to a file with `suffix`, returning the temp handle.
    fn fixture(suffix: &str, contents: &[u8]) -> NamedTempFile {
        let mut tmp = tempfile::Builder::new().suffix(suffix).tempfile().unwrap();
        tmp.write_all(contents).unwrap();
        tmp.flush().unwrap();
        tmp
    }

    /// Ask DuckDB to write a fixture in a format only it can produce.
    /// `None` when the format needs an extension this host cannot fetch.
    fn generated(
        name: &str,
        copy_sql: impl Fn(&str) -> String,
    ) -> Option<(tempfile::TempDir, PathBuf)> {
        let dir = tempfile::tempdir().ok()?;
        let path = dir.path().join(name);
        let db = DuckdbConnection::new().ok()?;
        db.query(&copy_sql(&path.to_string_lossy())).ok()?;
        path.exists().then_some((dir, path))
    }

    #[test]
    fn format_csv() {
        let f = fixture(".csv", b"a,b\n1,x\n2,y\n");
        assert_eq!(open_and_count(f.path()).unwrap(), (2, 2));
    }

    #[test]
    fn format_tsv() {
        let f = fixture(".tsv", b"a\tb\n1\tx\n2\ty\n");
        assert_eq!(open_and_count(f.path()).unwrap(), (2, 2));
    }

    #[test]
    fn format_ndjson() {
        let f = fixture(".ndjson", b"{\"a\":1,\"b\":\"x\"}\n{\"a\":2,\"b\":\"y\"}\n");
        assert_eq!(open_and_count(f.path()).unwrap(), (2, 2));
    }

    #[test]
    fn format_json_array() {
        let f = fixture(".json", b"[{\"a\":1,\"b\":\"x\"},{\"a\":2,\"b\":\"y\"}]");
        assert_eq!(open_and_count(f.path()).unwrap(), (2, 2));
    }

    #[test]
    fn format_parquet() {
        let Some((_dir, path)) = generated("t.parquet", |p| {
            format!(
                "COPY (SELECT 1 AS a, 'x' AS b UNION ALL SELECT 2, 'y') TO '{p}' (FORMAT PARQUET)"
            )
        }) else {
            panic!("parquet is bundled; writing a fixture must work");
        };
        assert_eq!(open_and_count(&path).unwrap(), (2, 2));
    }

    #[test]
    fn format_duckdb_database() {
        let Some((_dir, path)) = generated("t.duckdb", |p| {
            format!("ATTACH '{p}' AS mk; CREATE TABLE mk.t AS SELECT 1 AS a, 'x' AS b; DETACH mk;")
        }) else {
            panic!("attaching a duckdb database is core; writing one must work");
        };
        assert_eq!(open_and_count(&path).unwrap(), (1, 2));
    }

    #[test]
    fn a_database_offers_every_table_not_just_the_first() {
        // A database opens on one of its tables — it has to open on
        // something — and the rest used to be reachable only by writing SQL.
        // The picker needs the whole list, and switching must actually move
        // the primary relation.
        let Some((_dir, path)) = generated("multi.duckdb", |p| {
            format!(
                "ATTACH '{p}' AS mk; \
                 CREATE TABLE mk.customers AS SELECT 1 AS id, 'ada' AS name; \
                 CREATE TABLE mk.orders AS SELECT 1 AS id UNION ALL SELECT 2; \
                 CREATE TABLE mk.order_items AS SELECT 1 AS order_id UNION ALL SELECT 2 \
                   UNION ALL SELECT 3; \
                 DETACH mk;"
            )
        }) else {
            panic!("attaching a duckdb database is core; writing one must work");
        };

        let db = DuckdbConnection::open_path(&path).unwrap();
        assert_eq!(
            db.database_tables(),
            ["customers", "order_items", "orders"],
            "every table in the database is offered"
        );

        // It opens on the first by name, and each other one is reachable.
        assert_eq!(open_and_count(&path).unwrap(), (1, 2));
        assert_eq!(db.show_database_table("orders").unwrap(), 2);
        assert_eq!(db.show_database_table("order_items").unwrap(), 3);
        // And back again — switching is not one-way.
        assert_eq!(db.show_database_table("customers").unwrap(), 1);
    }

    #[test]
    fn a_file_that_is_not_a_database_offers_no_tables() {
        // The picker keys off this being empty to fall back to a document's
        // collections, so a plain file must answer with nothing rather than
        // with whatever `duckdb_tables()` happens to hold.
        let f = fixture(".csv", b"a,b\n1,x\n");
        let db = DuckdbConnection::open_path(f.path()).unwrap();
        assert!(db.database_tables().is_empty());
    }

    #[test]
    fn format_sqlite_database() {
        // Needs the `sqlite` extension, which DuckDB fetches on first use — so
        // an offline host skips rather than fails. The point of the test is
        // that the wiring is right when the extension is there.
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("t.sqlite");
        let built = std::process::Command::new("sqlite3")
            .arg(&path)
            .arg("CREATE TABLE t(a INTEGER, b TEXT); INSERT INTO t VALUES (1,'x'),(2,'y');")
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);
        if !built {
            eprintln!("skipping: no sqlite3 CLI to build a fixture");
            return;
        }
        match open_and_count(&path) {
            Ok(counts) => assert_eq!(counts, (2, 2)),
            Err(e) => eprintln!("skipping: sqlite extension unavailable ({e})"),
        }
    }

    #[test]
    fn format_excel() {
        // Same story as SQLite: the `excel` extension is fetched on demand.
        let Some((_dir, path)) = generated("t.xlsx", |p| {
            format!(
                "INSTALL excel; LOAD excel; \
                 COPY (SELECT 1 AS a, 'x' AS b UNION ALL SELECT 2, 'y') TO '{p}' (FORMAT XLSX, HEADER true)"
            )
        }) else {
            eprintln!("skipping: excel extension unavailable");
            return;
        };
        match open_and_count(&path) {
            Ok(counts) => assert_eq!(counts, (2, 2)),
            Err(e) => eprintln!("skipping: excel extension unavailable ({e})"),
        }
    }

    #[test]
    fn every_native_format_has_a_reader() {
        // `is_native` is what stops a plugin shadowing the engine, so each
        // format it claims must actually resolve to a reader — a format listed
        // there with no reader behind it would be unopenable by anything.
        for ty in [
            FileType::Json,
            FileType::Csv,
            FileType::Parquet,
            FileType::Excel,
            FileType::DB,
        ] {
            assert!(ty.is_native(), "{ty:?} should be read by the engine");
        }
        // And the two that are not the engine's: Unknown is probed at open
        // time, Plugin is a plugin's by definition.
        assert!(!FileType::Unknown.is_native());
        assert!(!FileType::Plugin.is_native());
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
        assert_eq!(
            alias_for(Path::new("/tmp/sales-2024.parquet")),
            "sales_2024"
        );
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
        assert_eq!(
            db.len().unwrap(),
            2,
            "the first staged collection is primary"
        );
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

    #[test]
    fn switching_the_primary_relation_changes_what_is_read() {
        use crate::file::json_envelope::JsonEnvelope;

        let file =
            envelope_doc(r#"{"users":[{"id":1},{"id":2},{"id":3}],"logs":[{"level":"INFO"}]}"#);
        let env = JsonEnvelope::scan(file.path()).unwrap().unwrap();
        let db = DuckdbConnection::new().unwrap();
        for c in env.queryable() {
            db.stage_collection(file.path(), c).unwrap();
        }

        // Whichever was staged first is primary; selecting another re-points
        // `len` and `fetch` without disturbing the rest.
        db.set_primary("users").unwrap();
        assert_eq!(db.primary_alias().as_deref(), Some("users"));
        assert_eq!(db.len().unwrap(), 3);

        db.set_primary("logs").unwrap();
        assert_eq!(db.len().unwrap(), 1, "row count follows the selection");

        // And both remain queryable together.
        assert_eq!(db.row_count_of("users").unwrap(), 3);
        assert_eq!(
            batch_rows_of(&db.query("SELECT * FROM users, logs").unwrap()),
            3
        );
    }

    #[test]
    fn selecting_an_unknown_relation_is_an_error_not_a_silent_switch() {
        let file = envelope_doc(r#"{"rows":[{"a":1}]}"#);
        use crate::file::json_envelope::JsonEnvelope;
        let env = JsonEnvelope::scan(file.path()).unwrap().unwrap();
        let db = DuckdbConnection::new().unwrap();
        db.stage_collection(file.path(), env.get("rows").unwrap())
            .unwrap();

        assert!(db.set_primary("nope").is_err());
        assert_eq!(db.primary_alias().as_deref(), Some("rows"));
    }

    // ── Compiled queries actually run ───────────────────────────────────────
    //
    // The builder's compiler is tested against its own output in the SDK; these
    // check the output is SQL DuckDB accepts and answers correctly, which a
    // string comparison cannot.

    fn query_rows(db: &DuckdbConnection, sql: &str) -> Vec<serde_json::Value> {
        crate::file::loaders::batches_to_values(&db.query(sql).unwrap()).unwrap()
    }

    fn logs_table() -> (DuckdbConnection, NamedTempFile) {
        let file = ndjson_file(
            "{\"level\":\"ERROR\",\"service\":\"api\",\"ms\":120}\n\
             {\"level\":\"INFO\",\"service\":\"api\",\"ms\":8}\n\
             {\"level\":\"ERROR\",\"service\":\"web\",\"ms\":300}\n\
             {\"level\":\"WARN\",\"service\":\"web\",\"ms\":45}\n",
        );
        let db = DuckdbConnection::open_path(file.path()).unwrap();
        (db, file)
    }

    #[test]
    fn a_compiled_filter_runs_and_selects_the_right_rows() {
        use thoth_plugin_sdk::components::ColumnType;
        use thoth_plugin_sdk::components::{Filter, Operator, QuerySpec};

        let (db, file) = logs_table();
        let alias = crate::file::loaders::duck_db::alias_for(file.path());

        let spec = QuerySpec {
            filters: vec![Filter {
                field: "level".into(),
                operator: Operator::Equals,
                values: vec!["ERROR".into()],
                column: ColumnType::Text,
            }],
            ..Default::default()
        };
        let rows = query_rows(&db, &spec.compile(&alias).unwrap());
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().all(|r| r["level"] == "ERROR"));
    }

    #[test]
    fn a_compiled_grouping_runs_and_aggregates() {
        use thoth_plugin_sdk::components::{Aggregate, AggregateFn, QuerySpec, Sort};

        let (db, file) = logs_table();
        let alias = crate::file::loaders::duck_db::alias_for(file.path());

        let spec = QuerySpec {
            group_by: vec!["service".into()],
            aggregates: vec![
                Aggregate {
                    function: AggregateFn::Count,
                    field: String::new(),
                },
                Aggregate {
                    function: AggregateFn::Sum,
                    field: "ms".into(),
                },
            ],
            sort: vec![Sort {
                field: "service".into(),
                descending: false,
            }],
            ..Default::default()
        };
        let rows = query_rows(&db, &spec.compile(&alias).unwrap());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0]["service"], "api");
        assert_eq!(rows[0]["count"], 2);
        assert_eq!(rows[0]["sum_ms"], 128);
        assert_eq!(rows[1]["sum_ms"], 345);
    }

    #[test]
    fn a_query_result_is_a_view_the_grid_can_page_through() {
        // The point of defining a view rather than collecting the result: the
        // grid reads windows out of it, so a query over a large file costs one
        // window and not the whole result set.
        use thoth_plugin_sdk::components::{ColumnType, Filter, Operator, QuerySpec};

        let (db, file) = logs_table();
        let alias = crate::file::loaders::duck_db::alias_for(file.path());
        let spec = QuerySpec {
            filters: vec![Filter {
                field: "level".into(),
                operator: Operator::Equals,
                values: vec!["ERROR".into()],
                column: ColumnType::Text,
            }],
            ..Default::default()
        };

        let rows = db
            .define_view("__result", &spec.compile(&alias).unwrap())
            .unwrap();
        assert_eq!(rows, 2);

        // Pointed at the view, the ordinary read path returns the result.
        db.set_primary("__result").unwrap();
        assert_eq!(db.len().unwrap(), 2);
        let page: usize = db
            .fetch(Vec::new(), Some(1), Some(1))
            .unwrap()
            .iter()
            .map(|b| b.num_rows())
            .sum();
        assert_eq!(page, 1, "a window of the result, not all of it");

        // And the source relation is untouched, so running a second query does
        // not compound on the first.
        assert_eq!(db.row_count_of(&alias).unwrap(), 4);
    }

    #[test]
    fn a_second_query_replaces_the_first_result() {
        let (db, file) = logs_table();
        let alias = crate::file::loaders::duck_db::alias_for(file.path());
        let quoted = quote_ident(&alias);

        assert_eq!(
            db.define_view("__result", &format!("SELECT * FROM {quoted}"))
                .unwrap(),
            4
        );
        // A tab runs query after query under one view name; the definition has
        // to give way rather than fail as "already exists".
        assert_eq!(
            db.define_view(
                "__result",
                &format!("SELECT * FROM {quoted} WHERE level = 'WARN'")
            )
            .unwrap(),
            1
        );
    }

    #[test]
    fn column_types_carry_the_schema_the_builder_offers_operators_by() {
        use thoth_plugin_sdk::components::ColumnType;

        let (db, file) = logs_table();
        let alias = crate::file::loaders::duck_db::alias_for(file.path());
        let columns = db.column_types(&alias).unwrap();

        let named: Vec<&str> = columns.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(named, ["level", "service", "ms"]);

        let classified: Vec<ColumnType> = columns
            .iter()
            .map(|(_, sql)| ColumnType::from_sql(sql))
            .collect();
        // Ordering comparisons belong on `ms` and substring ones on `level`,
        // which is the whole reason the types are read.
        assert_eq!(classified[0], ColumnType::Text);
        assert_eq!(classified[2], ColumnType::Integer);
    }

    #[test]
    fn a_hostile_value_is_data_not_syntax_when_it_reaches_duckdb() {
        use thoth_plugin_sdk::components::{ColumnType, Filter, Operator, QuerySpec};

        let (db, file) = logs_table();
        let alias = crate::file::loaders::duck_db::alias_for(file.path());

        let spec = QuerySpec {
            filters: vec![Filter {
                field: "level".into(),
                operator: Operator::Equals,
                // If this were interpolated rather than escaped, the table
                // would be gone rather than the result empty.
                values: vec!["'; DROP TABLE logs; --".into()],
                column: ColumnType::Text,
            }],
            ..Default::default()
        };
        let rows = query_rows(&db, &spec.compile(&alias).unwrap());
        assert!(rows.is_empty(), "matched nothing, and harmed nothing");
        // The relation is still there and still complete.
        assert_eq!(db.row_count_of(&alias).unwrap(), 4);
    }

    #[test]
    fn a_contains_filter_matches_a_literal_percent() {
        use thoth_plugin_sdk::components::{ColumnType, Filter, Operator, QuerySpec};

        let file = ndjson_file("{\"note\":\"50% off\"}\n{\"note\":\"50 percent\"}\n");
        let db = DuckdbConnection::open_path(file.path()).unwrap();
        let alias = crate::file::loaders::duck_db::alias_for(file.path());

        let spec = QuerySpec {
            filters: vec![Filter {
                field: "note".into(),
                operator: Operator::Contains,
                values: vec!["50%".into()],
                column: ColumnType::Text,
            }],
            ..Default::default()
        };
        let rows = query_rows(&db, &spec.compile(&alias).unwrap());
        assert_eq!(
            rows.len(),
            1,
            "the wildcard is the user's text, not a pattern"
        );
        assert_eq!(rows[0]["note"], "50% off");
    }
}
