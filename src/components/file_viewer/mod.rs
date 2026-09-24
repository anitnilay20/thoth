pub mod plugin_table_viewer;
pub mod types;
pub mod viewer_trait;

use eframe::egui::{self, Ui};
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use self::types::ViewerState;
use crate::components::file_viewer::plugin_table_viewer::PluginTableViewer;
use crate::components::file_viewer::viewer_trait::{FileFormatViewer, FileViewerLoader};
use crate::file::detect_file_type::{DetectedFileType, sniff_file_type};
use crate::file::loaders::DuckdbConnection;
use crate::file::loaders::duck_db::alias_for_name as alias_of;
use crate::file::{FileKind, FileType};
use crate::plugin::Capability;
use crate::plugin::wasm_file_viewer_loader::WasmFileViewerLoader;
use crate::search::results::{MatchFragment, SearchResults};
use thoth_plugin_sdk::components::{ColumnType, DataView, QueryBuilder, QueryField, TreeAction};

/// Wrapper for WasmFileViewerLoader to implement FileViewerLoader
struct PluginFileViewerLoader {
    inner: WasmFileViewerLoader,
}

impl PluginFileViewerLoader {
    fn new(inner: WasmFileViewerLoader) -> Self {
        Self { inner }
    }
}

impl FileViewerLoader for PluginFileViewerLoader {
    fn record_count(&self) -> usize {
        self.inner.len()
    }

    fn get_value(&mut self, index: usize) -> crate::error::Result<Value> {
        self.inner.get(index)
    }

    fn preferred_display(&mut self) -> crate::plugin::wasm_file_viewer_loader::DisplayMode {
        self.inner.preferred_display()
    }

    fn column_headers(&mut self) -> Option<Vec<String>> {
        self.inner.column_headers()
    }

    fn render_record(&mut self, record_json: &str) -> crate::error::Result<String> {
        self.inner.render_record(record_json)
    }
}

/// Rows crossed into a dataset from a plugin-rendered file (#113). Bounded so
/// a huge file never fully crosses the WASM boundary.
const DATASET_CAP: usize = 5000;

/// A row count with thousands separators, so the picker's column of figures
/// reads at a glance — design `.tablemenu .n` is tabular for the same reason.
fn grouped(count: usize) -> String {
    let digits = count.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.chars().enumerate() {
        if i > 0 && (digits.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(c);
    }
    out
}

/// The handoff's `.tnote`: a view that has to compromise says so once, above
/// the content it is compromising on. `TextView` draws its own; Markdown is
/// rendered by the component itself, so its note is drawn here.
fn document_note(ui: &mut Ui, text: &str) {
    use thoth_plugin_sdk::components::{Typography, TypographyVariant};
    let note = egui::Frame::NONE
        .inner_margin(egui::Margin::symmetric(12, 8))
        .show(ui, |ui| {
            ui.set_width(ui.available_width());
            ui.add(
                Typography::builder()
                    .text(text)
                    .variant(TypographyVariant::Mono)
                    .color("fg_muted")
                    .size(11.0)
                    .build(),
            );
        });
    let colors = thoth_plugin_sdk::theme::ThemeColors::from_ctx(ui.ctx());
    let rect = note.response.rect;
    ui.painter().hline(
        rect.x_range(),
        rect.bottom() - 0.5,
        thoth_plugin_sdk::theme::edge_stroke(&colors),
    );
}

/// A byte count at the coarsest unit that still reads precisely.
fn human_size(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = KB * 1024;
    const GB: u64 = MB * 1024;
    match bytes {
        b if b >= GB => format!("{:.1} GB", b as f64 / GB as f64),
        b if b >= MB => format!("{:.1} MB", b as f64 / MB as f64),
        b if b >= KB => format!("{} KB", b / KB),
        b => format!("{b} B"),
    }
}

/// Bytes indexed up front so a tab has content before its real index exists.
/// Thousands of lines — far more than a screen — for a single read.
const PREVIEW_BYTES: u64 = 1 << 20;

/// Which `DataView` view an engine-backed file opens in.
///
/// Only engine-backed tabs reach `DataView` at all — a document is drawn by
/// `TextView` or `Markdown` and never sees this — so every arm here has rows
/// behind it and must name a view `DataView` actually offers.
fn default_view(path: &Path) -> &'static str {
    match FileType::from_path(path) {
        // Records — the tree is the point.
        FileType::Json => "json",
        // Already rectangular.
        FileType::Csv | FileType::Parquet | FileType::Excel | FileType::Arrow | FileType::DB => {
            "table"
        }
        // The extension said nothing, but reaching here means DuckDB read it
        // anyway — so there is a grid to show.
        FileType::Plugin | FileType::Unknown => "table",
    }
}

/// Prefix of the temporary view a tab's query result is defined as. Per-tab,
/// so two tabs querying the same file never overwrite each other's result.
const QUERY_VIEW_PREFIX: &str = "__thoth_result_";

/// The lightweight tag a tab carries for a file the engine opened.
///
/// [`FileType`] already knows the format; NDJSON vs a JSON array is a
/// distinction only the sniffer makes, and only the status bar cares.
fn detect_kind(path: &Path) -> FileKind {
    match FileType::from_path(path) {
        FileType::Json => match sniff_file_type(path) {
            Ok(DetectedFileType::Ndjson) => FileKind::Ndjson,
            _ => FileKind::Json,
        },
        other => FileKind::from(other),
    }
}

/// A file tab's contents.
///
/// Almost every file is drawn by `DataView` reading from Papyrus, which brings
/// the table / JSON / raw views, keyboard navigation, the context menu, export
/// and Chart Studio with it. The exception is a file claimed by a plugin that
/// supplies its own renderer — the host cannot draw a plugin's custom nodes, so
/// those keep their own loader and viewer.
pub struct FileViewer {
    /// Papyrus handle for an engine-backed file. The `DataView` reads its rows
    /// from the bus, so the viewer itself holds no data.
    handle: Option<String>,

