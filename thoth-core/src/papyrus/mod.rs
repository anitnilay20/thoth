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
//! - [`Source::Arrow`] — Arrow batches, either scanned on demand from a live
//!   query engine or owned after a plugin's `publish-arrow`. A live engine
//!   materializes nothing: a read fetches exactly the window asked for, so an
//!   open file of any size costs a window. A published batch sheet owns its
//!   rows, and that memory counts against the registry budget.
//!
//! The two differ in fidelity as well as size. Arrow carries real types, nulls
//! and nesting; pushed rows are strings with a per-column type hint, so nested
//! values arrive already flattened.

use std::collections::HashMap;
use std::sync::{Arc, LazyLock, Mutex, RwLock, Weak};
use std::time::Instant;

use duckdb::arrow::array::RecordBatch;
use duckdb::arrow::datatypes::SchemaRef;
use serde_json::Value;
use thoth_plugin_sdk::dataset::{NodeKind, TreeNode};

use crate::file::loaders::{
    BatchLoader, FileLoader, ParsedIpc, RecordWindow, TextIndex, arrow_tree,
};

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

#[derive(Clone, Debug, PartialEq, Eq)]
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
    /// Which relation of the producer this sheet holds, when it is one of
    /// several (a document's collection, a database's table).
    pub relation: Option<String>,
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
    /// Which cells were NULL, parallel to `rows`. Empty when the source cannot
    /// tell a NULL from an empty string — pushed rows and text lines both
    /// arrive as strings and have no nulls to report.
    pub nulls: Vec<Vec<bool>>,
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
    /// Arrow batches: a live engine scanned on demand, or a plugin's owned
    /// `publish-arrow` sheet.
    Arrow(Box<ArrowSheet>),
    /// A file shown as lines of text — the floor under every format. Holds a
    /// line index, not the text, so a file of any size costs its index.
    Text(Box<TextIndex>),
}

impl Source {
    fn rows_mut(&mut self) -> Option<&mut Vec<Vec<String>>> {
        match self {
            Source::Rows(rows) => Some(rows),
            _ => None,
        }
    }

    /// Whether this is a batch-backed Arrow sheet whose rows the registry owns.
    /// Live engine sheets scan their source on demand and stay read-only here.
    fn is_owned_arrow(&self) -> bool {
        matches!(self, Source::Arrow(sheet) if sheet.loader.is_owned())
    }

    /// Materialize an owned batch-backed sheet as string rows, keeping at most
    /// the most recent [`MAX_STREAM_ROWS`]. Live engine sheets return `None`.
    fn take_owned_rows(&mut self) -> Option<Vec<Vec<String>>> {
        match self {
            Source::Rows(rows) => Some(std::mem::take(rows)),
            Source::Arrow(sheet) if sheet.loader.is_owned() => {
                let total = sheet.total as usize;
                let keep = total.min(MAX_STREAM_ROWS);
                let start = total.saturating_sub(keep);
                let batches = sheet
                    .loader
                    .fetch(Vec::new(), Some(start), Some(keep))
                    .ok()?;
                crate::file::to_dataset::batches_to_rows(&batches)
            }
            _ => None,
        }
    }

    /// Bytes this source owns in the registry's budget. Only a batch-backed
    /// Arrow sheet owns rows; an engine sheet holds a window and costs nothing.
    fn owned_bytes(&self) -> usize {
        match self {
            Source::Arrow(sheet) => sheet.loader.owned_bytes(),
            _ => 0,
        }
    }
}

struct Stored {
    meta: DatasetMeta,
    source: Source,
    last_access: Instant,
    /// Estimated heap footprint of this dataset, tracked so the registry can
    /// enforce [`MAX_BYTES`] without re-summing every entry. A live engine
    /// sheet holds only a window and contributes nothing; a batch-backed Arrow
    /// sheet counts the `RecordBatch` memory it owns.
    size: usize,
    /// Consumers currently holding this handle. Pinned datasets are never
    /// evicted for budget; an explicit `release` still drops them.
    pins: usize,
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
    /// the byte budget. `keep` is the dataset being published or mutated: it is
    /// never evicted, so a publish survives even when every other dataset is
    /// pinned, sitting over budget until a pin is released. Keeps at least one
    /// dataset (guard on `len > 1`) so a single oversized dataset can't loop
    /// forever.
    fn enforce_budget(&mut self, keep: Option<&str>) {
        while self.order.len() > 1 && (self.order.len() > MAX_DATASETS || self.bytes > MAX_BYTES) {
            let Some(victim) = self
                .order
                .iter()
                .filter(|id| keep != Some(id.as_str()))
                .filter_map(|id| {
                    self.map
                        .get(id)
                        .filter(|s| s.pins == 0)
                        .map(|s| (id.clone(), s.last_access))
                })
                .min_by_key(|(_, last_access)| *last_access)
                .map(|(id, _)| id)
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
        .filter(|s| s.meta.source_instance == source_instance && s.meta.relation.is_none())
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
        relation: None,
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
            pins: 0,
        },
    );
    reg.order.push(id.clone());

