use eframe::egui::Ui;
use serde_json::Value;

use crate::file::lazy_loader::LazyJsonFile;
use crate::helpers::LruCache;

/// Trait that all file format viewers must implement
///
/// This is a specialized stateful component for rendering file content.
/// New file format viewers should implement this trait to integrate with FileViewer.
///
/// # Example
/// ```ignore
/// impl FileFormatViewer for JsonTreeViewer {
///     fn reset(&mut self) { ... }
///     fn rebuild_view(&mut self, ...) { ... }
///     fn render(&mut self, ...) -> bool { ... }
/// }
/// ```
pub trait FileFormatViewer {
    /// Reset the viewer state (called when opening a new file)
    #[allow(dead_code)]
    fn reset(&mut self);

    /// Rebuild the view based on visible items and cache
    ///
    /// Called when:
    /// - File is first opened
    /// - Search filter changes (visible_roots changes)
    /// - Data needs to be refreshed
    fn rebuild_view(
        &mut self,
        visible_roots: &Option<Vec<usize>>,
        cache: &mut LruCache<usize, Value>,
        loader: &mut LazyJsonFile,
        total_len: usize,
    );

    /// Render the viewer UI and return whether a rebuild is needed
    ///
    /// Returns `true` if the view needs to be rebuilt (e.g., user toggled expansion)
    /// Returns `false` if no rebuild is needed
    ///
    /// # Arguments
    /// * `ui` - egui UI context
    /// * `selected` - Currently selected item path (mutable)
    /// * `cache` - LRU cache for parsed values
    /// * `loader` - File loader for lazy loading
    /// * `should_scroll_to_selection` - Whether to scroll to the selected item (mutable, will be reset after scrolling)
    fn render(
        &mut self,
        ui: &mut Ui,
        selected: &mut Option<String>,
        cache: &mut LruCache<usize, Value>,
        loader: &mut LazyJsonFile,
        should_scroll_to_selection: &mut bool,
    ) -> bool;

    // ========================================================================
    // Navigation & Tree Operations (for keyboard shortcuts)
    // ========================================================================

    /// Expand the currently selected node
    /// Returns true if a rebuild is needed
    fn expand_selected(&mut self, selected: &Option<String>) -> bool {
        let _ = selected;
        false // Default: no-op
    }

    /// Collapse the currently selected node
    /// Returns true if a rebuild is needed
    fn collapse_selected(&mut self, selected: &Option<String>) -> bool {
        let _ = selected;
        false // Default: no-op
    }

    /// Expand all nodes in the tree
    /// Returns true if a rebuild is needed
    fn expand_all(&mut self) -> bool {
        false // Default: no-op
    }

    /// Collapse all nodes in the tree
    /// Returns true if a rebuild is needed
    fn collapse_all(&mut self) -> bool {
        false // Default: no-op
    }

    /// Move selection up to previous visible item
    /// Returns the new selection path, or None if can't move up
    fn move_selection_up(&self, current: &Option<String>) -> Option<String> {
        let _ = current;
        None // Default: no-op
    }

    /// Move selection down to next visible item
    /// Returns the new selection path, or None if can't move down
    fn move_selection_down(&self, current: &Option<String>) -> Option<String> {
        let _ = current;
        None // Default: no-op
    }

    // ========================================================================
    // Clipboard Operations (for keyboard shortcuts)
    // ========================================================================

    /// Copy the key of the currently selected item to clipboard
    /// Returns the text to copy, or None if not applicable
    fn copy_selected_key(&self, selected: &Option<String>) -> Option<String> {
        let _ = selected;
        None // Default: no-op
    }

    /// Copy the value of the currently selected item to clipboard
    /// Returns the text to copy, or None if not applicable
    fn copy_selected_value(
        &self,
        selected: &Option<String>,
        cache: &mut LruCache<usize, Value>,
        loader: &mut LazyJsonFile,
    ) -> Option<String> {
        let _ = (selected, cache, loader);
        None // Default: no-op
    }

    /// Copy the entire object of the currently selected item to clipboard (formatted JSON)
    /// Returns the text to copy, or None if not applicable
    fn copy_selected_object(
        &self,
        selected: &Option<String>,
        cache: &mut LruCache<usize, Value>,
        loader: &mut LazyJsonFile,
    ) -> Option<String> {
        let _ = (selected, cache, loader);
        None // Default: no-op
    }

    /// Copy the path of the currently selected item to clipboard
    /// Returns the text to copy, or None if not applicable
    fn copy_selected_path(&self, selected: &Option<String>) -> Option<String> {
        selected.clone() // Default: return the path itself
    }
}