    /// The engine behind that handle, kept so the tab can still publish itself
    /// as a dataset (#113).
    engine: Option<Arc<DuckdbConnection>>,

    /// View the file opens in, by format.
    default_view: &'static str,

    /// A running text-index build, for files with no structured reader. The
    /// tab is usable while it runs; the sheet is published when it finishes.
    index_job: Option<crate::file::indexing::IndexJob>,

    /// Set once the finished index has been announced, so the notification
    /// fires exactly once per file.
    index_announced: bool,

    /// The handle currently points at a prefix of the file, shown while the
    /// real index builds. The status bar says so, because the tab would
    /// otherwise look complete.
    showing_preview: bool,

    /// Everything the document contains — tables, and the objects and scalars
    /// that have no row shape. All of it is listed: a viewer that shows only
    /// the queryable parts misrepresents the file.
    collections: Vec<crate::file::json_envelope::Collection>,

    /// Row counts for collections already ingested, by name. Absent means "not
    /// staged yet", which is the normal state — only the collection being read
    /// is built.
    collection_rows: HashMap<String, usize>,

    /// A collection being ingested after the user chose it.
    staging: Option<crate::file::indexing::StageJob>,

    /// Which collection the tab is showing.
    selected_collection: usize,

    /// Tables inside an attached database, when the file is one.
    ///
    /// The other half of what the picker offers. A database opens on one of
    /// its tables and the rest used to be reachable only by writing SQL —
    /// which is the same misrepresentation the envelope collections fixed, so
    /// they go through the same control.
    db_tables: Vec<String>,

    /// Which of `db_tables` is showing.
    selected_db_table: Option<String>,

    /// The optional DuckDB reader this file needs, when it came back as text
    /// only because that reader is not installed. Drives the offer above the
    /// document.
    needs_extension: Option<crate::file::extensions::Extension>,

    /// A running extension install, started from that offer.
    installing: Option<crate::file::indexing::ExtensionJob>,

    /// The extension revision this tab was indexed against. When it moves, a
    /// reader arrived from somewhere — the marketplace, most likely — and a
    /// tab showing its file as text should look again.
    extensions_seen: u64,

    /// The owning tab, so the sheet keeps a stable producer identity when the
    /// selected collection changes.
    tab_id: usize,

    /// Set only for files a plugin renders itself — those keep their own loader
    /// and viewer, since the host cannot draw a plugin's custom nodes.
    loader: Option<Box<dyn FileViewerLoader>>,

    /// Format-specific viewer (handles different file types)
    viewer: Option<PluginTableViewer>,

    /// Common viewer state
    state: ViewerState,

    /// Current file path (for display and reloading)
    file_path: Option<PathBuf>,

    /// Highlights for records and paths from search results
    highlights: HashMap<usize, Arc<Vec<MatchFragment>>>,

    /// Enable syntax highlighting
    syntax_highlighting: bool,

    /// Events raised by the embedded `DataView`, drained by the app.
    pending_events: Vec<thoth_plugin_sdk::render_node::UiEvent>,

    /// A command from the app's configurable shortcuts, handed to the tree on
    /// the next frame. The binding is the host's; the behaviour is the
    /// component's.
    tree_action: Option<thoth_plugin_sdk::components::TreeAction>,

    /// The query builder above the grid, for engine-backed files. It owns the
    /// query; the tab only runs it.
    query: QueryBuilder,

    /// A query running on a worker. Until it lands the grid keeps showing what
    /// it has, because a query that clears the screen to say "working" is worse
    /// than one that takes a moment.
    query_job: Option<crate::file::indexing::QueryJob>,
}

impl FileViewer {
    /// Create a new FileViewer.
    ///
    /// The viewer holds no records of its own — it renders straight from Arrow,
    /// and any record cache belongs to the viewer that needs one (see
    /// `PluginTableViewer`).
    pub fn new() -> Self {
        Self {
            handle: None,
            engine: None,
            default_view: "table",
            index_job: None,
            index_announced: false,
            showing_preview: false,
            collections: Vec::new(),
            collection_rows: HashMap::new(),
            staging: None,
            selected_collection: 0,
            db_tables: Vec::new(),
            selected_db_table: None,
            needs_extension: None,
            installing: None,
            extensions_seen: 0,
            tab_id: 0,
            loader: None,
            viewer: None,
            state: ViewerState::default(),
            file_path: None,
            highlights: HashMap::new(),
            syntax_highlighting: true, // Default to enabled
            pending_events: Vec::new(),
            tree_action: None,
            query: QueryBuilder::default(),
            query_job: None,
        }
    }

    /// Abandon any running index build — the tab no longer wants it.
    pub fn cancel_indexing(&mut self) {
        if let Some(job) = self.index_job.take() {
            job.cancel();
        }
    }

    /// Set syntax highlighting enabled/disabled
    pub fn set_syntax_highlighting(&mut self, enabled: bool) {
        self.syntax_highlighting = enabled;
    }

    /// Open a file for viewing.
    ///
    /// Everything goes through the DuckDB engine except files claimed by a
    /// plugin that supplies its own renderer — those keep rendering through
    /// the plugin, since the host has no way to draw their custom nodes.
    pub fn open(
        &mut self,
        path: &Path,
        tab_id: usize,
        file_type: &mut FileKind,
    ) -> crate::error::Result<()> {
        let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase());
        let ext_str = ext.as_deref().unwrap_or("");

        // A plugin that declares FileViewer owns rendering as well as loading,
        // and its result is then used as-is — but only for a format the engine
        // does not read itself. The engine brings lazy scanning, real types,
        // SQL and everything built on it, so handing one of its formats to a
        // plugin is a downgrade the user never asked for.
        let plugin_manager = crate::plugin::runtime::active_manager();
        let plugin_rendered = plugin_manager.as_deref().and_then(|pm| {
            if !FileType::from_path(path).is_native()
                && pm.plugin_has_capability(ext_str, &Capability::FileViewer)
            {
                Some(pm.open_file_with_viewer(ext_str, path))
            } else {
                None
            }
        });