    // Evict down to the count/byte budget. The dataset just published is kept:
    // it can sit over budget only while other datasets are pinned, and `unpin`
    // reclaims once they are released.
    reg.enforce_budget(Some(&id));
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
    let total = loader.len().ok()?;
    publish_arrow_with_total(source, instance, name, loader, total as u64)
}

/// Publish a live engine whose row count is already known.
///
/// `count(*)` over a JSON file is a full scan, so a caller that computed it off
/// the UI thread must not be made to pay for it again here.
pub fn publish_arrow_with_total(
    source: &str,
    instance: &str,
    name: String,
    loader: Arc<dyn FileLoader + Send + Sync>,
    total: u64,
) -> Option<String> {
    let columns: Vec<DatasetColumn> = {
        use crate::file::loaders::RecordSource;
        loader
            .columns()
            .unwrap_or_default()
            .into_iter()
            .map(|(name, type_hint)| DatasetColumn { name, type_hint })
            .collect()
    };

    Some(insert_sheet(
        source,
        instance,
        name,
        "file".to_string(),
        columns.clone(),
        total,
        Source::Arrow(Box::new(ArrowSheet {
            loader,
            window: RecordWindow::default(),
            total,
            columns,
        })),
    ))
}

/// Register a sheet the host owns, replacing any previous one from the same
/// instance. An index- or engine-backed sheet carries no byte weight; a
/// batch-backed Arrow sheet counts the batches it owns.
fn insert_sheet(
    source: &str,
    instance: &str,
    name: String,
    kind: String,
    columns: Vec<DatasetColumn>,
    total: u64,
    sheet: Source,
) -> String {
    let registry = registry();
    let Ok(mut reg) = registry.lock() else {
        return String::new();
    };
    let stale: Vec<String> = reg
        .map
        .values()
        .filter(|s| s.meta.source_instance == instance && s.meta.relation.is_none())
        .map(|s| s.meta.id.clone())
        .collect();
    for id in stale {
        reg.drop_dataset(&id);
    }

    let size = sheet.owned_bytes();
    reg.seq += 1;
    let revision = reg.next_rev();
    let id = format!("ds-{}", reg.seq);
    reg.map.insert(
        id.clone(),
        Stored {
            meta: DatasetMeta {
                id: id.clone(),
                name,
                source_plugin: source.to_string(),
                source_instance: instance.to_string(),
                kind,
                tags: Vec::new(),
                relation: None,
                row_count: total,
                columns,
                revision,
            },
            source: sheet,
            last_access: Instant::now(),
            size,
            pins: 0,
        },
    );
    reg.order.push(id.clone());
    reg.bytes = reg.bytes.saturating_add(size);
    reg.enforce_budget(Some(&id));
    id
}

/// Publish a file as lines of text, returning its handle.
///
/// The fallback when no reader can make sense of a file — the index is already
/// built, so this costs nothing beyond it and the file becomes browsable at any
/// size.
pub fn publish_text(
    source: &str,
    instance: &str,
    name: String,
    index: TextIndex,
) -> Option<String> {
    let total = index.len() as u64;
    let columns = vec![
        DatasetColumn {
            name: "line".to_string(),
            type_hint: "integer".to_string(),
        },
        DatasetColumn {
            name: "text".to_string(),
            type_hint: "text".to_string(),
        },
    ];
    Some(insert_sheet(
        source,
        instance,
        name,
        "text".to_string(),
        columns,
        total,
        Source::Text(Box::new(index)),
    ))
}

/// Publish one named relation of an engine as its own sheet, returning its
/// handle.
///
/// A file tab can hold several relations (a document's collections, a
/// database's tables). The primary handle always points at whichever one the
/// grid shows; a chart or renderer that asks for a different relation gets a
/// stable sheet of its own here, still scanned lazily by the engine.
pub fn publish_arrow_relation(
    source: &str,
    instance: &str,
    name: String,
    relation: String,
    loader: Arc<dyn FileLoader + Send + Sync>,
) -> Option<String> {
    use crate::file::loaders::{RecordSource, RelationLoader};

    let scoped: Arc<dyn FileLoader + Send + Sync> =
        Arc::new(RelationLoader::new(loader, relation.clone()));
    let total = scoped.len().ok()? as u64;
    let columns: Vec<DatasetColumn> = scoped
        .columns()
        .unwrap_or_default()
        .into_iter()
        .map(|(name, type_hint)| DatasetColumn { name, type_hint })
        .collect();

    Some(insert_relation(
        source, instance, name, relation, columns, total, scoped,
    ))
}

