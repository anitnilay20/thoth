//! Papyrus — the data bus. The host's single owned view of tabular data, read
//! by handle in *pages*.
//!
//! Everything that displays data goes through here. A `data-view` render node
//! holds only a handle and asks Papyrus for the rows it is currently showing,
//! so the data never has to be resident anywhere else.
//!
//! Two kinds of sheet sit behind a handle:
//!
//! - [`Source::Rows`] — pushed by a producer over the `dataset-bus` WIT import
//!   (a plugin publishing results). Owned, bounded, stringly-typed. The
//!   registry LRU-evicts these, replaces an instance's sheet on re-`publish`,
//!   and drops a producer's sheets when its instance closes.
//! - [`Source::Arrow`] — a live query engine, scanned on demand. Nothing is
//!   materialized: a read fetches exactly the window asked for, so an open file
//!   of any size costs a window. This is the path DuckDB writes into.
//!
//! The two differ in fidelity as well as size. Arrow carries real types, nulls
//! and nesting; pushed rows are strings with a per-column type hint, so nested
//! values arrive already flattened.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, RwLock, Weak};
use std::time::Instant;

use serde_json::Value;
use thoth_plugin_sdk::dataset::{NodeKind, TreeNode};

use crate::file::loaders::{FileLoader, RecordWindow, arrow_tree};

/// Most datasets kept before LRU eviction of the least-recently-accessed.
const MAX_DATASETS: usize = 32;
/// Aggregate memory budget across all stored datasets. The least-recently-
/// accessed are evicted until the total fits (a single dataset larger than
/// this is still kept — we can't do better than one).
const MAX_BYTES: usize = 128 * 1024 * 1024;
/// Hard cap on rows returned by a single `read`, so a huge dataset never
/// crosses the boundary at once.
pub const MAX_READ_LIMIT: u32 = 1000;
/// Rows retained by a streaming dataset (`append`). Past this the oldest rows
/// are dropped (ring buffer) so an unbounded stream can't grow without limit.
const MAX_STREAM_ROWS: usize = 50_000;

#[derive(Clone, Debug)]
pub struct DatasetColumn {
    pub name: String,
    pub type_hint: String,
}

/// Registry metadata for a published dataset (no rows).
#[derive(Clone, Debug)]
pub struct DatasetMeta {
    pub id: String,
    pub name: String,
    pub source_plugin: String,
    /// Producer instance id, used to drop datasets when the producer closes.
    pub source_instance: String,
    pub kind: String,
    pub tags: Vec<String>,
    pub row_count: u64,
    pub columns: Vec<DatasetColumn>,
    /// Monotonic revision, bumped on every mutation (publish/update/append) so
    /// consumers (e.g. the render cache) can detect changes even when the row
    /// count is unchanged (an in-place `update`).
    pub revision: u64,
}

/// A contiguous page of rows.
#[derive(Clone, Debug)]
pub struct Page {
    pub columns: Vec<DatasetColumn>,
    pub rows: Vec<Vec<String>>,
    pub offset: u64,
    pub total: u64,
}

/// A live query engine behind a handle, plus the Arrow window currently held.
///
/// The window is what makes a read cheap: consecutive reads of nearby rows are
/// served from Arrow already in hand, and only a jump outside it costs a query.
struct ArrowSheet {
    loader: Arc<dyn FileLoader + Send + Sync>,
    window: RecordWindow,
    total: u64,
    columns: Vec<DatasetColumn>,
}

/// What backs a handle.
enum Source {
    /// Rows pushed by a producer — owned by the registry.
    Rows(Vec<Vec<String>>),
    /// A live engine, scanned on demand.
    Arrow(Box<ArrowSheet>),
}

impl Source {
    fn rows(&self) -> Option<&Vec<Vec<String>>> {
        match self {
            Source::Rows(rows) => Some(rows),
            Source::Arrow(_) => None,
        }
    }

    fn rows_mut(&mut self) -> Option<&mut Vec<Vec<String>>> {
        match self {
            Source::Rows(rows) => Some(rows),
            Source::Arrow(_) => None,
        }
    }
}

struct Stored {
    meta: DatasetMeta,
    source: Source,
    last_access: Instant,
    /// Estimated heap footprint of this dataset, tracked so the registry can
    /// enforce [`MAX_BYTES`] without re-summing every entry. An Arrow sheet
    /// holds only a window, so it contributes ~nothing.
    size: usize,
}

