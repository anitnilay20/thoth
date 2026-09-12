pub mod context_menu;
pub mod json_tree_viewer;
pub mod plugin_table_viewer;
pub mod types;
pub mod viewer_trait;
pub mod viewer_type;

use eframe::egui::Ui;
use serde_json::Value;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use self::types::ViewerState;
use self::viewer_type::ViewerType;
use crate::components::file_viewer::viewer_trait::FileViewerLoader;
use crate::file::detect_file_type::{DetectedFileType, sniff_file_type};
use crate::file::loaders::DuckdbConnection;
use thoth_plugin_sdk::components::DataView;
use crate::file::{FileKind, FileType};
use crate::plugin::Capability;
use crate::plugin::wasm_file_viewer_loader::WasmFileViewerLoader;
use crate::search::results::{MatchFragment, SearchResults};

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

/// The view a file opens in, before the user picks one.
///
/// Chosen by format rather than fixed, because the right first look differs:
/// records read best as a tree, tabular formats as a grid, and anything we have
/// no structure for is most honestly shown as text.
fn default_view(path: &Path) -> &'static str {
    match FileType::from_path(path) {
        // Records — the tree is the point.
        FileType::Json => "json",
        // Already rectangular.
        FileType::Csv | FileType::Parquet | FileType::DB => "table",
        // No schema we can trust; show it as it is.
        FileType::Plugin | FileType::Unknown => "raw",
    }
}

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

/// Generic file viewer that manages common viewing concerns (loading, caching, selection)
/// and delegates format-specific rendering to specialized viewers via the ViewerType enum.
///
/// This architecture makes it easy to add new file format viewers:
/// 1. Create a new viewer struct (e.g., `CsvTableViewer`)
/// 2. Implement `FileFormatViewer` trait for it
/// 3. Add the viewer to `ViewerType` enum
/// 4. That's it! FileViewer will automatically work with the new viewer
pub struct FileViewer {
    /// Papyrus handle for an engine-backed file. The `DataView` reads its rows
    /// from the bus, so the viewer itself holds no data.
    handle: Option<String>,

    /// The engine behind that handle, kept so the tab can still publish itself
    /// as a dataset (#113).
    engine: Option<Arc<DuckdbConnection>>,

    /// View the file opens in, by format.
    default_view: &'static str,

    /// Set only for files a plugin renders itself — those keep their own loader
    /// and viewer, since the host cannot draw a plugin's custom nodes.
    loader: Option<Box<dyn FileViewerLoader>>,

    /// Format-specific viewer (handles different file types)
    viewer: Option<ViewerType>,

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
}

impl FileViewer {
    /// Create a new FileViewer
    pub fn new() -> Self {
        Self::with_cache_size(0)
    }

