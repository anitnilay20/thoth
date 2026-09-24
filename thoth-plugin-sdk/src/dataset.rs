//! Host-installed resolver that lets a [`DataView`](crate::components::DataView)
//! render node read dataset rows **by handle** from the host's single-owned
//! registry. The host owns the data; a plugin only ever holds the handle it got
//! from `dataset-bus.publish` and embeds in a `data-view` node — the rows never
//! enter the plugin's memory.

use std::sync::OnceLock;

/// A resolved column: display name + SQL-ish type hint (drives table
/// alignment / colour via `ColumnType::from_sql`).
#[derive(Clone, Debug)]
pub struct DatasetColumn {
    /// Display name.
    pub name: String,
    /// SQL-ish type hint (e.g. "integer", "text").
    pub type_hint: String,
}

/// A page of a dataset resolved from the host registry.
#[derive(Clone, Debug)]
pub struct DatasetPage {
    /// Column schema.
    pub columns: Vec<DatasetColumn>,
    /// Row-major string cells (length ≤ `total`; the page may be capped).
    pub rows: Vec<Vec<String>>,
    /// Which cells of [`rows`](DatasetPage::rows) were NULL, in the same shape.
    ///
    /// A NULL formats as the empty string, so without this a field the record
    /// does not carry is indistinguishable from one that carries `""` — and
    /// with mixed record shapes in one file, that difference is most of what
    /// the grid is there to show. Empty (or short) means "not known", which
    /// reads every cell as present: a source that cannot tell them apart says
    /// nothing rather than guessing.
    pub nulls: Vec<Vec<bool>>,
    /// Total rows available (the page may be capped).
    pub total: u64,
}

impl DatasetPage {
    /// Whether the cell at `row`/`col` was NULL in the source.
    pub fn is_null(&self, row: usize, col: usize) -> bool {
        self.nulls
            .get(row)
            .and_then(|r| r.get(col))
            .copied()
            .unwrap_or(false)
    }
}

/// `(handle, row limit) -> page`. Installed by the host.
type Resolver = fn(&str, u32) -> Option<DatasetPage>;

static RESOLVER: OnceLock<Resolver> = OnceLock::new();

/// Install the host's dataset resolver. Call once at startup; later calls are
/// ignored.
pub fn set_dataset_resolver(resolver: Resolver) {
    let _ = RESOLVER.set(resolver);
}

/// Resolve up to `limit` rows for `handle`, or `None` if no resolver is
/// installed or the handle is unknown.
pub fn resolve_dataset(handle: &str, limit: u32) -> Option<DatasetPage> {
    RESOLVER.get().and_then(|r| r(handle, limit))
}

// ── Tree access ──────────────────────────────────────────────────────────────
//
// The page API above is enough for a table: a bounded rectangle of strings.
// A *tree* needs something else — the ability to ask what a single node's
// children are without materializing its siblings, so a viewer can draw a
// screenful of a file that does not fit in memory.
//
// These types carry only strings and the SDK's own `TextToken`, so a host can
// back them with Arrow (typed, nested, lazily scanned) without any Arrow
// dependency reaching a plugin.

/// What a tree node is, which is all a viewer needs to decide expandability.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// A keyed container (JSON object / Arrow struct).
    Struct,
    /// An indexed container (JSON array / Arrow list).
    List,
    /// A scalar.
    Leaf,
}

impl NodeKind {
    /// Whether this node can be expanded to reveal children.
    pub fn is_expandable(self) -> bool {
        matches!(self, NodeKind::Struct | NodeKind::List)
    }
}

/// One child node, resolved only when its parent is expanded.
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// Field name, or the element index for a list.
    pub label: String,
    /// Path segment to append to the parent's path (`.name` or `[i]`).
    pub segment: String,
    /// Whether this node is a container, and of which shape.
    pub kind: NodeKind,
    /// Formatted leaf text; empty for containers.
    pub preview: String,
    /// Syntax token for the value half of the row.
    ///
    /// Reached through `tokens`, where it is defined, rather than through
    /// `theme`, which only re-exports it and is gated behind the `egui`
    /// feature. A plugin builds this crate *without* that feature, so the
    /// re-export path does not exist there — and naming it here failed every
    /// plugin's build with an error about `theme` that had nothing to do with
    /// theming.
    pub token: crate::tokens::TextToken,
}

/// Lazy, node-at-a-time access to a dataset's records.
///
/// A record is addressed by index; a node *within* a record by a relative path
/// (`""` is the record, `user.address.city` a nested field, `items[2]` a list
/// element). Every call reads only what it returns.
#[derive(Clone, Copy)]
pub struct DatasetAccess {
    /// Total records available.
    pub total: fn(&str) -> u64,
    /// Whether records have any children at all. Answered from the schema, so
    /// listing a million collapsed records costs nothing.
    pub records_expandable: fn(&str) -> bool,
    /// Children of the node at `rel` within record `root`.
    pub children: fn(&str, u64, &str) -> Vec<TreeNode>,
    /// Display text of the leaf at `rel`.
    pub node_preview: fn(&str, u64, &str) -> String,
    /// The subtree at `rel` as JSON — the edge conversion, for clipboard and
    /// export. Drawing a row must not call this.
    pub node_json: fn(&str, u64, &str) -> Option<serde_json::Value>,
}