#[derive(Default)]
struct Registry {
    map: HashMap<String, Stored>,
    /// Publish order for stable listing.
    order: Vec<String>,
    /// Running sum of every `Stored::size`, kept in step via [`Registry::drop_dataset`].
    bytes: usize,
    seq: u64,
    /// Monotonic revision counter stamped onto a dataset on every mutation.
    rev: u64,
}

impl Registry {
    /// Next monotonic revision (stamped on publish/update/append).
    fn next_rev(&mut self) -> u64 {
        self.rev += 1;
        self.rev
    }
}

impl Registry {
    /// Remove a dataset by id, keeping `order` and the `bytes` total in step.
    fn drop_dataset(&mut self, id: &str) {
        if let Some(s) = self.map.remove(id) {
            self.bytes = self.bytes.saturating_sub(s.size);
        }
        self.order.retain(|o| o != id);
    }

    /// Evict the least-recently-accessed datasets while over either the count or
    /// the byte budget. Keeps at least one dataset (guard on `len > 1`) so a
    /// single oversized dataset can't loop forever.
    fn enforce_budget(&mut self) {
        while self.order.len() > 1 && (self.order.len() > MAX_DATASETS || self.bytes > MAX_BYTES) {
            let Some(victim) = self
                .order
                .iter()
                .min_by_key(|id| self.map.get(*id).map(|s| s.last_access))
                .cloned()
            else {
                break;
            };
            self.drop_dataset(&victim);
        }
    }
}

/// Estimated heap footprint of a single row (used so `append` can track bytes
/// incrementally instead of re-summing every retained row each call).
fn row_bytes(row: &[String]) -> usize {
    std::mem::size_of::<Vec<String>>()
        + row
            .iter()
            .map(|c| std::mem::size_of::<String>() + c.len())
            .sum::<usize>()
}

/// Estimated heap footprint of a dataset's rows + metadata strings.
fn dataset_bytes(meta: &DatasetMeta, rows: &[Vec<String>]) -> usize {
    let cells: usize = rows.iter().map(|r| row_bytes(r)).sum();
    let cols: usize = meta
        .columns
        .iter()
        .map(|c| c.name.len() + c.type_hint.len())
        .sum();
    let tags: usize = meta
        .tags
        .iter()
        .map(|t| std::mem::size_of::<String>() + t.len())
        .sum();
    cells + cols + tags + meta.name.len() + meta.source_plugin.len() + meta.source_instance.len()
}

type SharedRegistry = Arc<Mutex<Registry>>;

/// Core-owned dataset store.
pub struct PapyrusStore {
    registry: SharedRegistry,
}

impl PapyrusStore {
    /// Create an empty dataset store.
    pub fn new() -> Self {
        Self {
            registry: Arc::new(Mutex::new(Registry::default())),
        }
    }

    /// Install this store as the weak bridge used by WIT and SDK callbacks.
    pub fn install_as_active(&self) {
        if let Ok(mut active) = ACTIVE_REGISTRY.write() {
            *active = Arc::downgrade(&self.registry);
        }
    }
}

impl Default for PapyrusStore {
    fn default() -> Self {
        Self::new()
    }
}

static ACTIVE_REGISTRY: LazyLock<RwLock<Weak<Mutex<Registry>>>> =
    LazyLock::new(|| RwLock::new(Weak::new()));
static FALLBACK_REGISTRY: LazyLock<SharedRegistry> =
    LazyLock::new(|| Arc::new(Mutex::new(Registry::default())));

fn registry() -> SharedRegistry {
    ACTIVE_REGISTRY
        .read()
        .ok()
        .and_then(|active| active.upgrade())
        .unwrap_or_else(|| Arc::clone(&FALLBACK_REGISTRY))
}