    /// Retained for call-site compatibility — the viewer no longer caches
    /// records, because it renders straight from Arrow. Any record cache now
    /// belongs to the viewer that needs one (see `PluginTableViewer`).
    pub fn with_cache_size(_cache_size: usize) -> Self {
        Self {
            handle: None,
            engine: None,
            default_view: "table",
            loader: None,
            viewer: None,
            state: ViewerState::default(),
            file_path: None,
            highlights: HashMap::new(),
            syntax_highlighting: true, // Default to enabled
            pending_events: Vec::new(),
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

        // A plugin that declares FileViewer owns rendering as well as loading.
        // If one claims this extension its result is used as-is — we do NOT
        // silently fall through to the engine when a plugin claims the format.
        let plugin_manager = crate::plugin::runtime::active_manager();
        let plugin_rendered = plugin_manager.as_deref().and_then(|pm| {
            if pm.plugin_has_capability(ext_str, &Capability::FileViewer) {
                Some(pm.open_file_with_viewer(ext_str, path))
            } else {
                None
            }
        });

        self.handle = None;
        self.engine = None;
        self.loader = None;

        let kind = match plugin_rendered {
            Some(Ok(wfl)) => {
                self.loader = Some(Box::new(PluginFileViewerLoader::new(wfl)));
                FileKind::PluginTable
            }
            Some(Err(e)) => return Err(e),
            // Everything else goes through the engine and onto the bus. The
            // engine picks the right DuckDB reader, staging through a
            // file-loader plugin when there is no native one.
            None => {
                let engine = Arc::new(DuckdbConnection::open_path(path)?);
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| "file".to_string());
                // The producer marker the app reserves for core file tabs. It
                // must match, or the frame reaper drops the sheet as soon as it
                // is published — and it keys on the *tab*, so re-opening a file
                // in place replaces its sheet rather than stacking a new one.
                let instance = format!("core#{tab_id}");
                self.handle =
                    crate::papyrus::publish_arrow("core", &instance, name, engine.clone());
                self.engine = Some(engine);
                self.default_view = default_view(path);
                detect_kind(path)
            }
        };

        *file_type = kind;
        self.file_path = Some(path.to_path_buf());

        self.state = ViewerState::default();
        self.highlights.clear();

        // Create appropriate viewer for file type
        self.viewer = Some(ViewerType::from_file_type(*file_type));
        self.apply_highlights_to_viewer();

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
            let needs_rebuild = viewer.as_viewer_mut().navigate_to_root(root_index);
            if needs_rebuild && let Some(loader) = self.loader.as_mut() {
                // Rebuild view immediately so rows are ready for scrolling
                let total_len = loader.record_count();
                viewer.as_viewer_mut().rebuild_view(
                    &self.state.visible_roots,
                    loader,
                    total_len,
                );
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
                    viewer
                        .as_viewer_mut()
                        .expand_selected(&Some(current_path.clone()));
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
        if let Some(handle) = self.handle.clone() {
            let mut events = Vec::new();
            DataView::builder()
                .id(format!("file_view_{handle}"))
                .handle(handle)
                .default_view(self.default_view)
                .build()
                .show(ui, &mut events);
            self.pending_events.extend(events);
            return;
        }

        let (Some(loader), Some(viewer_box)) = (self.loader.as_mut(), self.viewer.as_mut()) else {
            ui.centered_and_justified(|ui| {
                ui.label("No file loaded");
            });
            return;
        };

        let total_len = loader.record_count();
        let viewer = viewer_box.as_viewer_mut();

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
        self.apply_highlights_to_viewer();
    }

    fn apply_highlights_to_viewer(&mut self) {
        if let Some(ViewerType::Json(json)) = self.viewer.as_mut() {
            json.set_highlights(&self.highlights);
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
        let records: Vec<Value> = (0..count).filter_map(|i| loader.get_value(i).ok()).collect();
        crate::file::to_dataset::records_to_dataset(&records)
    }

    // ========================================================================
    // Keyboard Shortcut Support - Navigation & Tree Operations
    // ========================================================================

    /// Expand the currently selected node (for keyboard shortcuts)
    /// Returns true if view needs to be rebuilt
    pub fn expand_selected_node(&mut self) -> bool {
        if let Some(viewer) = self.viewer.as_mut() {
            let result = viewer.as_viewer_mut().expand_selected(&self.state.selected);
            if result && let Some(loader) = self.loader.as_mut() {
                // Rebuild if needed
                let total_len = loader.record_count();
                viewer.as_viewer_mut().rebuild_view(
                    &self.state.visible_roots,
                    loader,
                    total_len,
                );
            }
            return result;
        }
        false
    }

    /// Collapse the currently selected node (for keyboard shortcuts)
    /// Returns true if view needs to be rebuilt
    pub fn collapse_selected_node(&mut self) -> bool {
        if let Some(viewer) = self.viewer.as_mut() {
            let result = viewer
                .as_viewer_mut()
                .collapse_selected(&self.state.selected);
            if result && let Some(loader) = self.loader.as_mut() {
                // Rebuild if needed
                let total_len = loader.record_count();
                viewer.as_viewer_mut().rebuild_view(
                    &self.state.visible_roots,
                    loader,
                    total_len,
                );
            }
            return result;
        }
        false
    }

    /// Expand all nodes in the tree (for keyboard shortcuts)
    pub fn expand_all_nodes(&mut self) -> bool {
        if let Some(viewer) = self.viewer.as_mut() {
            let result = viewer.as_viewer_mut().expand_all();
            if result && let Some(loader) = self.loader.as_mut() {
                // Rebuild if needed
                let total_len = loader.record_count();
                viewer.as_viewer_mut().rebuild_view(
                    &self.state.visible_roots,
                    loader,
                    total_len,
                );
            }
            return result;
        }
        false
    }

    /// Collapse all nodes in the tree (for keyboard shortcuts)
    pub fn collapse_all_nodes(&mut self) -> bool {
        if let Some(viewer) = self.viewer.as_mut() {
            let result = viewer.as_viewer_mut().collapse_all();
            if result && let Some(loader) = self.loader.as_mut() {
                // Rebuild if needed
                let total_len = loader.record_count();
                viewer.as_viewer_mut().rebuild_view(
                    &self.state.visible_roots,
                    loader,
                    total_len,
                );
            }
            return result;
        }
        false
    }

    /// Move selection up to previous item (for keyboard shortcuts)
    pub fn move_selection_up(&mut self) {
        if let Some(viewer) = self.viewer.as_mut()
            && let Some(new_selection) = viewer
                .as_viewer_mut()
                .move_selection_up(&self.state.selected)
        {
            self.state.selected = Some(new_selection);
            self.state.should_scroll_to_selection = true;
        }
    }

    /// Move selection down to next item (for keyboard shortcuts)
    pub fn move_selection_down(&mut self) {
        if let Some(viewer) = self.viewer.as_mut()
            && let Some(new_selection) = viewer
                .as_viewer_mut()
                .move_selection_down(&self.state.selected)
        {
            self.state.selected = Some(new_selection);
            self.state.should_scroll_to_selection = true;
        }
    }

    // ========================================================================
    // Keyboard Shortcut Support - Clipboard Operations
    // ========================================================================

    /// Copy the key of the currently selected item (for keyboard shortcuts)
    /// Returns the text to copy, or None
    pub fn copy_selected_key(&mut self) -> Option<String> {
        self.viewer
            .as_mut()?
            .as_viewer_mut()
            .copy_selected_key(&self.state.selected)
    }

    /// Copy the value of the currently selected item (for keyboard shortcuts)
    /// Returns the text to copy, or None
    pub fn copy_selected_value(&mut self) -> Option<String> {
        if let (Some(viewer), Some(loader)) = (self.viewer.as_mut(), self.loader.as_mut()) {
            return viewer.as_viewer_mut().copy_selected_value(
                &self.state.selected,
                loader,
            );
        }
        None
    }

    /// Copy the entire object of the currently selected item (for keyboard shortcuts)
    /// Returns the text to copy (formatted JSON), or None
    pub fn copy_selected_object(&mut self) -> Option<String> {
        if let (Some(viewer), Some(loader)) = (self.viewer.as_mut(), self.loader.as_mut()) {
            return viewer.as_viewer_mut().copy_selected_object(
                &self.state.selected,
                loader,
            );
        }
        None
    }

    /// Copy the path of the currently selected item (for keyboard shortcuts)
    /// Returns the text to copy, or None
    pub fn copy_selected_path(&mut self) -> Option<String> {
        self.viewer
            .as_mut()?
            .as_viewer_mut()
            .copy_selected_path(&self.state.selected)
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
        // ...and anything we can't infer a shape for, as text.
        assert_eq!(default_view(Path::new("/tmp/a.weird")), "raw");
        assert_eq!(default_view(Path::new("/tmp/noext")), "raw");
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
        ] {
            let view = default_view(Path::new(path));
            assert!(
                matches!(view, "table" | "json" | "raw"),
                "{path} → {view} is not a built-in view"
            );
        }
    }
}