/// Publish an in-memory Arrow dataset (a plugin's `publish-arrow`).
///
/// The IPC stream was parsed once by the host; the batches are served as a
/// window like any engine-backed sheet, so types, nulls and nesting survive.
pub fn publish_batches(source: &str, instance: &str, name: String, parsed: ParsedIpc) -> String {
    let loader: Arc<dyn FileLoader + Send + Sync> =
        Arc::new(BatchLoader::new(parsed.schema, parsed.batches));
    let total = loader.len().unwrap_or(0) as u64;
    publish_arrow_with_total(source, instance, name, loader, total).unwrap_or_default()
}

/// Register one relation of an instance, replacing only a previous sheet for
/// the *same* relation. Other relations of that instance stay live.
fn insert_relation(
    source: &str,
    instance: &str,
    name: String,
    relation: String,
    columns: Vec<DatasetColumn>,
    total: u64,
    loader: Arc<dyn FileLoader + Send + Sync>,
) -> String {
    let registry = registry();
    let Ok(mut reg) = registry.lock() else {
        return String::new();
    };

    let stale: Vec<String> = reg
        .map
        .values()
        .filter(|s| {
            s.meta.source_instance == instance
                && s.meta.relation.as_deref() == Some(relation.as_str())
        })
        .map(|s| s.meta.id.clone())
        .collect();
    for id in stale {
        reg.drop_dataset(&id);
    }

    reg.seq += 1;
    let revision = reg.next_rev();
    let id = format!("ds-{}", reg.seq);
    reg.map.insert(
        id.clone(),
        Stored {
            meta: DatasetMeta {
                id: id.clone(),
                name,
                source_plugin: source.to_string(),
                source_instance: instance.to_string(),
                kind: "file".to_string(),
                tags: Vec::new(),
                relation: Some(relation),
                row_count: total,
                columns: columns.clone(),
                revision,
            },
            source: Source::Arrow(Box::new(ArrowSheet {
                loader,
                window: RecordWindow::default(),
                total,
                columns,
            })),
            last_access: Instant::now(),
            size: 0,
            pins: 0,
        },
    );
    reg.order.push(id.clone());
    // A relation sheet is Arrow-backed (size 0), but it still counts toward the
    // dataset-count budget, so evict after registering it (never this one).
    reg.enforce_budget(Some(&id));
    id
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
        Source::Text(index) => index.len() as u64,
    })
    .unwrap_or(0)
}

/// Whether records have any children to expand.
///
/// Answered from the schema, never from the data — which is what makes listing
/// a million collapsed records free.
pub fn records_expandable(id: &str) -> bool {
    with_sheet(id, |stored| match &stored.source {
        // Answered from the schema, never the data.
        Source::Arrow(sheet) => !sheet.columns.is_empty(),
        Source::Rows(_) => !stored.meta.columns.is_empty(),
        // A line of text is a leaf. Claiming otherwise renders every row with
        // a caret that opens onto nothing.
        Source::Text(_) => false,
    })
    .unwrap_or(false)
}

/// Children of the node at `rel` within record `root`.
pub fn children(id: &str, root: u64, rel: &str) -> Vec<TreeNode> {
    with_sheet(id, |stored| match &mut stored.source {
        Source::Arrow(sheet) => match sheet.window.locate(&*sheet.loader, root as usize) {
            Ok((batches, local)) => arrow_tree::children(batches, local, rel),
            Err(_) => Vec::new(),
        },
        // A line of text has no structure to walk into.
        Source::Text(_) => Vec::new(),
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
        Source::Text(index) => index
            .read(root as usize, 1)
            .ok()
            .and_then(|l| l.into_iter().next())
            .unwrap_or_default(),
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
        Source::Text(index) => index
            .read(root as usize, 1)
            .ok()
            .and_then(|l| l.into_iter().next())
            .map(Value::String),
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
fn cell_preview(cell: &str, type_hint: &str) -> (String, thoth_plugin_sdk::TextToken) {
    use thoth_plugin_sdk::TextToken;
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

/// Highest revision among a producer's primary sheets (its displayed dataset).
///
/// Chart Studio uses this as a signal that a plugin's own dataset changed, so
/// it can re-pull the pull-only handle it charts from.
pub fn instance_revision(instance: &str) -> Option<u64> {
    let registry = registry();
    let reg = registry.lock().ok()?;
    reg.map
        .values()
        .filter(|s| s.meta.source_instance == instance && s.meta.relation.is_none())
        .map(|s| s.meta.revision)
        .max()
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
                nulls: Vec::new(),
                offset: start as u64,
                total,
            })
        }
        // Seek straight to the visible lines.
        Source::Text(index) => {
            let total = index.len() as u64;
            let start = offset.min(total);
            let lines = index.read(start as usize, capped as usize).ok()?;
            Some(Page {
                columns: stored.meta.columns.clone(),
                rows: lines
                    .into_iter()
                    .enumerate()
                    .map(|(i, text)| vec![(start as usize + i + 1).to_string(), text])
                    .collect(),
                nulls: Vec::new(),
                offset: start,
                total,
            })
        }
        // Scanned on demand — only the requested window crosses, and only
        // once. Through the sheet's `RecordWindow`, which is what makes a
        // repeated read free: the grid resolves its page on *every frame* it
        // is drawn, and going straight to `loader.fetch` re-ran the query each
        // time — for a grouped result, the whole aggregation, sixty times a
        // second. The window was already here and simply was not used.
        Source::Arrow(sheet) => {
            let total = sheet.total;
            let start = offset.min(total);
            // Destructured so the window can be borrowed mutably while the
            // loader beside it is borrowed shared.
            let ArrowSheet {
                loader,
                window,
                columns,
                ..
            } = &mut **sheet;
            let (rows, nulls) = window
                .cells(loader.as_ref(), start as usize, capped as usize)
                .ok()?;
            Some(Page {
                columns: columns.clone(),
                rows,
                nulls,
                offset: start,
                total,
            })
        }
    }
}