/// Store a dataset published by `source_plugin` (instance `source_instance`),
/// returning its assigned id. Evicts the least-recently-accessed dataset when
/// the registry is full.
#[allow(clippy::too_many_arguments)]
pub fn publish(
    source_plugin: &str,
    source_instance: &str,
    name: String,
    kind: String,
    tags: Vec<String>,
    columns: Vec<DatasetColumn>,
    rows: Vec<Vec<String>>,
) -> String {
    let registry = registry();
    let Ok(mut reg) = registry.lock() else {
        return String::new();
    };
    // A fresh publish from an instance replaces that instance's previous
    // dataset — dropping the old rows immediately rather than waiting for LRU
    // or tab close. A producer that wants to keep the same dataset live should
    // call `update` (same handle) instead of re-publishing.
    let stale: Vec<String> = reg
        .map
        .values()
        .filter(|s| s.meta.source_instance == source_instance)
        .map(|s| s.meta.id.clone())
        .collect();
    for id in stale {
        reg.drop_dataset(&id);
    }
    reg.seq += 1;
    let revision = reg.next_rev();
    let id = format!("ds-{}", reg.seq);
    let meta = DatasetMeta {
        id: id.clone(),
        name,
        source_plugin: source_plugin.to_string(),
        source_instance: source_instance.to_string(),
        kind,
        tags,
        row_count: rows.len() as u64,
        columns,
        revision,
    };
    let size = dataset_bytes(&meta, &rows);
    reg.bytes += size;
    reg.map.insert(
        id.clone(),
        Stored {
            meta,
            source: Source::Rows(rows),
            last_access: Instant::now(),
            size,
        },
    );
    reg.order.push(id.clone());

    // Evict down to the count/byte budget (never the just-published dataset —
    // it's the most recently accessed).
    reg.enforce_budget();
    id
}

/// Publish a live query engine as a sheet, returning its handle.
///
/// Nothing is copied. The engine stays the owner of the data and Papyrus reads
/// windows out of it on demand, so a file of any size costs one window — this
/// is the path an open file takes to reach a `data-view`.
pub fn publish_arrow(
    source: &str,
    instance: &str,
    name: String,
    loader: Arc<dyn FileLoader + Send + Sync>,
) -> Option<String> {
    let total = loader.len().ok()? as u64;
    let columns: Vec<DatasetColumn> = {
        use crate::file::loaders::RecordSource;
        loader
            .column_names()
            .unwrap_or_default()
            .into_iter()
            .map(|name| DatasetColumn {
                name,
                type_hint: String::new(),
            })
            .collect()
    };

    let registry = registry();
    let mut reg = registry.lock().ok()?;
    // Re-publishing from the same instance replaces its previous sheet.
    let stale: Vec<String> = reg
        .map
        .values()
        .filter(|s| s.meta.source_instance == instance)
        .map(|s| s.meta.id.clone())
        .collect();
    for id in stale {
        reg.drop_dataset(&id);
    }

    reg.seq += 1;
    let revision = reg.next_rev();
    let id = format!("ds-{}", reg.seq);
    let meta = DatasetMeta {
        id: id.clone(),
        name,
        source_plugin: source.to_string(),
        source_instance: instance.to_string(),
        kind: "file".to_string(),
        tags: Vec::new(),
        row_count: total,
        columns: columns.clone(),
        revision,
    };
    reg.map.insert(
        id.clone(),
        Stored {
            meta,
            source: Source::Arrow(Box::new(ArrowSheet {
                loader,
                window: RecordWindow::default(),
                total,
                columns,
            })),
            last_access: Instant::now(),
            // A live sheet holds only a window; it doesn't count against the
            // registry's byte budget for owned rows.
            size: 0,
        },
    );
    reg.order.push(id.clone());
    Some(id)
}

// ── Lazy node access ─────────────────────────────────────────────────────────
//
// A page is enough to draw a table. A *tree* needs to ask what one node's
// children are without materializing its siblings — that is what lets the
// viewer draw a screenful of a file that does not fit in memory.

/// Total records behind a handle.
pub fn total(id: &str) -> u64 {
    with_sheet(id, |stored| match &stored.source {
        Source::Rows(rows) => rows.len() as u64,
        Source::Arrow(sheet) => sheet.total,
    })
    .unwrap_or(0)
}

/// Whether records have any children to expand.
///
/// Answered from the schema, never from the data — which is what makes listing
/// a million collapsed records free.
pub fn records_expandable(id: &str) -> bool {
    with_sheet(id, |stored| !stored.meta.columns.is_empty()).unwrap_or(false)
}

