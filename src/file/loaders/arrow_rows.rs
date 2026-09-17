//! Arrow is the transport for every read (#147 / #148).
//!
//! Nothing in Thoth pulls a file into memory as JSON. A read is always a
//! windowed [`FileLoader::fetch`] that comes back as Arrow `RecordBatch`es.
//!
//! [`RecordWindow`] is the lazy-loading primitive: it holds one Arrow window
//! and refetches only when a caller asks for rows outside it, so scrolling a
//! multi-gigabyte file costs one query per window rather than one per row.
//!
//! Two ways to read that window. [`RecordWindow::locate`] hands back the raw
//! batches so callers can walk them with
//! [`arrow_tree`](crate::file::loaders::arrow_tree) — that is what the file
//! viewer does, and it never builds a `Value`. [`RecordWindow::records`]
//! converts to JSON, for consumers that genuinely think in records (the MCP
//! tools, export).

use arrow::json::ArrayWriter;
use duckdb::arrow::array::RecordBatch;
use serde_json::Value;

use crate::error::Result;
use crate::file::loaders::FileLoader;

/// Rows fetched per window. Large enough that a screenful of a table never
/// spans more than one query, small enough to stay cheap on a huge file.
pub const WINDOW_ROWS: usize = 2048;

/// Convert Arrow batches to one JSON value per row.
///
/// Goes through Arrow's own JSON writer rather than a hand-rolled match on
/// `DataType`, so nested structs, lists, decimals and timestamps all render
/// the way DuckDB and Arrow agree they should.
pub fn batches_to_values(batches: &[RecordBatch]) -> Result<Vec<Value>> {
    let populated: Vec<&RecordBatch> = batches.iter().filter(|b| b.num_rows() > 0).collect();
    if populated.is_empty() {
        return Ok(Vec::new());
    }

    let mut buf = Vec::new();
    let mut writer = ArrayWriter::new(&mut buf);
    for batch in populated {
        writer.write(batch)?;
    }
    writer.finish()?;

    match serde_json::from_slice(&buf)? {
        Value::Array(rows) => Ok(rows),
        other => Ok(vec![other]),
    }
}

/// Column names carried by a set of batches, in schema order.
pub fn batch_columns(batches: &[RecordBatch]) -> Vec<String> {
    batches
        .first()
        .map(|b| {
            b.schema()
                .fields()
                .iter()
                .map(|f| f.name().to_string())
                .collect()
        })
        .unwrap_or_default()
}

/// Total rows across a set of batches.
pub fn batch_rows(batches: &[RecordBatch]) -> usize {
    batches.iter().map(|b| b.num_rows()).sum()
}

/// One lazily-fetched Arrow window over a [`FileLoader`].
///
/// Callers ask for a row index; the window fetches the surrounding block once
/// and answers from Arrow until the caller scrolls out of range.
pub struct RecordWindow {
    batches: Vec<RecordBatch>,
    /// The window's rows as JSON, converted lazily and at most once per fetch.
    ///
    /// Only [`RecordWindow::records`] populates this, so callers that walk
    /// Arrow directly never pay for it.
    values: Option<Vec<Value>>,
    /// Absolute row index of the first row held.
    start: usize,
    /// Rows actually held (may be short at end-of-file).
    len: usize,
    /// Rows requested per fetch.
    window: usize,
}

impl Default for RecordWindow {
    fn default() -> Self {
        Self::new(WINDOW_ROWS)
    }
}

impl RecordWindow {
    pub fn new(window: usize) -> Self {
        Self {
            batches: Vec::new(),
            values: None,
            start: 0,
            len: 0,
            window: window.max(1),
        }
    }

    /// Drop the cached window — call when the underlying file changes.
    pub fn invalidate(&mut self) {
        self.batches.clear();
        self.values = None;
        self.start = 0;
        self.len = 0;
    }

    /// Whether `[start, start + count)` is already loaded.
    fn holds(&self, start: usize, count: usize) -> bool {
        self.len > 0 && start >= self.start && start + count <= self.start + self.len
    }

    /// Ensure `[start, start + count)` is loaded, fetching if needed.
    ///
    /// Returns the batches covering the current window (which may extend
    /// beyond the requested range).
    pub fn ensure(
        &mut self,
        loader: &dyn FileLoader,
        start: usize,
        count: usize,
    ) -> Result<&[RecordBatch]> {
        if !self.holds(start, count) {
            let span = count.max(self.window);
            let batches = loader.fetch(Vec::new(), Some(start), Some(span))?;
            self.len = batch_rows(&batches);
            self.batches = batches;
            self.values = None; // stale for the new window
            self.start = start;
        }
        Ok(&self.batches)
    }

    /// Rows `[start, start + count)` as JSON, fetching only when the window
    /// does not already cover them.
    ///
    /// The Arrow → JSON conversion happens once per window, not once per call,
    /// so reading a window row by row stays linear.
    pub fn records(
        &mut self,
        loader: &dyn FileLoader,
        start: usize,
        count: usize,
    ) -> Result<Vec<Value>> {
        if count == 0 {
            return Ok(Vec::new());
        }
        self.ensure(loader, start, count)?;
        if self.values.is_none() {
            self.values = Some(batches_to_values(&self.batches)?);
        }
        let offset = start - self.start;
        Ok(self
            .values
            .as_ref()
            .map(|values| values.iter().skip(offset).take(count).cloned().collect())
            .unwrap_or_default())
    }