/// A window of a handle as Arrow record batches, plus the sheet's schema.
///
/// Only Arrow-fidelity sheets (engine-backed, or a plugin's `publish-arrow`)
/// answer this; string-only pushes and text lines return `None`, and callers
/// fall back to [`read`].
///
/// The schema is returned separately because an empty window (a zero limit, or
/// an offset past the end) has no batch to carry it. `None` means the sheet is
/// Arrow-native but cannot supply a schema, e.g. an empty published batch set.
pub fn read_arrow(
    id: &str,
    offset: u64,
    limit: u32,
) -> Option<(Option<SchemaRef>, Vec<RecordBatch>)> {
    let registry = registry();
    let mut reg = registry.lock().ok()?;
    let stored = reg.map.get_mut(id)?;
    stored.last_access = Instant::now();

    let count = limit.min(MAX_READ_LIMIT) as usize;
    let start = offset as usize;
    let Source::Arrow(sheet) = &mut stored.source else {
        return None;
    };
    let ArrowSheet { loader, window, .. } = &mut **sheet;

    window.ensure(loader.as_ref(), start, count).ok()?;
    let (batches, local) = window.locate(loader.as_ref(), start).ok()?;

    let mut out = Vec::new();
    let mut need = count;
    let mut skip = local;
    for batch in batches {
        if need == 0 {
            break;
        }
        let rows = batch.num_rows();
        if skip >= rows {
            skip -= rows;
            continue;
        }
        let take = (rows - skip).min(need);
        out.push(batch.slice(skip, take));
        need -= take;
        skip = 0;
    }
    // Prefer a batch's schema (no extra fetch); fall back to the loader for an
    // empty window.
    let schema = out
        .first()
        .map(|batch| batch.schema())
        .or_else(|| loader.schema());
    Some((schema, out))
}

/// Replace the columns + rows behind an existing handle in place (keeping its
/// id, source, and byte-budget accounting current). No-op unless the handle is
/// known and owned by `instance` — a producer can only mutate its own datasets.
pub fn update(instance: &str, id: &str, columns: Vec<DatasetColumn>, rows: Vec<Vec<String>>) {
    let registry = registry();
    let Ok(mut reg) = registry.lock() else {
        return;
    };

    // Re-pulling a producer with unchanged data must not bump the revision, or
    // every Chart Studio refresh would look like new data. An owned batch sheet
    // is mutable through this string ABI; a live engine sheet is not.
    let unchanged = {
        let Some(stored) = reg.map.get(id) else {
            return;
        };
        if stored.meta.source_instance != instance {
            return;
        }
        match &stored.source {
            Source::Rows(existing) => stored.meta.columns == columns && *existing == rows,
            Source::Arrow(_) if stored.source.is_owned_arrow() => false,
            _ => return,
        }
    };
    if unchanged {
        if let Some(stored) = reg.map.get_mut(id) {
            stored.last_access = Instant::now();
        }
        return;
    }

    let revision = reg.next_rev();
    let (old_size, size) = {
        let Some(stored) = reg.map.get_mut(id) else {
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
        stored.meta = meta;
        // A string update replaces an owned batch sheet with string rows; the
        // handle stays stable, which is what a `data-view` holds.
        stored.source = Source::Rows(rows);
        stored.size = size;
        stored.last_access = Instant::now();
        (old_size, size)
    };
    reg.bytes = reg.bytes.saturating_add(size).saturating_sub(old_size);
    // A larger dataset may push us over budget — evict to fit, but never the
    // dataset just updated.
    reg.enforce_budget(Some(id));
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

        // An owned batch sheet is mutable through this string ABI: materialize
        // its recent rows and continue as a string sheet. A live engine sheet
        // stays read-only.
        if !matches!(&stored.source, Source::Rows(_)) {
            let Some(mut existing) = stored.source.take_owned_rows() else {
                return;
            };
            existing.extend(rows);
            let overflow = existing.len().saturating_sub(MAX_STREAM_ROWS);
            if overflow > 0 {
                existing.drain(..overflow);
            }
            let (old_size, size) = {
                stored.meta.row_count = existing.len() as u64;
                stored.meta.revision = revision;
                let size = dataset_bytes(&stored.meta, &existing);
                let old_size = stored.size;
                stored.source = Source::Rows(existing);
                stored.size = size;
                stored.last_access = Instant::now();
                (old_size, size)
            };
            reg.bytes = reg.bytes.saturating_add(size).saturating_sub(old_size);
            reg.enforce_budget(Some(id));
            return;
        }

        // Track bytes incrementally (add appended, subtract evicted) rather than
        // re-summing every retained row — an append-heavy stream would otherwise
        // be O(rows) per call.
        let added: usize = rows.iter().map(|r| row_bytes(r)).sum();
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
        // Never evict the dataset just appended to.
        reg.enforce_budget(Some(id));
    }
}