/// Children of the node at `rel` within record `root`.
pub fn children(id: &str, root: u64, rel: &str) -> Vec<TreeNode> {
    with_sheet(id, |stored| match &mut stored.source {
        Source::Arrow(sheet) => match sheet.window.locate(&*sheet.loader, root as usize) {
            Ok((batches, local)) => arrow_tree::children(batches, local, rel),
            Err(_) => Vec::new(),
        },
        // Pushed rows are already flat: a record's children are its cells, and
        // a cell has none.
        Source::Rows(rows) => {
            if !rel.is_empty() {
                return Vec::new();
            }
            let Some(row) = rows.get(root as usize) else {
                return Vec::new();
            };
            stored
                .meta
                .columns
                .iter()
                .enumerate()
                .map(|(i, col)| {
                    let cell = row.get(i).map(String::as_str).unwrap_or("");
                    let (preview, token) = cell_preview(cell, &col.type_hint);
                    TreeNode {
                        label: col.name.clone(),
                        segment: format!(".{}", col.name),
                        kind: NodeKind::Leaf,
                        preview,
                        token,
                    }
                })
                .collect()
        }
    })
    .unwrap_or_default()
}

/// Display text of the leaf at `rel`.
pub fn node_preview(id: &str, root: u64, rel: &str) -> String {
    with_sheet(id, |stored| match &mut stored.source {
        Source::Arrow(sheet) => match sheet.window.locate(&*sheet.loader, root as usize) {
            Ok((batches, local)) => arrow_tree::node_preview(batches, local, rel),
            Err(_) => "null".to_string(),
        },
        Source::Rows(_) => "null".to_string(),
    })
    .unwrap_or_else(|| "null".to_string())
}

/// The subtree at `rel` as JSON — the edge conversion, for clipboard and
/// export. Drawing a row must not call this.
pub fn node_json(id: &str, root: u64, rel: &str) -> Option<Value> {
    with_sheet(id, |stored| match &mut stored.source {
        Source::Arrow(sheet) => {
            let (batches, local) = sheet.window.locate(&*sheet.loader, root as usize).ok()?;
            arrow_tree::node_to_json(batches, local, rel)
        }
        Source::Rows(rows) => {
            let row = rows.get(root as usize)?;
            let mut map = serde_json::Map::new();
            for (i, col) in stored.meta.columns.iter().enumerate() {
                let cell = row.get(i).map(String::as_str).unwrap_or("");
                map.insert(col.name.clone(), typed_cell(cell, &col.type_hint));
            }
            Some(Value::Object(map))
        }
    })
    .flatten()
}

fn with_sheet<T>(id: &str, f: impl FnOnce(&mut Stored) -> T) -> Option<T> {
    let registry = registry();
    let mut reg = registry.lock().ok()?;
    let stored = reg.map.get_mut(id)?;
    stored.last_access = Instant::now();
    Some(f(stored))
}

/// Recover a pushed cell's JSON value from its column's type hint. Pushed rows
/// are strings, so this is the best fidelity available on that path.
fn typed_cell(cell: &str, type_hint: &str) -> Value {
    if cell.is_empty() {
        return Value::String(String::new());
    }
    match type_hint {
        "integer" => cell
            .parse::<i64>()
            .map(Value::from)
            .unwrap_or_else(|_| Value::String(cell.to_string())),
        "float" => cell
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(cell.to_string())),
        "boolean" => match cell.to_ascii_lowercase().as_str() {
            "true" | "t" | "1" => Value::Bool(true),
            "false" | "f" | "0" => Value::Bool(false),
            _ => Value::String(cell.to_string()),
        },
        _ => Value::String(cell.to_string()),
    }
}

/// Row text and syntax token for a pushed cell.
fn cell_preview(cell: &str, type_hint: &str) -> (String, thoth_plugin_sdk::theme::TextToken) {
    use thoth_plugin_sdk::theme::TextToken;
    match typed_cell(cell, type_hint) {
        Value::Number(n) => (n.to_string(), TextToken::Number),
        Value::Bool(b) => (b.to_string(), TextToken::Boolean),
        other => (
            format!("\"{}\"", other.as_str().unwrap_or_default()),
            TextToken::Str,
        ),
    }
}

/// Metadata (no rows) for a single dataset by id; `None` if unknown.
pub fn meta(id: &str) -> Option<DatasetMeta> {
    let registry = registry();
    let reg = registry.lock().ok()?;
    reg.map.get(id).map(|s| s.meta.clone())
}

