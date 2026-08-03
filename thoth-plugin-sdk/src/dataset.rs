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
    /// Total rows available (the page may be capped).
    pub total: u64,
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