        self.handle = None;
        self.engine = None;
        self.loader = None;
        self.collections.clear();
        self.collection_rows.clear();
        self.staging = None;
        self.selected_collection = 0;
        self.db_tables.clear();
        self.selected_db_table = None;
        self.needs_extension = None;
        self.installing = None;
        self.extensions_seen = crate::file::extensions::revision();
        self.tab_id = tab_id;

        let kind = match plugin_rendered {
            Some(Ok(wfl)) => {
                self.loader = Some(Box::new(PluginFileViewerLoader::new(wfl)));
                FileKind::PluginTable
            }
            Some(Err(e)) => return Err(e),
            // Everything else goes through the engine and onto the bus. The
            // engine picks the right DuckDB reader, staging through a
            // file-loader plugin when there is no native one.
            // Opening is deferred to a worker. Deciding how to read a file
            // means asking DuckDB, and for a large document that question is
            // answered by parsing it — which would freeze the UI for as long as
            // that takes. The tab appears now and upgrades in place.
            None => {
                // Show the file straight away. A prefix index costs one read
                // and gives the tab real content — the alternative is staring
                // at a spinner for as long as the scan takes, which on a large
                // document is most of a minute.
                if let Ok(preview) = crate::file::loaders::TextIndex::preview(path, PREVIEW_BYTES) {
                    self.handle = crate::papyrus::publish_text(
                        "core",
                        &format!("core#{tab_id}"),
                        path.file_name()
                            .map(|n| n.to_string_lossy().to_string())
                            .unwrap_or_else(|| "file".to_string()),
                        preview,
                    );
                    self.showing_preview = true;
                }
                self.index_job = Some(crate::file::indexing::IndexJob::spawn(path));
                self.default_view = default_view(path);
                detect_kind(path)
            }
        };

        *file_type = kind;
        self.file_path = Some(path.to_path_buf());

        self.state = ViewerState::default();
        self.highlights.clear();

        // Create appropriate viewer for file type
        self.viewer = (kind == FileKind::PluginTable).then(PluginTableViewer::new);

