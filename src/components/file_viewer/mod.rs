pub mod context_menu;
pub mod json_tree_viewer;
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
use crate::file::loaders::{FileType, LazyJsonFile, load_file_auto};
use crate::helpers::LruCache;
use crate::search::results::{MatchFragment, SearchResults};

/// Generic file viewer that manages common viewing concerns (loading, caching, selection)
/// and delegates format-specific rendering to specialized viewers via the ViewerType enum.
///
/// This architecture makes it easy to add new file format viewers:
/// 1. Create a new viewer struct (e.g., `CsvTableViewer`)
/// 2. Implement `FileFormatViewer` trait for it
/// 3. Add the viewer to `ViewerType` enum
/// 4. That's it! FileViewer will automatically work with the new viewer
pub struct FileViewer {
    /// File loader for lazy parsing
    loader: Option<LazyJsonFile>,

    /// LRU cache for parsed values
    cache: LruCache<usize, Value>,

    /// Cache capacity
    cache_size: usize,

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
}

impl FileViewer {
    /// Create a new FileViewer with default cache size
    pub fn new() -> Self {
        Self::with_cache_size(100)
    }

    /// Create a new FileViewer with custom cache size
    pub fn with_cache_size(cache_size: usize) -> Self {
        Self {
            loader: None,
            cache: LruCache::new(cache_size),
            cache_size,
            viewer: None,
            state: ViewerState::default(),
            file_path: None,
            highlights: HashMap::new(),
            syntax_highlighting: true, // Default to enabled
        }
    }

    /// Set syntax highlighting enabled/disabled
    pub fn set_syntax_highlighting(&mut self, enabled: bool) {
        self.syntax_highlighting = enabled;
    }

    /// Open a file for viewing (compatible with old JsonViewer API)
    pub fn open(&mut self, path: &Path, file_type: &mut FileType) -> crate::error::Result<()> {
        // Load file and detect type
        let (detected_type, loader) = load_file_auto(path)?;
        *file_type = detected_type.into();

        // Store state
        self.loader = Some(loader);
        self.file_path = Some(path.to_path_buf());

        // Clear cache and reset state (recreate cache since LruCache doesn't have clear)
        self.cache = LruCache::new(self.cache_size);
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
            if needs_rebuild {
                if let Some(loader) = self.loader.as_mut() {
                    // Rebuild view immediately so rows are ready for scrolling
                    let total_len = loader.len();
                    viewer.as_viewer_mut().rebuild_view(
                        &self.state.visible_roots,
                        &mut self.cache,
                        loader,
                        total_len,
                    );
                }
            }
            return needs_rebuild;
        }

        false
    }

    /// Render the file viewer UI
    pub fn ui(&mut self, ui: &mut Ui) {
        let (Some(loader), Some(viewer_box)) = (self.loader.as_mut(), self.viewer.as_mut()) else {
            ui.centered_and_justified(|ui| {
                ui.label("No file loaded");
            });
            return;
        };

        let total_len = loader.len();
        let viewer = viewer_box.as_viewer_mut();

        // Rebuild view initially or when visible roots change
        viewer.rebuild_view(
            &self.state.visible_roots,
            &mut self.cache,
            loader,
            total_len,
        );

        // Render the viewer and check if rebuild is needed (due to user interaction)
        let needs_rebuild = viewer.render(
            ui,
            &mut self.state.selected,
            &mut self.cache,
            loader,
            &mut self.state.should_scroll_to_selection,
            self.state.is_search_navigation,
            self.syntax_highlighting,
        );

        // Reset the search navigation flag after rendering
        if self.state.is_search_navigation {
            self.state.is_search_navigation = false;
        }

        // Rebuild if needed (e.g., user toggled expansion)
        if needs_rebuild {
            viewer.rebuild_view(
                &self.state.visible_roots,
                &mut self.cache,
                loader,
                total_len,
            );
        }
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
        self.loader.as_ref().map(|l| l.len()).unwrap_or(0)
    }

    // ========================================================================
    // Keyboard Shortcut Support - Navigation & Tree Operations
    // ========================================================================

    /// Expand the currently selected node (for keyboard shortcuts)
    /// Returns true if view needs to be rebuilt
    pub fn expand_selected_node(&mut self) -> bool {
        if let Some(viewer) = self.viewer.as_mut() {
            let result = viewer.as_viewer_mut().expand_selected(&self.state.selected);
            if result {
                if let Some(loader) = self.loader.as_mut() {
                    // Rebuild if needed
                    let total_len = loader.len();
                    viewer.as_viewer_mut().rebuild_view(
                        &self.state.visible_roots,
                        &mut self.cache,
                        loader,
                        total_len,
                    );
                }
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
            if result {
                if let Some(loader) = self.loader.as_mut() {
                    // Rebuild if needed
                    let total_len = loader.len();
                    viewer.as_viewer_mut().rebuild_view(
                        &self.state.visible_roots,
                        &mut self.cache,
                        loader,
                        total_len,
                    );
                }
            }
            return result;
        }
        false
    }

    /// Expand all nodes in the tree (for keyboard shortcuts)
    pub fn expand_all_nodes(&mut self) -> bool {
        if let Some(viewer) = self.viewer.as_mut() {
            let result = viewer.as_viewer_mut().expand_all();
            if result {
                if let Some(loader) = self.loader.as_mut() {
                    // Rebuild if needed
                    let total_len = loader.len();
                    viewer.as_viewer_mut().rebuild_view(
                        &self.state.visible_roots,
                        &mut self.cache,
                        loader,
                        total_len,
                    );
                }
            }
            return result;
        }
        false
    }

    /// Collapse all nodes in the tree (for keyboard shortcuts)
    pub fn collapse_all_nodes(&mut self) -> bool {
        if let Some(viewer) = self.viewer.as_mut() {
            let result = viewer.as_viewer_mut().collapse_all();
            if result {
                if let Some(loader) = self.loader.as_mut() {
                    // Rebuild if needed
                    let total_len = loader.len();
                    viewer.as_viewer_mut().rebuild_view(
                        &self.state.visible_roots,
                        &mut self.cache,
                        loader,
                        total_len,
                    );
                }
            }
            return result;
        }
        false
    }

    /// Move selection up to previous item (for keyboard shortcuts)
    pub fn move_selection_up(&mut self) {
        if let Some(viewer) = self.viewer.as_mut() {
            if let Some(new_selection) = viewer
                .as_viewer_mut()
                .move_selection_up(&self.state.selected)
            {
                self.state.selected = Some(new_selection);
                self.state.should_scroll_to_selection = true;
            }
        }
    }

    /// Move selection down to next item (for keyboard shortcuts)
    pub fn move_selection_down(&mut self) {
        if let Some(viewer) = self.viewer.as_mut() {
            if let Some(new_selection) = viewer
                .as_viewer_mut()
                .move_selection_down(&self.state.selected)
            {
                self.state.selected = Some(new_selection);
                self.state.should_scroll_to_selection = true;
            }
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
                &mut self.cache,
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
                &mut self.cache,
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
