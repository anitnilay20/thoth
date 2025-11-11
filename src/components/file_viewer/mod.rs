pub mod json_tree_viewer;
pub mod types;
pub mod viewer_trait;
pub mod viewer_type;

use eframe::egui::Ui;
use serde_json::Value;
use std::path::{Path, PathBuf};

use self::types::ViewerState;
use self::viewer_type::ViewerType;
use crate::file::lazy_loader::{FileType, LazyJsonFile, load_file_auto};
use crate::helpers::LruCache;

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
        }
    }

    /// Open a file for viewing (compatible with old JsonViewer API)
    pub fn open(&mut self, path: &Path, file_type: &mut FileType) -> anyhow::Result<()> {
        // Load file and detect type
        let (detected_type, loader) = load_file_auto(path)?;
        *file_type = detected_type.into();

        // Store state
        self.loader = Some(loader);
        self.file_path = Some(path.to_path_buf());

        // Clear cache and reset state (recreate cache since LruCache doesn't have clear)
        self.cache = LruCache::new(self.cache_size);
        self.state = ViewerState::default();

        // Create appropriate viewer for file type
        self.viewer = Some(ViewerType::from_file_type(*file_type));

        Ok(())
    }

    /// Set root filter for search results
    pub fn set_root_filter(&mut self, visible_roots: Option<Vec<usize>>) {
        self.state.visible_roots = visible_roots;
    }

    /// Render the file viewer UI
    pub fn ui(&mut self, ui: &mut Ui) {
        if self.loader.is_none() || self.viewer.is_none() {
            ui.centered_and_justified(|ui| {
                ui.label("No file loaded");
            });
            return;
        }

        let loader = self.loader.as_mut().unwrap();
        let total_len = loader.len();
        let viewer = self.viewer.as_mut().unwrap().as_viewer_mut();

        // Rebuild view initially or when visible roots change
        viewer.rebuild_view(
            &self.state.visible_roots,
            &mut self.cache,
            loader,
            total_len,
        );

        // Render the viewer and check if rebuild is needed (due to user interaction)
        let needs_rebuild = viewer.render(ui, &mut self.state.selected, &mut self.cache, loader);

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

    /// Get the current filter length if a filter is active (compatible with old JsonViewer API)
    pub fn current_filter_len(&self) -> Option<usize> {
        self.state.visible_roots.as_ref().map(|v| v.len())
    }
}

impl Default for FileViewer {
    fn default() -> Self {
        Self::new()
    }
}