    /// A single row as JSON.
    pub fn record(&mut self, loader: &dyn FileLoader, index: usize) -> Result<Option<Value>> {
        Ok(self.records(loader, index, 1)?.into_iter().next())
    }

    /// Ensure `index` is loaded and return the window's batches together with
    /// the row's index *within* them.
    ///
    /// This is the Arrow-native entry point: callers walk the batches with
    /// [`crate::file::loaders::arrow_tree`] instead of asking for JSON, so
    /// nothing is materialized.
    pub fn locate(
        &mut self,
        loader: &dyn FileLoader,
        index: usize,
    ) -> Result<(&[RecordBatch], usize)> {
        self.ensure(loader, index, 1)?;
        let local = index - self.start;
        Ok((&self.batches, local))
    }

    /// Column names of the loaded window, if any rows have been fetched.
    pub fn columns(&self) -> Vec<String> {
        batch_columns(&self.batches)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file::loaders::duck_db::DuckdbConnection;
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn ndjson(lines: &str) -> NamedTempFile {
        let mut tmp = tempfile::Builder::new()
            .suffix(".ndjson")
            .tempfile()
            .unwrap();
        tmp.write_all(lines.as_bytes()).unwrap();
        tmp.flush().unwrap();
        tmp
    }

    #[test]
    fn batches_convert_to_one_value_per_row() {
        let file = ndjson("{\"a\":1,\"b\":\"x\"}\n{\"a\":2,\"b\":\"y\"}\n");
        let db = DuckdbConnection::open_path(file.path()).unwrap();
        let batches = db.fetch(Vec::new(), None, None).unwrap();

        let values = batches_to_values(&batches).unwrap();
        assert_eq!(values.len(), 2);
        assert_eq!(values[0]["a"], 1);
        assert_eq!(values[1]["b"], "y");
        assert_eq!(batch_columns(&batches), vec!["a", "b"]);
    }

    #[test]
    fn nested_structures_survive_the_arrow_round_trip() {
        let file = ndjson("{\"user\":{\"name\":\"ada\"},\"tags\":[\"x\",\"y\"]}\n");
        let db = DuckdbConnection::open_path(file.path()).unwrap();
        let values = batches_to_values(&db.fetch(Vec::new(), None, None).unwrap()).unwrap();

        assert_eq!(values[0]["user"]["name"], "ada");
        assert_eq!(values[0]["tags"][1], "y");
    }

    #[test]
    fn window_serves_repeat_reads_without_refetching() {
        let rows: String = (0..50).map(|i| format!("{{\"n\":{i}}}\n")).collect();
        let file = ndjson(&rows);
        let db = DuckdbConnection::open_path(file.path()).unwrap();

        let mut window = RecordWindow::new(32);
        assert_eq!(window.records(&db, 0, 4).unwrap()[0]["n"], 0);
        // Inside the loaded window — served from Arrow, no new fetch.
        assert!(window.holds(4, 8));
        assert_eq!(window.records(&db, 4, 1).unwrap()[0]["n"], 4);

        // Past the window — triggers a refetch anchored at the new start.
        assert_eq!(window.records(&db, 40, 2).unwrap()[1]["n"], 41);
        assert!(!window.holds(0, 1));
    }

    /// A loader that counts how often it is actually queried.
    struct CountingLoader<'a> {
        inner: &'a DuckdbConnection,
        fetches: std::cell::Cell<usize>,
    }

    impl FileLoader for CountingLoader<'_> {
        fn query(&self, q: &str) -> Result<Vec<RecordBatch>> {
            self.inner.query(q)
        }
        fn fetch(
            &self,
            f: Vec<String>,
            o: Option<usize>,
            l: Option<usize>,
        ) -> Result<Vec<RecordBatch>> {
            self.fetches.set(self.fetches.get() + 1);
            self.inner.fetch(f, o, l)
        }
        fn size(&self) -> Result<u128> {
            self.inner.size()
        }
        fn len(&self) -> Result<usize> {
            self.inner.len()
        }
        fn get(&self, i: usize) -> Result<RecordBatch> {
            self.inner.get(i)
        }
        fn open(&self, p: &str, a: &str) -> Result<()> {
            self.inner.open(p, a)
        }
    }

    #[test]
    fn reading_a_window_row_by_row_queries_once() {
        let rows: String = (0..500).map(|i| format!("{{\"n\":{i}}}\n")).collect();
        let file = ndjson(&rows);
        let db = DuckdbConnection::open_path(file.path()).unwrap();
        let counting = CountingLoader {
            inner: &db,
            fetches: std::cell::Cell::new(0),
        };

        // The tree viewer's access pattern: one record per visible row.
        let mut window = RecordWindow::new(1024);
        for i in 0..500 {
            assert_eq!(window.record(&counting, i).unwrap().unwrap()["n"], i);
        }

        assert_eq!(
            counting.fetches.get(),
            1,
            "the whole file fits one window, so it must be fetched once"
        );
    }

    #[test]
    fn window_clamps_at_end_of_file() {
        let file = ndjson("{\"n\":0}\n{\"n\":1}\n");
        let db = DuckdbConnection::open_path(file.path()).unwrap();

        let mut window = RecordWindow::default();
        assert_eq!(window.records(&db, 0, 100).unwrap().len(), 2);
        assert!(window.record(&db, 9).unwrap().is_none());
    }
}