/// Metadata for all published datasets, in publish order.
pub fn list() -> Vec<DatasetMeta> {
    let registry = registry();
    let Ok(reg) = registry.lock() else {
        return Vec::new();
    };
    reg.order
        .iter()
        .filter_map(|id| reg.map.get(id).map(|s| s.meta.clone()))
        .collect()
}

/// Read rows `[offset, offset + limit)` of dataset `id`; `limit` is capped by
/// [`MAX_READ_LIMIT`]. Returns `None` if the id is unknown.
pub fn read(id: &str, offset: u64, limit: u32) -> Option<Page> {
    let registry = registry();
    let Ok(mut reg) = registry.lock() else {
        return None;
    };
    let stored = reg.map.get_mut(id)?;
    stored.last_access = Instant::now();
    let capped = limit.min(MAX_READ_LIMIT);

    match &mut stored.source {
        Source::Rows(rows) => {
            let total = rows.len() as u64;
            let start = offset.min(total) as usize;
            let end = (offset.saturating_add(capped as u64)).min(total) as usize;
            Some(Page {
                columns: stored.meta.columns.clone(),
                rows: rows[start..end].to_vec(),
                offset: start as u64,
                total,
            })
        }
        // Scanned on demand — only the requested window crosses.
        Source::Arrow(sheet) => {
            let total = sheet.total;
            let start = offset.min(total);
            let batches = sheet
                .loader
                .fetch(Vec::new(), Some(start as usize), Some(capped as usize))
                .ok()?;
            Some(Page {
                columns: sheet.columns.clone(),
                rows: crate::file::to_dataset::batches_to_dataset(&batches)
                    .map(|(_, rows)| rows)
                    .unwrap_or_default(),
                offset: start,
                total,
            })
        }
    }
}

/// Replace the columns + rows behind an existing handle in place (keeping its
/// id, source, and byte-budget accounting current). No-op unless the handle is
/// known and owned by `instance` — a producer can only mutate its own datasets.
pub fn update(instance: &str, id: &str, columns: Vec<DatasetColumn>, rows: Vec<Vec<String>>) {
    let registry = registry();
    if let Ok(mut reg) = registry.lock() {
        let Some(stored) = reg.map.get(id) else {
            return;
        };
        if stored.meta.source_instance != instance {
            return;
        }
        let revision = reg.next_rev();
        let Some(stored) = reg.map.get(id) else {
            return;
        };
        let meta = DatasetMeta {
            row_count: rows.len() as u64,
            columns,
            revision,
            ..stored.meta.clone()
        };
        let size = dataset_bytes(&meta, &rows);
        let old_size = stored.size;
        if let Some(stored) = reg.map.get_mut(id)
            && stored.source.rows().is_some()
        {
            stored.meta = meta;
            stored.source = Source::Rows(rows);
            stored.size = size;
            stored.last_access = Instant::now();
        }
        reg.bytes = reg.bytes.saturating_add(size).saturating_sub(old_size);
        // A larger dataset may push us over budget — evict to fit.
        reg.enforce_budget();
    }
}

/// Append rows to an existing handle, keeping its columns — for streaming
/// producers that push batches over time. Retains only the most recent
/// [`MAX_STREAM_ROWS`] (a ring buffer, so an unbounded stream stays bounded),
/// keeps the byte total and budget current. No-op unless the handle is known
/// and owned by `instance`.
pub fn append(instance: &str, id: &str, rows: Vec<Vec<String>>) {
    if rows.is_empty() {
        return;
    }
    let registry = registry();
    if let Ok(mut reg) = registry.lock() {
        match reg.map.get(id) {
            Some(s) if s.meta.source_instance == instance => {}
            _ => return,
        }
        let revision = reg.next_rev();
        let Some(stored) = reg.map.get_mut(id) else {
            return;
        };
        // Track bytes incrementally (add appended, subtract evicted) rather than
        // re-summing every retained row — an append-heavy stream would otherwise
        // be O(rows) per call.
        let added: usize = rows.iter().map(|r| row_bytes(r)).sum();
        // Streaming only applies to pushed sheets; a live engine has nothing to
        // append to.
        let Some(stored_rows) = stored.source.rows_mut() else {
            return;
        };
        stored_rows.extend(rows);
        // Ring-buffer: drop the oldest rows past the cap.
        let overflow = stored_rows.len().saturating_sub(MAX_STREAM_ROWS);
        let evicted: usize = if overflow > 0 {
            let e = stored_rows[..overflow].iter().map(|r| row_bytes(r)).sum();
            stored_rows.drain(..overflow);
            e
        } else {
            0
        };
        let retained = stored_rows.len() as u64;
        stored.meta.row_count = retained;
        stored.meta.revision = revision;
        let old_size = stored.size;
        stored.size = old_size.saturating_add(added).saturating_sub(evicted);
        let new_size = stored.size;
        stored.last_access = Instant::now();
        reg.bytes = reg.bytes.saturating_add(new_size).saturating_sub(old_size);
        reg.enforce_budget();
    }
}