static ACCESS: OnceLock<DatasetAccess> = OnceLock::new();

/// Install the host's lazy dataset access. Call once at startup.
pub fn set_dataset_access(access: DatasetAccess) {
    let _ = ACCESS.set(access);
}

/// The installed lazy access, if any.
pub fn dataset_access() -> Option<&'static DatasetAccess> {
    ACCESS.get()
}

/// An installed exporter plugin the [`DataView`](crate::components::DataView)
/// offers in its "Export" dropdown.
#[derive(Clone, Debug)]
pub struct ExporterInfo {
    /// Plugin id (routed back to the host to run the export).
    pub id: String,
    /// Display label, e.g. "CSV Export".
    pub label: String,
    /// Output extension without the dot, e.g. "csv".
    pub extension: String,
}

/// `() -> installed exporters`. Installed by the host.
type ExportersProvider = fn() -> Vec<ExporterInfo>;

static EXPORTERS: OnceLock<ExportersProvider> = OnceLock::new();

/// Install the host's exporter enumerator. Call once at startup.
pub fn set_exporters_provider(provider: ExportersProvider) {
    let _ = EXPORTERS.set(provider);
}

/// The exporter plugins currently installed (empty if none / no provider).
pub fn exporters() -> Vec<ExporterInfo> {
    EXPORTERS.get().map(|p| p()).unwrap_or_default()
}

/// An installed renderer plugin the [`DataView`](crate::components::DataView)
/// offers as an extra view format.
#[derive(Clone, Debug)]
pub struct RendererInfo {
    /// Plugin id (routed back to the host to render).
    pub id: String,
    /// View label shown in the DataView's view dropdown, e.g. "Cards".
    pub label: String,
}

/// `() -> installed renderers`. Installed by the host.
type RenderersProvider = fn() -> Vec<RendererInfo>;

static RENDERERS: OnceLock<RenderersProvider> = OnceLock::new();

/// Install the host's renderer enumerator. Call once at startup.
pub fn set_renderers_provider(provider: RenderersProvider) {
    let _ = RENDERERS.set(provider);
}

/// The renderer plugins currently installed (empty if none / no provider).
pub fn renderers() -> Vec<RendererInfo> {
    RENDERERS.get().map(|p| p()).unwrap_or_default()
}

/// Outcome of rendering a dataset through a renderer plugin.
pub enum PluginRenderResult {
    /// The plugin's `RenderNode` tree, ready to draw.
    Rendered(Box<crate::render_node::RenderNode>),
    /// The user hasn't granted this renderer access yet; a consent prompt was
    /// raised. The view re-renders once approved.
    ConsentPending,
    /// The renderer or dataset is unavailable (uninstalled / dropped / errored).
    Unavailable,
}

/// `(plugin_id, handle) -> render result`. Installed by the host; it reads the
/// dataset, gates consent, runs the plugin, and returns the node tree (cached
/// so it isn't re-run every frame).
type PluginRenderFn = fn(&str, &str) -> PluginRenderResult;

static PLUGIN_RENDERER: OnceLock<PluginRenderFn> = OnceLock::new();

/// Install the host's plugin-render hook. Call once at startup.
pub fn set_plugin_renderer(f: PluginRenderFn) {
    let _ = PLUGIN_RENDERER.set(f);
}

/// Render dataset `handle` through renderer `plugin_id`, or `Unavailable` if no
/// hook is installed.
pub fn render_with_plugin(plugin_id: &str, handle: &str) -> PluginRenderResult {
    match PLUGIN_RENDERER.get() {
        Some(f) => f(plugin_id, handle),
        None => PluginRenderResult::Unavailable,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_page_with_no_mask_reads_every_cell_as_present() {
        // A source that cannot tell a NULL from an empty string says nothing
        // rather than guessing, and the grid draws values, not dashes.
        let page = DatasetPage {
            columns: vec![DatasetColumn {
                name: "a".into(),
                type_hint: "text".into(),
            }],
            rows: vec![vec![String::new()]],
            nulls: Vec::new(),
            total: 1,
        };
        assert!(!page.is_null(0, 0));
        // Out of range is not a null either.
        assert!(!page.is_null(9, 9));
    }

    #[test]
    fn a_short_mask_only_speaks_for_the_cells_it_covers() {
        let page = DatasetPage {
            columns: Vec::new(),
            rows: Vec::new(),
            nulls: vec![vec![true, false]],
            total: 1,
        };
        assert!(page.is_null(0, 0));
        assert!(!page.is_null(0, 1));
        assert!(!page.is_null(0, 2));
        assert!(!page.is_null(1, 0));
    }
}