/// Pin a handle so budget eviction cannot drop it while a consumer reads it.
///
/// Returns `false` for an unknown handle. Pins are reference-counted; every
/// successful `pin` needs a matching [`unpin`].
pub fn pin(id: &str) -> bool {
    let registry = registry();
    let Ok(mut reg) = registry.lock() else {
        return false;
    };
    let Some(stored) = reg.map.get_mut(id) else {
        return false;
    };
    stored.pins += 1;
    stored.last_access = Instant::now();
    true
}

/// Release one pin. Returns `false` for an unknown handle.
pub fn unpin(id: &str) -> bool {
    let registry = registry();
    let Ok(mut reg) = registry.lock() else {
        return false;
    };
    {
        let Some(stored) = reg.map.get_mut(id) else {
            return false;
        };
        stored.pins = stored.pins.saturating_sub(1);
        stored.last_access = Instant::now();
    }
    // Releasing a pin may put the registry over budget; reclaim now rather than
    // waiting for the next publish.
    reg.enforce_budget(None);
    true
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

/// Whether a sheet's source instance is still open.
///
/// Chart Studio pulls a plugin producer into a separate `#pull` sheet so it
/// does not replace the plugin's own displayed dataset; that sheet follows the
/// same tab lifecycle as its producer.
fn instance_is_open(instance: &str, open: &std::collections::HashSet<String>) -> bool {
    open.contains(instance)
        || instance
            .strip_suffix("#pull")
            .is_some_and(|base| open.contains(base))
}

/// Drop datasets whose producing instance is no longer open. Called each frame
/// with the set of live plugin-instance ids (same set signals uses).
pub fn retain_instances(open: &std::collections::HashSet<String>) {
    let registry = registry();
    if let Ok(mut reg) = registry.lock() {
        let dropped: Vec<String> = reg
            .map
            .values()
            .filter(|s| !instance_is_open(&s.meta.source_instance, open))
            .map(|s| s.meta.id.clone())
            .collect();
        for id in dropped {
            reg.drop_dataset(&id);
        }
    }
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// The registry is process-wide, and these tests assert exact `list()`
    /// lengths — so anything that publishes into it takes turns.
    static TEST_LOCK: Mutex<()> = Mutex::new(());

    /// Take the registry for the duration of a test, without clearing it.
    ///
    /// For tests elsewhere that *publish* — opening a file does — which would
    /// otherwise race the counting assertions here.
    #[allow(dead_code)]
    pub(crate) fn exclusive() -> std::sync::MutexGuard<'static, ()> {
        TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner())
    }

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

    fn ndjson_temp(contents: &str) -> tempfile::NamedTempFile {
        use std::io::Write as _;
        let mut file = tempfile::NamedTempFile::new().unwrap();
        file.write_all(contents.as_bytes()).unwrap();
        file.flush().unwrap();
        file
    }

    #[test]
    fn arrow_ipc_publish_round_trips_types_and_pages() {
        use crate::file::loaders::{encode_ipc, parse_ipc};
        use duckdb::arrow::array::{Int64Array, StringArray};
        use duckdb::arrow::datatypes::{DataType, Field, Schema};
        use duckdb::arrow::record_batch::RecordBatch;

        let _g = reset();
        let schema = Arc::new(Schema::new(vec![
            Field::new("id", DataType::Int64, false),
            Field::new("name", DataType::Utf8, false),
        ]));
        let batch = RecordBatch::try_new(
            schema,
            vec![
                Arc::new(Int64Array::from(vec![1, 2, 3])),
                Arc::new(StringArray::from(vec!["a", "b", "c"])),
            ],
        )
        .unwrap();

        let bytes = encode_ipc(std::slice::from_ref(&batch)).unwrap();
        let parsed = parse_ipc(&bytes).unwrap();
        let handle = publish_batches("p", "p#ipc", "ipc".into(), parsed);

        assert_eq!(total(&handle), 3);
        assert_eq!(meta(&handle).unwrap().columns[0].type_hint, "integer");
        assert_eq!(
            read(&handle, 1, 10).unwrap().rows,
            vec![
                vec!["2".to_string(), "b".to_string()],
                vec!["3".to_string(), "c".to_string()],
            ]
        );

        {
            let registry = registry();
            let reg = registry.lock().unwrap();
            assert!(
                reg.map.get(&handle).unwrap().size > 0,
                "owned batches count against the byte budget"
            );
        }

        let (schema, window) = read_arrow(&handle, 1, 2).unwrap();
        assert!(schema.is_some(), "a nonempty window carries its schema");
        let decoded = parse_ipc(&encode_ipc(&window).unwrap()).unwrap();
        assert_eq!(
            decoded
                .batches
                .iter()
                .map(RecordBatch::num_rows)
                .sum::<usize>(),
            2,
            "the typed window carries exactly the requested rows"
        );

        // Past the end there are no batches, but the stream must still carry a
        // schema so Arrow readers can open it.
        let (schema, empty) = read_arrow(&handle, 3, 1).unwrap();
        assert!(empty.is_empty());
        let schema = schema.expect("an empty window still has a schema");
        let bytes = crate::file::loaders::encode_ipc_with_schema(schema.as_ref(), &empty).unwrap();
        assert!(
            parse_ipc(&bytes).unwrap().batches.is_empty(),
            "a schema with no batches decodes to no rows"
        );

        release("p#ipc", &handle);
    }

    #[test]
    fn string_mutations_apply_to_an_owned_batch_sheet() {
        use crate::file::loaders::{encode_ipc, parse_ipc};
        use duckdb::arrow::array::Int64Array;
        use duckdb::arrow::datatypes::{DataType, Field, Schema};
        use duckdb::arrow::record_batch::RecordBatch;

        let _g = reset();
        let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int64, false)]));
        let batch =
            RecordBatch::try_new(schema, vec![Arc::new(Int64Array::from(vec![1, 2]))]).unwrap();
        let handle = publish_batches(
            "p",
            "p#mut",
            "mut".into(),
            parse_ipc(&encode_ipc(std::slice::from_ref(&batch)).unwrap()).unwrap(),
        );

        update("p#mut", &handle, vec![col("id")], vec![vec!["9".into()]]);
        assert_eq!(
            read(&handle, 0, 10).unwrap().rows,
            vec![vec!["9".to_string()]]
        );
        assert!(
            read_arrow(&handle, 0, 10).is_none(),
            "a string update turns the batch sheet into a string sheet"
        );

        append("p#mut", &handle, vec![vec!["10".into()]]);
        assert_eq!(
            read(&handle, 0, 10).unwrap().rows,
            vec![vec!["9".to_string()], vec!["10".to_string()]]
        );

        release("p#mut", &handle);
    }

    #[test]
    fn string_mutations_leave_a_live_engine_sheet_alone() {
        let _g = reset();
        let (handle, _file) = publish_file("{\"n\":1}\n", "engine-mut");

        update(
            "engine-mut",
            &handle,
            vec![col("n")],
            vec![vec!["9".into()]],
        );
        append("engine-mut", &handle, vec![vec!["9".into()]]);

        assert_eq!(
            read(&handle, 0, 10).unwrap().rows,
            vec![vec!["1".to_string()]],
            "a live engine is not mutated by the string ABI"
        );
        assert!(
            read_arrow(&handle, 0, 10).is_some(),
            "the engine sheet stays Arrow-native"
        );

        release("engine-mut", &handle);
    }

    #[test]
    fn a_schema_only_batch_stream_keeps_its_columns() {
        use crate::file::loaders::{encode_ipc_with_schema, parse_ipc};
        use duckdb::arrow::datatypes::{DataType, Field, Schema};

        let _g = reset();
        let schema = Arc::new(Schema::new(vec![Field::new("id", DataType::Int64, false)]));
        let bytes = encode_ipc_with_schema(schema.as_ref(), &[]).unwrap();
        let parsed = parse_ipc(&bytes).unwrap();
        assert!(parsed.batches.is_empty(), "the stream carries no batches");
        let handle = publish_batches("p", "p#schema", "schema".into(), parsed);

        let meta = meta(&handle).unwrap();
        assert_eq!(meta.columns.len(), 1, "the schema survives an empty stream");
        assert_eq!(meta.columns[0].name, "id");
        assert_eq!(meta.columns[0].type_hint, "integer");

        let page = read(&handle, 0, 10).expect("an empty page is still a page");
        assert!(page.rows.is_empty());
        assert_eq!(page.columns[0].name, "id");

        // read-arrow can hand the schema back even with no rows.
        let (schema, batches) = read_arrow(&handle, 0, 10).unwrap();
        assert!(batches.is_empty());
        let schema = schema.expect("a schema-only sheet still has a schema");
        assert!(
            !encode_ipc_with_schema(schema.as_ref(), &batches)
                .unwrap()
                .is_empty(),
            "an empty read still encodes a schema-bearing stream"
        );

        // A later append keeps the columns and starts delivering rows.
        append("p#schema", &handle, vec![vec!["1".into()]]);
        let page = read(&handle, 0, 10).unwrap();
        assert_eq!(page.columns[0].name, "id");
        assert_eq!(page.rows, vec![vec!["1".to_string()]]);

        release("p#schema", &handle);
    }

    #[test]
    fn read_does_not_turn_a_formatting_failure_into_an_empty_page() {
        use crate::file::loaders::{encode_ipc, parse_ipc};
        use duckdb::arrow::array::Date32Array;
        use duckdb::arrow::datatypes::{DataType, Field, Schema};

        let _g = reset();
        let schema = Arc::new(Schema::new(vec![Field::new(
            "day",
            DataType::Date32,
            false,
        )]));
        let batch = RecordBatch::try_new(schema, vec![Arc::new(Date32Array::from(vec![i32::MAX]))])
            .unwrap();
        let parsed = parse_ipc(&encode_ipc(std::slice::from_ref(&batch)).unwrap()).unwrap();
        let handle = publish_batches("p", "p#bad-date", "bad".into(), parsed);

        assert!(
            read(&handle, 0, 10).is_none(),
            "a formatting failure must not be returned as a page"
        );
        assert!(
            read_arrow(&handle, 0, 10).is_some(),
            "the typed source is still available through Arrow"
        );

        release("p#bad-date", &handle);
    }

    #[test]
    fn relations_of_one_instance_get_independent_handles() {
        use crate::file::loaders::DuckdbConnection;

        let _g = reset();
        let a = ndjson_temp("{\"id\":1}\n{\"id\":2}\n");
        let b = ndjson_temp("{\"id\":10}\n{\"id\":20}\n{\"id\":30}\n");
        let db = Arc::new(DuckdbConnection::new().unwrap());
        db.open(&a.path().to_string_lossy(), "a").unwrap();
        db.open(&b.path().to_string_lossy(), "b").unwrap();

        let engine: Arc<dyn FileLoader + Send + Sync> = db;
        let h1 = publish_arrow_relation("core", "core#rel", "a".into(), "a".into(), engine.clone())
            .unwrap();
        let h2 = publish_arrow_relation("core", "core#rel", "b".into(), "b".into(), engine.clone())
            .unwrap();

        assert_ne!(h1, h2, "each relation gets its own handle");
        assert_eq!(total(&h1), 2);
        assert_eq!(total(&h2), 3);
        assert_eq!(read(&h2, 0, 10).unwrap().rows.len(), 3);
        assert_eq!(
            meta(&h1).unwrap().columns[0].type_hint,
            "integer",
            "Arrow types reach the handle"
        );

        // Re-publishing one relation replaces only that relation.
        let h1b =
            publish_arrow_relation("core", "core#rel", "a".into(), "a".into(), engine).unwrap();
        assert_ne!(h1b, h1);
        assert!(meta(&h1).is_none(), "the replaced relation is dropped");
        assert!(meta(&h2).is_some(), "the other relation stays live");

        // A primary-sheet republish removes only the primary, so named
        // relations stay readable.
        let primary = publish(
            "core",
            "core#rel",
            "primary".into(),
            "file".into(),
            vec![],
            vec![col("x")],
            vec![vec!["x".into()]],
        );
        assert!(
            meta(&h2).is_some(),
            "a primary republish must not drop named relations"
        );

        release("core#rel", &primary);
        release("core#rel", &h1b);
        release("core#rel", &h2);
    }

    #[test]
    fn pull_sheets_follow_their_producer() {
        let _g = reset();
        let handle = publish(
            "p",
            "p#1#pull",
            "pull".into(),
            "dataset".into(),
            vec![],
            vec![col("x")],
            vec![vec!["x".into()]],
        );

        retain_instances(&std::collections::HashSet::from(["p#1".to_string()]));
        assert!(
            meta(&handle).is_some(),
            "a #pull sheet follows its producer instance"
        );

        retain_instances(&std::collections::HashSet::new());
        assert!(meta(&handle).is_none());
    }

    #[test]
    fn identical_update_keeps_revision_and_changes_bump_it() {
        let _g = reset();
        let handle = publish(
            "p",
            "p#upd",
            "d".into(),
            "k".into(),
            vec![],
            vec![col("x")],
            vec![vec!["1".into()]],
        );
        let revision = meta(&handle).unwrap().revision;

        update("p#upd", &handle, vec![col("x")], vec![vec!["1".into()]]);
        assert_eq!(
            meta(&handle).unwrap().revision,
            revision,
            "an identical update must keep the revision"
        );

        update("p#upd", &handle, vec![col("x")], vec![vec!["2".into()]]);
        assert!(
            meta(&handle).unwrap().revision > revision,
            "changed content must bump the revision"
        );

        release("p#upd", &handle);
    }

    #[test]
    fn instance_revision_tracks_the_displayed_primary() {
        let _g = reset();
        let handle = publish(
            "p",
            "p#inst",
            "d".into(),
            "k".into(),
            vec![],
            vec![col("x")],
            vec![vec!["1".into()]],
        );
        let first = instance_revision("p#inst");
        assert_eq!(first, Some(meta(&handle).unwrap().revision));

        update("p#inst", &handle, vec![col("x")], vec![vec!["2".into()]]);
        assert!(instance_revision("p#inst").unwrap() > first.unwrap());

        release("p#inst", &handle);
    }

    #[test]
    fn pinned_datasets_survive_budget_pressure() {
        let _g = reset();
        let pinned = publish(
            "p",
            "p#pin",
            "pinned".into(),
            "k".into(),
            vec![],
            vec![col("x")],
            vec![vec!["a".into()]],
        );
        assert!(pin(&pinned));

        for i in 0..(MAX_DATASETS + 5) {
            publish(
                "p",
                &format!("p#{i}"),
                format!("d{i}"),
                "k".into(),
                vec![],
                vec![col("x")],
                vec![vec!["a".into()]],
            );
        }

        assert!(
            meta(&pinned).is_some(),
            "a pinned handle must not be evicted for budget"
        );
        assert!(unpin(&pinned));
    }

    #[test]
    fn a_publish_is_kept_when_every_other_dataset_is_pinned() {
        let _g = reset();
        for i in 0..MAX_DATASETS {
            let id = publish(
                "p",
                &format!("p#{i}"),
                format!("d{i}"),
                "k".into(),
                vec![],
                vec![col("x")],
                vec![vec!["a".into()]],
            );
            assert!(pin(&id));
        }

        // One over the count budget, with every existing dataset pinned: the
        // new dataset must be retained (temporarily over budget), not evicted.
        let fresh = publish(
            "p",
            "p#fresh",
            "fresh".into(),
            "k".into(),
            vec![],
            vec![col("x")],
            vec![vec!["a".into()]],
        );
        assert!(
            meta(&fresh).is_some(),
            "the dataset being published must not be its own eviction victim"
        );
        assert_eq!(list().len(), MAX_DATASETS + 1);
    }

    #[test]
    fn unpinning_reclaims_budget_immediately() {
        let _g = reset();
        let mut ids = Vec::new();
        for i in 0..MAX_DATASETS {
            let id = publish(
                "p",
                &format!("p#{i}"),
                format!("d{i}"),
                "k".into(),
                vec![],
                vec![col("x")],
                vec![vec!["a".into()]],
            );
            assert!(pin(&id));
            ids.push(id);
        }
        // Retained over budget because everything else is pinned, then pinned
        // itself so releasing any one of the others is the only way down.
        let fresh = publish(
            "p",
            "p#fresh",
            "fresh".into(),
            "k".into(),
            vec![],
            vec![col("x")],
            vec![vec!["a".into()]],
        );
        assert!(pin(&fresh));
        assert_eq!(list().len(), MAX_DATASETS + 1);

        assert!(unpin(&ids[0]));
        assert_eq!(
            list().len(),
            MAX_DATASETS,
            "releasing a pin must enforce the budget immediately"
        );
        assert!(
            meta(&ids[0]).is_none(),
            "the released dataset is the eviction victim"
        );
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
        assert_eq!(
            page.total, 10_000,
            "the total still reflects the whole file"
        );
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
        let (handle, _file) = publish_file(
            "{\"user\":{\"name\":\"ada\"},\"tags\":[\"x\",\"y\"]}\n",
            "nested",
        );

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

    #[test]
    fn a_file_with_no_reader_still_opens_as_text() {
        let _guard = reset();
        // The shape that defeats every structured reader: one enormous JSON
        // object. It must still be openable, and reads must stay windowed.
        let mut tmp = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
        writeln!(tmp, "{{").unwrap();
        for i in 0..5000 {
            writeln!(tmp, "  \"key_{i}\": {i},").unwrap();
        }
        writeln!(tmp, "  \"last\": 0\n}}").unwrap();
        tmp.flush().unwrap();

        let index = TextIndex::build(tmp.path()).unwrap();
        let handle =
            publish_text("core", "inst-text", "big.json".to_string(), index).expect("published");

        assert_eq!(total(&handle), 5003);
        // A window returns line numbers and their text, and nothing else.
        let page = read(&handle, 100, 3).unwrap();
        assert_eq!(page.rows.len(), 3);
        assert_eq!(page.rows[0][0], "101");
        assert!(page.rows[0][1].contains("key_99"));
        assert_eq!(page.total, 5003);
        // Lines are leaves — there is nothing to expand into.
        assert!(!records_expandable(&handle) || children(&handle, 0, "").is_empty());
    }
}