/// Remove a dataset (idempotent). No-op unless the handle is owned by
/// `instance`, so a producer can only release its own datasets.
pub fn release(instance: &str, id: &str) {
    let registry = registry();
    if let Ok(mut reg) = registry.lock()
        && reg
            .map
            .get(id)
            .is_some_and(|s| s.meta.source_instance == instance)
    {
        reg.drop_dataset(id);
    }
}

/// Drop datasets whose producing instance is no longer open. Called each frame
/// with the set of live plugin-instance ids (same set signals uses).
pub fn retain_instances(open: &std::collections::HashSet<String>) {
    let registry = registry();
    if let Ok(mut reg) = registry.lock() {
        let dropped: Vec<String> = reg
            .map
            .values()
            .filter(|s| !open.contains(&s.meta.source_instance))
            .map(|s| s.meta.id.clone())
            .collect();
        for id in dropped {
            reg.drop_dataset(&id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    static TEST_LOCK: Mutex<()> = Mutex::new(());

    fn reset() -> std::sync::MutexGuard<'static, ()> {
        let guard = TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let registry = registry();
        if let Ok(mut reg) = registry.lock() {
            reg.map.clear();
            reg.order.clear();
            reg.bytes = 0;
            reg.seq = 0;
        }
        guard
    }

    fn col(name: &str) -> DatasetColumn {
        DatasetColumn {
            name: name.to_string(),
            type_hint: "text".to_string(),
        }
    }

    #[test]
    fn publish_replaces_same_instance() {
        let _g = reset();
        let first = publish(
            "p",
            "p#1",
            "a".into(),
            "k".into(),
            vec![],
            vec![col("x")],
            vec![],
        );
        let second = publish(
            "p",
            "p#1",
            "b".into(),
            "k".into(),
            vec![],
            vec![col("x")],
            vec![],
        );
        // The fresh publish from p#1 dropped its previous dataset.
        let metas = list();
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].name, "b");
        assert!(read(&first, 0, 10).is_none());
        assert!(read(&second, 0, 10).is_some());
    }

    #[test]
    fn update_replaces_rows_in_place() {
        let _g = reset();
        let id = publish(
            "p",
            "p#1",
            "a".into(),
            "k".into(),
            vec![],
            vec![col("x")],
            vec![vec!["1".into()]],
        );
        update(
            "p#1",
            &id,
            vec![col("x")],
            vec![vec!["1".into()], vec!["2".into()], vec!["3".into()]],
        );
        let page = read(&id, 0, 10).unwrap();
        assert_eq!(page.total, 3); // same handle, new rows
        assert_eq!(list().len(), 1);

        // A different instance can't mutate this dataset.
        update("other#1", &id, vec![col("x")], vec![]);
        assert_eq!(read(&id, 0, 10).unwrap().total, 3);
    }

    #[test]
    fn append_adds_rows_scoped_to_instance() {
        let _g = reset();
        let id = publish(
            "p",
            "p#1",
            "a".into(),
            "k".into(),
            vec![],
            vec![col("x")],
            vec![vec!["1".into()]],
        );
        append("p#1", &id, vec![vec!["2".into()], vec!["3".into()]]);
        assert_eq!(read(&id, 0, 10).unwrap().total, 3);
        // A different instance can't append.
        append("other#1", &id, vec![vec!["9".into()]]);
        assert_eq!(read(&id, 0, 10).unwrap().total, 3);
    }

    #[test]
    fn revision_bumps_on_every_mutation() {
        let _g = reset();
        let id = publish(
            "p",
            "p#1",
            "a".into(),
            "k".into(),
            vec![],
            vec![col("x")],
            vec![vec!["1".into()]],
        );
        let r0 = meta(&id).unwrap().revision;
        // In-place update with the SAME row count must still bump the revision.
        update("p#1", &id, vec![col("x")], vec![vec!["2".into()]]);
        let r1 = meta(&id).unwrap().revision;
        assert!(
            r1 > r0,
            "update should bump revision even at equal row count"
        );
        append("p#1", &id, vec![vec!["3".into()]]);
        let r2 = meta(&id).unwrap().revision;
        assert!(r2 > r1, "append should bump revision");
    }

    #[test]
    fn append_ring_buffers_past_cap() {
        let _g = reset();
        let id = publish_small("p#1", "a"); // seeds one row "1"
        let batch: Vec<Vec<String>> = (0..MAX_STREAM_ROWS).map(|i| vec![i.to_string()]).collect();
        append("p#1", &id, batch);
        let page = read(&id, 0, MAX_READ_LIMIT).unwrap();
        // Capped to MAX_STREAM_ROWS; the seed row was the oldest and got dropped.
        assert_eq!(page.total, MAX_STREAM_ROWS as u64);
        assert_eq!(page.rows[0][0], "0");
    }

    #[test]
    fn publish_list_read_paged() {
        let _g = reset();
        let rows: Vec<Vec<String>> = (0..10)
            .map(|i| vec![i.to_string(), format!("n{i}")])
            .collect();
        let id = publish(
            "com.thoth.seshat",
            "seshat#1",
            "orders".into(),
            "sql-result".into(),
            vec!["db".into()],
            vec![col("id"), col("name")],
            rows,
        );
        let metas = list();
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].name, "orders");
        assert_eq!(metas[0].row_count, 10);

        let page = read(&id, 3, 4).unwrap();
        assert_eq!(page.total, 10);
        assert_eq!(page.offset, 3);
        assert_eq!(page.rows.len(), 4);
        assert_eq!(page.rows[0][0], "3");
    }

    #[test]
    fn retain_drops_closed_producers() {
        let _g = reset();
        publish(
            "p",
            "p#1",
            "a".into(),
            "k".into(),
            vec![],
            vec![col("x")],
            vec![],
        );
        publish(
            "p",
            "p#2",
            "b".into(),
            "k".into(),
            vec![],
            vec![col("x")],
            vec![],
        );
        let open = std::collections::HashSet::from(["p#2".to_string()]);
        retain_instances(&open);
        let metas = list();
        assert_eq!(metas.len(), 1);
        assert_eq!(metas[0].name, "b");
    }

    fn publish_small(instance: &str, name: &str) -> String {
        publish(
            "p",
            instance,
            name.into(),
            "k".into(),
            vec![],
            vec![col("v")],
            vec![vec!["1".into()]],
        )
    }

    #[test]
    fn release_removes_dataset() {
        let _g = reset();
        let id = publish_small("p#1", "a");
        assert_eq!(list().len(), 1);
        // A different instance can't release it.
        release("other#1", &id);
        assert_eq!(list().len(), 1);
        release("p#1", &id);
        assert!(list().is_empty());
        assert!(read(&id, 0, 1).is_none());
        // Idempotent.
        release("p#1", &id);
    }

    #[test]
    fn count_cap_holds_at_max() {
        let _g = reset();
        for i in 0..(MAX_DATASETS + 5) {
            publish_small(&format!("p#{i}"), &format!("d{i}"));
        }
        assert_eq!(list().len(), MAX_DATASETS, "count cap enforced");
    }

    #[test]
    fn byte_budget_evicts_lru() {
        let _g = reset();
        // Each row alone is the whole budget, so publishing a second one forces
        // eviction of the older (least-recently-accessed) dataset.
        let big_row = || vec![vec!["x".repeat(MAX_BYTES)]];

        publish(
            "p",
            "p#1",
            "first".into(),
            "k".into(),
            vec![],
            vec![col("v")],
            big_row(),
        );
        publish(
            "p",
            "p#2",
            "second".into(),
            "k".into(),
            vec![],
            vec![col("v")],
            big_row(),
        );

        let metas = list();
        assert_eq!(metas.len(), 1, "over budget → only the survivor remains");
        assert_eq!(metas[0].name, "second");
    }

    use crate::file::loaders::DuckdbConnection;
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

    fn publish_file(lines: &str, instance: &str) -> (String, NamedTempFile) {
        let file = ndjson(lines);
        let engine = DuckdbConnection::open_path(file.path()).unwrap();
        let handle = publish_arrow("core", instance, "test".to_string(), Arc::new(engine))
            .expect("published");
        (handle, file)
    }

    #[test]
    fn an_engine_publishes_without_copying_its_rows() {
        let _guard = reset();
        let rows: String = (0..50_000).map(|i| format!("{{\"n\":{i}}}\n")).collect();
        let (handle, _file) = publish_file(&rows, "big");

        // Every row is addressable...
        assert_eq!(total(&handle), 50_000);
        // ...but nothing was materialized into the registry's byte budget.
        let registry = registry();
        let reg = registry.lock().unwrap();
        assert_eq!(reg.map.get(&handle).unwrap().size, 0);
    }

    #[test]
    fn a_read_returns_only_the_window_asked_for() {
        let _guard = reset();
        let rows: String = (0..10_000).map(|i| format!("{{\"n\":{i}}}\n")).collect();
        let (handle, _file) = publish_file(&rows, "window");

        let page = read(&handle, 500, 10).expect("page");
        assert_eq!(page.rows.len(), 10, "only the requested rows cross");
        assert_eq!(page.offset, 500);
        assert_eq!(page.total, 10_000, "the total still reflects the whole file");
        assert_eq!(page.rows[0][0], "500");
    }

    #[test]
    fn records_expand_from_the_schema_alone() {
        let _guard = reset();
        let (handle, _file) = publish_file("{\"a\":1,\"b\":\"x\"}\n", "schema");
        assert!(records_expandable(&handle));

        let kids = children(&handle, 0, "");
        assert_eq!(
            kids.iter().map(|n| n.label.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(kids[0].preview, "1");
    }

    #[test]
    fn nesting_survives_the_bus() {
        let _guard = reset();
        // The whole point of the Arrow path: a nested value stays a subtree
        // rather than collapsing to a string.
        let (handle, _file) =
            publish_file("{\"user\":{\"name\":\"ada\"},\"tags\":[\"x\",\"y\"]}\n", "nested");

        let kids = children(&handle, 0, "");
        assert_eq!(kids[0].kind, NodeKind::Struct);
        assert_eq!(kids[1].kind, NodeKind::List);
        assert_eq!(children(&handle, 0, "user")[0].preview, "\"ada\"");
        assert_eq!(children(&handle, 0, "tags").len(), 2);

        let json = node_json(&handle, 0, "user").unwrap();
        assert_eq!(json["name"], "ada");
    }

    #[test]
    fn pushed_rows_still_work_and_stay_flat() {
        let _guard = reset();
        let columns = vec![
            DatasetColumn {
                name: "n".to_string(),
                type_hint: "integer".to_string(),
            },
            DatasetColumn {
                name: "s".to_string(),
                type_hint: "text".to_string(),
            },
        ];
        let handle = publish(
            "plug",
            "inst-rows",
            "pushed".to_string(),
            "table".to_string(),
            vec![],
            columns,
            vec![vec!["1".to_string(), "x".to_string()]],
        );

        assert_eq!(total(&handle), 1);
        let kids = children(&handle, 0, "");
        assert_eq!(kids.len(), 2);
        assert_eq!(kids[0].preview, "1");
        assert_eq!(kids[0].kind, NodeKind::Leaf);
        // A pushed cell has no children — that path is flat by construction.
        assert!(children(&handle, 0, ".n").is_empty());

        let json = node_json(&handle, 0, "").unwrap();
        assert_eq!(json["n"], 1);
        assert_eq!(json["s"], "x");
    }

    #[test]
    fn appending_to_a_live_sheet_is_a_no_op() {
        let _guard = reset();
        let (handle, _file) = publish_file("{\"n\":1}\n", "no-append");
        append("no-append", &handle, vec![vec!["2".to_string()]]);
        assert_eq!(total(&handle), 1);
    }
}