        Ok(())
    }

    /// Set root filter for search results
    pub fn set_root_filter(&mut self, visible_roots: Option<Vec<usize>>) {
        self.state.visible_roots = visible_roots;
    }

    /// Navigate to and expand a specific root record by index
    /// This selects the record, expands it, and scrolls to it
    pub fn navigate_to_root(&mut self, root_index: usize) -> bool {
        // Set selection to the root record path (e.g., "0", "1", "2")
        let path = root_index.to_string();
        self.state.selected = Some(path);

        // Trigger scroll to selection on next render
        self.state.should_scroll_to_selection = true;
        // Mark this as search navigation (large jump) not keyboard navigation
        self.state.is_search_navigation = true;

        // Delegate to the viewer's navigate_to_root implementation and rebuild if needed
        if let Some(viewer) = self.viewer.as_mut() {
            let needs_rebuild = viewer.navigate_to_root(root_index);
            if needs_rebuild && let Some(loader) = self.loader.as_mut() {
                // Rebuild view immediately so rows are ready for scrolling
                let total_len = loader.record_count();
                viewer.rebuild_view(&self.state.visible_roots, loader, total_len);
            }
            return needs_rebuild;
        }

        false
    }

    /// Navigate to a specific JSON path
    /// This selects the path and scrolls to it
    /// Automatically expands parent nodes to make the path visible
    pub fn navigate_to_path(&mut self, path: String) {
        // Auto-expand parent nodes to make the path visible
        if let Some(viewer) = self.viewer.as_mut() {
            // Expand each parent node in the path hierarchy
            let path_parts: Vec<&str> = path.split('.').collect();
            let mut current_path = String::new();

            for (i, part) in path_parts.iter().enumerate() {
                if i > 0 {
                    current_path.push('.');
                }
                current_path.push_str(part);

                // Expand this node (except the leaf)
                if i < path_parts.len() - 1 {
                    viewer.expand_selected(&Some(current_path.clone()));
                }
            }
        }

        // Set selection to the path
        self.state.selected = Some(path);

        // Trigger scroll to selection on next render
        self.state.should_scroll_to_selection = true;
        // Mark this as search navigation (large jump) not keyboard navigation
        self.state.is_search_navigation = true;
    }

    /// Get the currently selected path
    pub fn get_selected_path(&self) -> Option<&String> {
        self.state.selected.as_ref()
    }

    /// Render the file viewer UI
    pub fn ui(&mut self, ui: &mut Ui) {
        // Engine-backed files are drawn by `DataView`, reading their rows from
        // Papyrus. That is what gives the file tab table / JSON / raw views,
        // export and Chart Studio for free.
        // A staging job or a query may have landed since the last frame.
        self.poll_staging();
        self.poll_query();
        self.poll_extension_install();
        self.poll_extension_arrivals();

        if let Some(handle) = self.handle.clone() {
            // A document is not a dataset. `DataView` leads with a table
            // picker and a format switcher and carries Copy and Export,
            // because rows can be drawn several ways and taken elsewhere — a
            // log has one sensible rendering and no columns to export, so all
            // of that is chrome answering questions nobody asked.
            if self.engine.is_none() {
                self.draw_document(ui, &handle);
                return;
            }
            // What a document holds is the first thing the pane has to answer,
            // so it is asked at the head of the view rather than in a list
            // beside it: the grid gets the full width, and the collections are
            // named where the question comes up.
            self.draw_data_view(ui, &handle);
            return;
        }

        let (Some(loader), Some(viewer_box)) = (self.loader.as_mut(), self.viewer.as_mut()) else {
            ui.centered_and_justified(|ui| {
                ui.add(
                    thoth_plugin_sdk::components::Typography::builder()
                        .text("No file loaded")
                        .variant(thoth_plugin_sdk::components::TypographyVariant::BodyMuted)
                        .build(),
                );
            });
            return;
        };

        let total_len = loader.record_count();
        let viewer = viewer_box;

        viewer.rebuild_view(&self.state.visible_roots, loader, total_len);

        let needs_rebuild = viewer.render(
            ui,
            &mut self.state.selected,
            loader,
            &mut self.state.should_scroll_to_selection,
            self.state.is_search_navigation,
            self.syntax_highlighting,
        );

        if self.state.is_search_navigation {
            self.state.is_search_navigation = false;
        }

        if needs_rebuild {
            viewer.rebuild_view(&self.state.visible_roots, loader, total_len);
        }
    }

    /// Whether the tab is showing a prefix of the file rather than all of it.
    pub fn showing_preview(&self) -> bool {
        self.showing_preview
    }

    /// Progress of a background index build, if one is running for this tab.
    pub fn index_progress(&self) -> Option<crate::file::indexing::Progress> {
        self.index_job.as_ref().map(|job| job.progress())
    }

    /// Adopt a finished index, publishing it to the bus.
    ///
    /// Returns the file's name and its true row count once, the frame the index
    /// becomes available. The count matters: until then the tab is showing a
    /// prefix, and reporting the preview's size as the file's would be wrong.
    pub fn poll_index(&mut self, tab_id: usize) -> Option<(String, usize)> {
        let job = self.index_job.as_ref()?;
        if !job.progress().is_finished() || self.index_announced {
            return None;
        }
        self.index_announced = true;
        self.showing_preview = false;

        let indexed = job.take()?;
        let name = job
            .path()
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "file".to_string());
        let instance = format!("core#{tab_id}");

        self.handle = match indexed {
            // Read natively, or an envelope whose collections are now tables —
            // either way the tab is queryable.
            crate::file::indexing::Indexed::Engine {
                engine,
                total,
                collections,
            } => {
                let engine = Arc::new(engine);
                self.collections = collections;
                self.collection_rows.clear();
                // A database's tables come from the engine rather than from a
                // document scan; everything downstream treats them the same.
                self.db_tables = engine.database_tables();
                self.selected_db_table = self.db_tables.first().cloned();
                // Only what is actually staged has a count; the rest report
                // their size until they are.
                if let Some(primary) = engine.primary_alias() {
                    self.collection_rows.insert(primary.clone(), total);
                    self.selected_collection = self
                        .collections
                        .iter()
                        .position(|c| alias_of(&c.name) == primary)
                        .unwrap_or(0);
                }
                let handle = crate::papyrus::publish_arrow_with_total(
                    "core",
                    &instance,
                    name.clone(),
                    engine.clone(),
                    total as u64,
                );
                self.engine = Some(engine);
                if let Some(alias) = self
                    .engine
                    .as_ref()
                    .and_then(|engine| engine.primary_alias())
                {
                    self.aim_query_at(&alias);
                }
                handle
            }
            crate::file::indexing::Indexed::Text(index) => {
                self.needs_extension = None;
                crate::papyrus::publish_text("core", &instance, name.clone(), *index)
            }
            // Readable, but only with a reader the user has not fetched. The
            // text is real and browsable; the banner says what would open it
            // properly, so the file does not just look corrupt.
            crate::file::indexing::Indexed::NeedsExtension { index, extension } => {
                self.needs_extension = Some(extension);
                self.extensions_seen = crate::file::extensions::revision();
                crate::papyrus::publish_text("core", &instance, name.clone(), *index)
            }
        };
        let total = self
            .handle
            .as_deref()
            .map(|handle| crate::papyrus::total(handle) as usize)
            .unwrap_or(0);
        Some((name, total))
    }

    /// The collections this document holds, as the `DataView`'s table picker
    /// offers them.
    ///
    /// Every collection is listed, including the objects and scalars that have
    /// no row shape: a picker that shows only the queryable parts of a file
    /// misrepresents the file. What each entry reports depends on what is
    /// actually known — a staged table its rows, an unstaged one its size —
    /// because claiming a row count would mean reading the collection, and
    /// reading it is the thing the user has not asked for yet.
    fn table_options(&self) -> Vec<thoth_plugin_sdk::components::DataTable> {
        use crate::file::json_envelope::ValueKind;
        use thoth_plugin_sdk::components::DataTable;

        // A database's tables, when the file is one. Only the table on screen
        // reports a count: `count(*)` is a scan per table, and this is read
        // while drawing.
        if !self.db_tables.is_empty() {
            return self
                .db_tables
                .iter()
                .map(|table| {
                    let showing = self.selected_db_table.as_deref() == Some(table.as_str());
                    let detail = showing
                        .then(|| self.collection_rows.get(table).map(|rows| grouped(*rows)))
                        .flatten();
                    DataTable::builder()
                        .value(table.clone())
                        .label(table.clone())
                        .maybe_detail(detail)
                        .build()
                })
                .collect();
        }

        let staging = self.staging.as_ref().map(|job| job.name());
        self.collections
            .iter()
            .map(|c| {
                let alias = alias_of(&c.name);
                let detail = if staging == Some(alias.as_str()) {
                    "staging…".to_string()
                } else if let Some(rows) = self.collection_rows.get(&alias) {
                    grouped(*rows)
                } else if c.kind == ValueKind::Array {
                    human_size(c.len())
                } else {
                    format!("{} · {}", human_size(c.len()), c.kind.as_str())
                };

                DataTable::builder()
                    .value(c.name.clone())
                    .label(c.name.clone())
                    .detail(detail)
                    .build()
            })
            .collect()
    }

    /// Point the tab at another table of the attached database.
    ///
    /// Nothing to stage: the table is already there, and switching is a view
    /// definition. The sheet is republished rather than mutated because its
    /// row count and window both describe the relation it points at.
    fn select_database_table(&mut self, table: &str) {
        let Some(engine) = self.engine.clone() else {
            return;
        };
        let Ok(rows) = engine.show_database_table(table) else {
            return;
        };
        self.collection_rows.insert(table.to_string(), rows);
        self.selected_db_table = Some(table.to_string());
        self.handle = crate::papyrus::publish_arrow_with_total(
            "core",
            &format!("core#{}", self.tab_id),
            table.to_string(),
            engine,
            rows as u64,
        );
        if let Some(alias) = self
            .engine
            .as_ref()
            .and_then(|engine| engine.primary_alias())
        {
            self.aim_query_at(&alias);
        }
    }

    /// Point the tab at another collection, staging it first if it has never
    /// been read.
    fn select_collection(&mut self, index: usize) {
        use crate::file::json_envelope::ValueKind;

        let Some(collection) = self.collections.get(index).cloned() else {
            return;
        };
        let Some(engine) = self.engine.clone() else {
            return;
        };
        // Objects and scalars are listed so the file is honestly represented,
        // but there is no table to point at.
        if collection.kind != ValueKind::Array {
            return;
        }
        let alias = alias_of(&collection.name);

        if !self.collection_rows.contains_key(&alias) {
            // Never staged. Ingesting is measured in seconds, so it runs on a
            // worker and the tab keeps showing what it has.
            if self.staging.is_none()
                && let Some(path) = self.file_path.clone()
            {
                self.staging = Some(crate::file::indexing::StageJob::spawn(
                    engine,
                    &path,
                    &collection,
                ));
                self.selected_collection = index;
            }
            return;
        }

        self.show_collection(index);
    }

    /// Re-point the view at a collection already present as a table.
    fn show_collection(&mut self, index: usize) {
        let Some(collection) = self.collections.get(index).cloned() else {
            return;
        };
        let Some(engine) = self.engine.clone() else {
            return;
        };
        let alias = alias_of(&collection.name);
        if engine.set_primary(&alias).is_err() {
            return;
        }
        let rows = engine.row_count_of(&alias).unwrap_or(0);
        self.collection_rows.insert(alias.clone(), rows);
        self.selected_collection = index;
        // Republished rather than mutated: the sheet's row count and window
        // both describe the relation it points at.
        self.handle = crate::papyrus::publish_arrow_with_total(
            "core",
            &format!("core#{}", self.tab_id),
            collection.name,
            engine,
            rows as u64,
        );
        self.aim_query_at(&alias);
    }

    /// Point the builder at `alias`, reading its columns from the engine.
    ///
    /// The lanes name the columns of the relation they were built against, so a
    /// change of relation clears them rather than carrying a filter over to a
    /// table that has no such column.
    fn aim_query_at(&mut self, alias: &str) {
        let Some(engine) = self.engine.as_ref() else {
            return;
        };
        self.query = QueryBuilder::builder()
            .id(format!("file_query_{}", self.tab_id))
            .relation(alias.to_string())
            .fields(
                engine
                    .column_types(alias)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|(name, sql_type)| {
                        QueryField::builder()
                            .name(name)
                            .column_type(ColumnType::from_sql(&sql_type))
                            .build()
                    })
                    .collect::<Vec<_>>(),
            )
            .build();
    }

    /// Run the query the lanes describe, on a worker thread.
    fn run_query(&mut self) {
        let Some(engine) = self.engine.clone() else {
            return;
        };
        // One query at a time per tab: the second would race the first for the
        // same view name, and the grid can only show one result anyway.
        if self.query_job.is_some() {
            return;
        }
        let sql = match self.query.spec.compile(&self.query.relation) {
            Ok(sql) => sql,
            // The foot already names an incomplete lane; Run is disabled while
            // it does, so reaching here means the shortcut fired instead.
            Err(error) => {
                self.query.status = Some(error.message);
                return;
            }
        };
        self.query.status = Some("running…".to_string());
        self.query_job = Some(crate::file::indexing::QueryJob::spawn(
            engine,
            format!("{QUERY_VIEW_PREFIX}{}", self.tab_id),
            sql,
        ));
    }

    /// Adopt a finished query, if one just landed.
    fn poll_query(&mut self) {
        let Some(job) = self.query_job.as_ref() else {
            return;
        };
        if !job.is_finished() {
            return;
        }
        let outcome = job.take();
        self.query_job = None;

        let Some(outcome) = outcome else {
            return;
        };
        let Some(engine) = self.engine.clone() else {
            return;
        };
        match outcome {
            Ok(result) => {
                if engine.set_primary(&result.view).is_err() {
                    self.query.status = Some("the result could not be read".to_string());
                    return;
                }
                self.query.status = Some(format!(
                    "{} {} · {} ms",
                    result.rows,
                    if result.rows == 1 { "row" } else { "rows" },
                    result.elapsed.as_millis()
                ));
                self.handle = crate::papyrus::publish_arrow_with_total(
                    "core",
                    &format!("core#{}", self.tab_id),
                    self.query.relation.clone(),
                    engine,
                    result.rows as u64,
                );
            }
            Err(message) => self.query.status = Some(message),
        }
    }

    /// Adopt a finished collection staging, if one just landed.
    fn poll_staging(&mut self) {
        let Some(job) = self.staging.as_ref() else {
            return;
        };
        if !job.is_finished() {
            return;
        }
        let failed = job.failed();
        let name = job.name().to_string();
        self.staging = None;
        if failed {
            return;
        }
        if let Some(index) = self
            .collections
            .iter()
            .position(|c| alias_of(&c.name) == name)
        {
            self.show_collection(index);
        }
    }

    /// Lines the document views read before they say they have stopped.
    ///
    /// Both materialize — the editor lays out every line it is handed and
    /// Markdown is a whole-document format — so a large file is a window onto
    /// itself, and the caption says which window.
    const DOCUMENT_LINES: u32 = 5_000;

    /// The offer above a document the engine could read with one more reader.
    ///
    /// Drawn where the compromise is, like every other note in the viewer, and
    /// it states the cost before asking: a one-time download of a stated size
    /// is a different proposition from an unexplained wait.
    fn draw_extension_offer(&mut self, ui: &mut Ui) {
        use thoth_plugin_sdk::components::{Button, ButtonColor, Typography, TypographyVariant};

        let Some(extension) = self.needs_extension.clone() else {
            return;
        };
        let busy = self
            .installing
            .as_ref()
            .is_some_and(|job| job.extension().name == extension.name && !job.is_finished());

        let colors = thoth_plugin_sdk::theme::ThemeColors::from_ctx(ui.ctx());
        let mut start = false;
        let note = egui::Frame::NONE
            .fill(colors.surface)
            .inner_margin(egui::Margin::symmetric(12, 8))
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.add(
                        Typography::builder()
                            .text(format!(
                                "{} need the DuckDB {} reader · {} one-time download",
                                extension.unlocks, extension.name, extension.size
                            ))
                            .variant(TypographyVariant::BodyMuted)
                            .build(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if busy {
                            ui.add(
                                Typography::builder()
                                    .text("Downloading…")
                                    .variant(TypographyVariant::BodyMuted)
                                    .build(),
                            );
                        } else {
                            start = ui
                                .add(
                                    Button::builder()
                                        .label("Install")
                                        .icon(egui_phosphor::regular::DOWNLOAD_SIMPLE)
                                        .color(ButtonColor::Primary)
                                        .hover_text(format!(
                                            "Fetch the {} reader and reopen this file",
                                            extension.name
                                        ))
                                        .build(),
                                )
                                .clicked();
                        }
                    });
                });
            });
        ui.painter().hline(
            note.response.rect.x_range(),
            note.response.rect.bottom() - 0.5,
            thoth_plugin_sdk::theme::edge_stroke(&colors),
        );

        if start {
            self.installing = Some(crate::file::indexing::ExtensionJob::spawn(extension));
        }
    }

    /// Reopen when a reader this file was waiting for turns up.
    ///
    /// The install can come from anywhere — the offer above the document, or
    /// the marketplace, which has never heard of this tab. Both bump the same
    /// counter, so both are noticed here.
    fn poll_extension_arrivals(&mut self) {
        let revision = crate::file::extensions::revision();
        if self.needs_extension.is_none() || revision == self.extensions_seen {
            return;
        }
        self.extensions_seen = revision;
        if let Some(path) = self.file_path.clone() {
            let mut kind = FileKind::default();
            let tab_id = self.tab_id;
            let _ = self.open(&path, tab_id, &mut kind);
        }
    }

    /// Adopt a finished extension install: on success the file is reopened,
    /// because the engine can read it now and text was only ever the fallback.
    fn poll_extension_install(&mut self) {
        let Some(job) = self.installing.as_ref() else {
            return;
        };
        if !job.is_finished() {
            return;
        }
        let outcome = job.take();
        self.installing = None;

        match outcome {
            Some(Ok(())) => {
                self.needs_extension = None;
                if let Some(path) = self.file_path.clone() {
                    let mut kind = FileKind::default();
                    let tab_id = self.tab_id;
                    let _ = self.open(&path, tab_id, &mut kind);
                }
            }
            Some(Err(message)) => {
                crate::notification::NotificationManager::notify_error(
                    crate::notification::Notification::new(
                        "Could not install the reader",
                        &message,
                    ),
                );
            }
            None => {}
        }
    }

    /// Draw a text-backed tab as the document it is.
    fn draw_document(&mut self, ui: &mut Ui, handle: &str) {
        use thoth_plugin_sdk::components::{Markdown, TextView};

        let total = crate::papyrus::total(handle);
        let page = crate::papyrus::read(handle, 0, Self::DOCUMENT_LINES);
        let text = page
            .map(|page| {
                // A text index publishes one row per line, numbered. The line
                // number is chrome the file never had.
                let col = page.columns.len().saturating_sub(1);
                page.rows
                    .iter()
                    .map(|row| row.get(col).map(String::as_str).unwrap_or(""))
                    .collect::<Vec<_>>()
                    .join("\n")
            })
            .unwrap_or_default();

        self.draw_extension_offer(ui);

        let markdown = self
            .file_path
            .as_deref()
            .is_some_and(FileType::is_prose_document);

        if markdown {
            // Markdown has no honest partial rendering — a heading means
            // nothing without the section under it — so a truncated document
            // says so before it is read, not after.
            if total > u64::from(Self::DOCUMENT_LINES) {
                document_note(
                    ui,
                    &format!(
                        "first {} of {} lines",
                        grouped(Self::DOCUMENT_LINES as usize),
                        grouped(total as usize)
                    ),
                );
            }
            egui::ScrollArea::vertical()
                .id_salt(("file_markdown", self.tab_id))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    Markdown::builder()
                        .id(format!("file_markdown_{}", self.tab_id))
                        .value(text)
                        .build()
                        .show(ui);
                });
            return;
        }

        TextView::builder()
            .id(format!("file_text_{}", self.tab_id))
            .value(text)
            .maybe_caption((total > u64::from(Self::DOCUMENT_LINES)).then(|| {
                format!(
                    "first {} of {} lines",
                    grouped(Self::DOCUMENT_LINES as usize),
                    grouped(total as usize)
                )
            }))
            .build()
            .show(ui);
    }

    fn draw_data_view(&mut self, ui: &mut Ui, handle: &str) {
        // Only an engine-backed file has a relation to query; a text index has
        // lines, and offering SQL over them would be a promise nothing keeps.
        if self.engine.is_some() {
            let out = self.query.show(ui);
            if out.run {
                self.run_query();
            }
        }

        let mut events = Vec::new();
        let mut view = DataView::builder()
            // Keyed on the tab, not the handle, so switching collections keeps
            // the chosen view instead of resetting it.
            .id(format!("file_view_{}", self.tab_id))
            .handle(handle.to_string())
            .default_view(self.default_view)
            .tables(self.table_options())
            .maybe_selected_table(if self.db_tables.is_empty() {
                self.collections
                    .get(self.selected_collection)
                    .map(|c| c.name.clone())
            } else {
                self.selected_db_table.clone()
            })
            .build();
        // Consumed, so a shortcut fires once rather than every frame until the
        // next one replaces it.
        view.tree_action = self.tree_action.take();
        view.show(ui, &mut events);

        // Switching table is this tab's work, not the app's: it may have to
        // stage the collection first, and only the viewer knows that. Kept out
        // of `pending_events` so the app never sees an action it has no handler
        // for.
        events.retain(|event| {
            if event.id != thoth_plugin_sdk::actions::SELECT_TABLE {
                return true;
            }
            if self.db_tables.contains(&event.value) {
                self.select_database_table(&event.value);
            } else if let Some(index) = self.collections.iter().position(|c| c.name == event.value)
            {
                self.select_collection(index);
            }
            false
        });
        self.pending_events.extend(events);
    }

    /// Drain UI events raised by the embedded `DataView` (export picks, the
    /// Charts shortcut) for the app to act on.
    pub fn take_events(&mut self) -> Vec<thoth_plugin_sdk::render_node::UiEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// Update highlight metadata from search results
    pub fn set_highlights(&mut self, results: Option<&SearchResults>) {
        self.highlights.clear();
        if let Some(res) = results {
            for hit in res.hits() {
                if !hit.fragments.is_empty() {
                    self.highlights
                        .insert(hit.record_index, Arc::new(hit.fragments.clone()));
                }
            }
        }
    }

    /// Get the total number of root items in the loaded file
    pub fn total_item_count(&self) -> usize {
        if let Some(handle) = self.handle.as_deref() {
            return crate::papyrus::total(handle) as usize;
        }
        self.loader.as_ref().map(|l| l.record_count()).unwrap_or(0)
    }

    /// Read this tab's live loader into a tabular dataset for the data bus
    /// (#113). Works for any backing loader — JSON, NDJSON, CSV, Parquet, a
    /// database, or a file-loader plugin — so every file tab is a producer by
    /// default.
    pub fn to_dataset(&mut self) -> Option<crate::file::to_dataset::DatasetTable> {
        // Engine-backed files convert straight from Arrow.
        if let Some(engine) = self.engine.as_ref() {
            return crate::file::to_dataset::loader_to_dataset(engine.as_ref());
        }

        // Plugin-rendered files have no Arrow side; read their records instead.
        let loader = self.loader.as_mut()?;
        let count = loader.record_count().min(DATASET_CAP);
        let records: Vec<Value> = (0..count)
            .filter_map(|i| loader.get_value(i).ok())
            .collect();
        crate::file::to_dataset::records_to_dataset(&records)
    }

    // ========================================================================
    // Keyboard Shortcut Support
    // ========================================================================
    //
    // The app owns these bindings because they are user-configurable; the tree
    // owns what they do. Each just queues a command for the next frame.

    pub fn expand_selected_node(&mut self) {
        self.queue(TreeAction::ExpandNode);
    }

    pub fn collapse_selected_node(&mut self) {
        self.queue(TreeAction::CollapseNode);
    }

    pub fn expand_all_nodes(&mut self) {
        self.queue(TreeAction::ExpandAll);
    }

    pub fn collapse_all_nodes(&mut self) {
        self.queue(TreeAction::CollapseAll);
    }

    pub fn move_selection_up(&mut self) {
        self.queue(TreeAction::MoveUp);
    }

    pub fn move_selection_down(&mut self) {
        self.queue(TreeAction::MoveDown);
    }

    pub fn copy_selected_key(&mut self) {
        self.queue(TreeAction::CopyKey);
    }

    pub fn copy_selected_value(&mut self) {
        self.queue(TreeAction::CopyValue);
    }

    pub fn copy_selected_object(&mut self) {
        self.queue(TreeAction::CopyObject);
    }

    pub fn copy_selected_path(&mut self) {
        self.queue(TreeAction::CopyPath);
    }

    fn queue(&mut self, action: TreeAction) {
        self.tree_action = Some(action);
    }
}

