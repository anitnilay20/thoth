use eframe::egui::Ui;
use serde_json::Value;

use crate::file::lazy_loader::LazyJsonFile;
use crate::helpers::LruCache;

/// Trait that all file format viewers must implement
///
/// This trait defines the common interface for rendering different file formats.
/// New file format viewers should implement this trait to integrate with FileViewer.
///
/// # Example
/// ```
/// impl FileFormatViewer for JsonTreeViewer {
///     fn reset(&mut self) { ... }
///     fn rebuild_view(&mut self, ...) { ... }
///     fn render(&mut self, ...) -> bool { ... }
/// }
/// ```
#[allow(dead_code)]
pub trait FileFormatViewer {
    /// Reset the viewer state (called when opening a new file)
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
    fn render(
        &mut self,
        ui: &mut Ui,
        selected: &mut Option<String>,
        cache: &mut LruCache<usize, Value>,
        loader: &mut LazyJsonFile,
    ) -> bool;
}