impl Default for FileViewer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn files_open_in_the_view_that_suits_their_shape() {
        // Records read as a tree...
        assert_eq!(default_view(Path::new("/tmp/a.json")), "json");
        assert_eq!(default_view(Path::new("/tmp/a.ndjson")), "json");
        assert_eq!(default_view(Path::new("/tmp/a.jsonl")), "json");
        // ...rectangles as a grid...
        assert_eq!(default_view(Path::new("/tmp/a.csv")), "table");
        assert_eq!(default_view(Path::new("/tmp/a.tsv")), "table");
        assert_eq!(default_view(Path::new("/tmp/a.parquet")), "table");
        assert_eq!(default_view(Path::new("/tmp/a.duckdb")), "table");
        assert_eq!(default_view(Path::new("/tmp/a.sqlite")), "table");
        // ...and an unnamed format that the engine read anyway as a grid.
        // (A document never gets here: it has no engine, so it is drawn by
        // `TextView` and `default_view` is not consulted.)
        assert_eq!(default_view(Path::new("/tmp/a.weird")), "table");
        assert_eq!(default_view(Path::new("/tmp/a.log")), "table");
        assert_eq!(default_view(Path::new("/tmp/noext")), "table");
    }

    #[test]
    fn a_markdown_file_is_never_handed_to_the_engine() {
        // DuckDB's CSV sniffer reads almost any line-oriented text as a
        // one-column table, so without this a README opens as a grid of its
        // own lines.
        assert!(FileType::is_prose_document("/tmp/README.md"));
        assert!(FileType::is_prose_document("/tmp/notes.MARKDOWN"));
        assert!(!FileType::is_prose_document("/tmp/data.csv"));
        assert!(!FileType::is_prose_document("/tmp/app.log"));
        assert!(!FileType::is_prose_document("/tmp/noext"));
    }

    #[test]
    fn a_row_count_reads_in_groups_of_three() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(7), "7");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1_000), "1,000");
        assert_eq!(grouped(4_812), "4,812");
        assert_eq!(grouped(1_234_567), "1,234,567");
    }

    #[test]
    fn the_picker_reports_what_is_known_and_no_more() {
        use crate::file::json_envelope::{Collection, ValueKind};

        let mut viewer = FileViewer::new();
        viewer.collections = vec![
            Collection {
                name: "users".to_string(),
                kind: ValueKind::Array,
                start: 0,
                end: 4096,
            },
            Collection {
                name: "logs".to_string(),
                kind: ValueKind::Array,
                start: 4096,
                end: 8192,
            },
            Collection {
                name: "meta".to_string(),
                kind: ValueKind::Object,
                start: 8192,
                end: 8292,
            },
        ];
        // Only `users` has been read, so only `users` can report rows.
        viewer.collection_rows.insert(alias_of("users"), 4812);

        let tables = viewer.table_options();
        assert_eq!(
            tables.iter().map(|t| t.value.as_str()).collect::<Vec<_>>(),
            ["users", "logs", "meta"],
            "every collection is offered, not just the queryable ones"
        );
        assert_eq!(tables[0].detail.as_deref(), Some("4,812"));
        // Unread: its size, never a row count it would have to scan for.
        assert_eq!(tables[1].detail.as_deref(), Some("4 KB"));
        // No row shape at all, and it says so instead of posing as a table.
        assert_eq!(tables[2].detail.as_deref(), Some("100 B · object"));
    }

    #[test]
    fn an_envelope_reaches_the_picker_with_every_collection_it_holds() {
        use std::io::Write;

        crate::file::index_cache::tests::isolate();

        let mut tmp = tempfile::Builder::new().suffix(".json").tempfile().unwrap();
        tmp.write_all(
            concat!(
                r#"{"users":[{"id":1,"name":"ada"},{"id":2,"name":"linus"}],"#,
                r#""logs":[{"level":"INFO"}],"meta":{"v":3}}"#
            )
            .as_bytes(),
        )
        .unwrap();
        tmp.flush().unwrap();

        let mut viewer = FileViewer::new();
        let mut kind = FileKind::Json;
        viewer.open(tmp.path(), 4242, &mut kind).expect("opened");

        // The index runs on a worker; the tab shows a preview until it lands.
        let mut landed = false;
        for _ in 0..2000 {
            if viewer.poll_index(4242).is_some() {
                landed = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        assert!(landed, "the index never landed");

        let tables = viewer.table_options();
        assert_eq!(
            tables.iter().map(|t| t.value.as_str()).collect::<Vec<_>>(),
            ["users", "logs", "meta"],
            "the picker offers what the document holds, in document order"
        );
        // Whichever collection the tab opened on is the one with a count; the
        // rest report their size, because nothing has scanned them.
        let opened = viewer.selected_collection;
        let staged = alias_of(&viewer.collections[opened].name);
        assert_eq!(viewer.collection_rows.len(), 1, "only one was staged");
        assert_eq!(
            tables[opened].detail.as_deref(),
            Some(grouped(viewer.collection_rows[&staged])).as_deref()
        );
        for (i, t) in tables.iter().enumerate() {
            if i != opened {
                let detail = t.detail.as_deref().unwrap();
                assert!(
                    detail.contains('B') || detail.contains("KB"),
                    "unstaged {} reported {detail}, not a size",
                    t.value
                );
            }
        }
    }

    #[test]
    fn a_document_is_drawn_as_a_document_not_as_a_dataset() {
        use std::io::Write;

        crate::file::index_cache::tests::isolate();
        // A file DuckDB declines is text, and a text tab has no engine — which
        // is what routes it to `TextView` instead of `DataView` and its table
        // picker, format switcher, Copy and Export.
        let mut tmp = tempfile::Builder::new().suffix(".log").tempfile().unwrap();
        tmp.write_all(b"starting worker\nshard-02 ready\nall done\n")
            .unwrap();
        tmp.flush().unwrap();

        let mut viewer = FileViewer::new();
        let mut kind = FileKind::Json;
        viewer.open(tmp.path(), 7, &mut kind).expect("opened");
        for _ in 0..2000 {
            if viewer.poll_index(7).is_some() {
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(5));
        }

        assert!(
            viewer.engine.is_none(),
            "prose reached the engine; it would be drawn as a dataset"
        );
        assert!(viewer.handle.is_some(), "the tab still has content to draw");
        assert!(
            viewer.table_options().is_empty(),
            "a document has no collections to pick between"
        );
    }

    #[test]
    fn a_file_that_is_one_table_offers_no_tables() {
        // Natively-read files (CSV, NDJSON, Parquet) yield no collections, and
        // the picker is hidden rather than showing the file back to itself.
        assert!(FileViewer::new().table_options().is_empty());
    }

    #[test]
    fn the_default_view_is_one_dataview_offers() {
        // A value DataView doesn't know falls back to Table, silently — so the
        // mapping must only ever produce views that exist.
        for path in [
            "/tmp/a.json",
            "/tmp/a.csv",
            "/tmp/a.parquet",
            "/tmp/a.weird",
            "/tmp/README.md",
        ] {
            let view = default_view(Path::new(path));
            assert!(
                matches!(view, "table" | "json" | "raw" | "chart"),
                "{path} → {view} is not a view DataView offers"
            );
        }
    }
}
