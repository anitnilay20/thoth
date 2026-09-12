use eframe::egui::Ui;
use serde_json::Value;

use crate::file::loaders::{ArrowNode, NodeKind};

/// A loaded file, as the viewers need to see it.
///
/// Reads are Arrow all the way down (see [`crate::file::loaders`]). The tree
/// methods — [`FileViewerLoader::record_expandable`],
/// [`FileViewerLoader::children`], [`FileViewerLoader::node_kind`] — address a
/// node by `(record, path)` and read only the cells they return, so painting a
/// screen never materializes a row.
///
/// [`FileViewerLoader::get_value`] and [`FileViewerLoader::node_json`] are the
/// edge conversions: clipboard, export and plugin rendering. Drawing a row must
/// not call them.
pub trait FileViewerLoader {
    /// Get the number of records
    fn record_count(&self) -> usize;

    /// A whole record as JSON. An *edge* conversion — see the trait docs.
    fn get_value(&mut self, index: usize) -> crate::error::Result<Value>;

    /// Whether record `root` has any children to expand.
    ///
    /// Engine-backed loaders answer this from the schema alone, which is what
    /// lets the tree list a million collapsed records without reading them.
    fn record_expandable(&mut self, root: usize) -> bool {
        matches!(
            self.get_value(root),
            Ok(Value::Object(_)) | Ok(Value::Array(_))
        )
    }

    /// The children of the node at `rel` within record `root` (`""` = the
    /// record itself).
    fn children(&mut self, root: usize, rel: &str) -> Vec<ArrowNode> {
        self.node_json(root, rel)
            .map(|value| value_children(&value))
            .unwrap_or_default()
    }

    /// What kind of node `rel` is, for deciding expandability.
    fn node_kind(&mut self, root: usize, rel: &str) -> NodeKind {
        match self.node_json(root, rel) {
            Some(Value::Object(_)) => NodeKind::Struct,
            Some(Value::Array(_)) => NodeKind::List,
            _ => NodeKind::Leaf,
        }
    }

    /// The display text of the leaf at `rel`.
    fn node_preview(&mut self, root: usize, rel: &str) -> String {
        self.node_json(root, rel)
            .map(|value| crate::helpers::preview_value(&value))
            .unwrap_or_else(|| "null".to_string())
    }

    /// The subtree at `rel` as JSON. An *edge* conversion — see the trait docs.
    fn node_json(&mut self, root: usize, rel: &str) -> Option<Value> {
        let value = self.get_value(root).ok()?;
        if rel.is_empty() {
            Some(value)
        } else {
            crate::helpers::walk_rel(value, rel).ok()
        }
    }

    /// The Arrow query engine behind this loader, when there is one.
    ///
    /// Present for engine-backed files (everything DuckDB opens), absent for
    /// loaders whose records only exist inside a plugin.
    fn query_engine(&self) -> Option<&dyn crate::file::loaders::FileLoader> {
        None
    }

    /// Get the preferred display mode (for plugin viewers)
    fn preferred_display(&mut self) -> crate::plugin::wasm_file_viewer_loader::DisplayMode {
        crate::plugin::wasm_file_viewer_loader::DisplayMode::Table
    }
    
    /// Get column headers (for plugin viewers)
    fn column_headers(&mut self) -> Option<Vec<String>> {
        None
    }
    
    /// Render a record using a plugin viewer
    fn render_record(&mut self, _record_json: &str) -> crate::error::Result<String> {
        Err(crate::error::ThothError::Unknown {
            message: "render_record not supported".to_string(),
        })
    }
}

/// Forwarding impl so a boxed loader can be passed wherever `&mut dyn
/// FileViewerLoader` is expected.
impl FileViewerLoader for Box<dyn FileViewerLoader> {
    fn record_count(&self) -> usize {
        (**self).record_count()
    }

    fn get_value(&mut self, index: usize) -> crate::error::Result<Value> {
        (**self).get_value(index)
    }

    fn record_expandable(&mut self, root: usize) -> bool {
        (**self).record_expandable(root)
    }

    fn children(&mut self, root: usize, rel: &str) -> Vec<ArrowNode> {
        (**self).children(root, rel)
    }

    fn node_kind(&mut self, root: usize, rel: &str) -> NodeKind {
        (**self).node_kind(root, rel)
    }

    fn node_preview(&mut self, root: usize, rel: &str) -> String {
        (**self).node_preview(root, rel)
    }

    fn node_json(&mut self, root: usize, rel: &str) -> Option<Value> {
        (**self).node_json(root, rel)
    }

    fn query_engine(&self) -> Option<&dyn crate::file::loaders::FileLoader> {
        (**self).query_engine()
    }

    fn preferred_display(&mut self) -> crate::plugin::wasm_file_viewer_loader::DisplayMode {
        (**self).preferred_display()
    }

    fn column_headers(&mut self) -> Option<Vec<String>> {
        (**self).column_headers()
    }

    fn render_record(&mut self, record_json: &str) -> crate::error::Result<String> {
        (**self).render_record(record_json)
    }
}


/// Build tree nodes from a JSON value — the fallback for loaders that have no
/// Arrow behind them (plugin-rendered files).
fn value_children(value: &Value) -> Vec<ArrowNode> {
    use thoth_plugin_sdk::tokens::TextToken;

    let node = |label: String, segment: String, child: &Value| {
        let kind = match child {
            Value::Object(_) => NodeKind::Struct,
            Value::Array(_) => NodeKind::List,
            _ => NodeKind::Leaf,
        };
        ArrowNode {
            label,
            segment,
            kind,
            preview: if kind.is_expandable() {
                String::new()
            } else {
                crate::helpers::preview_value(child)
            },
            token: if kind.is_expandable() {
                TextToken::Bracket
            } else {
                TextToken::from(child)
            },
        }
    };

    match value {
        Value::Object(map) => map
            .iter()
            .map(|(key, child)| node(key.clone(), format!(".{key}"), child))
            .collect(),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(i, child)| node(i.to_string(), format!("[{i}]"), child))
            .collect(),
        _ => Vec::new(),
    }
}

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
        loader: &mut dyn FileViewerLoader,
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
    /// * `loader` - File loader for lazy loading
    /// * `should_scroll_to_selection` - Whether to scroll to the selected item (mutable, will be reset after scrolling)
    /// * `is_search_navigation` - Whether this is search navigation (large jump) vs keyboard navigation
    /// * `syntax_highlighting` - Whether to enable syntax highlighting
    #[allow(clippy::too_many_arguments)]
    fn render(
        &mut self,
        ui: &mut Ui,
        selected: &mut Option<String>,
        loader: &mut dyn FileViewerLoader,
        should_scroll_to_selection: &mut bool,
        is_search_navigation: bool,
        syntax_highlighting: bool,
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

    /// Navigate to a specific root record by index
    /// This should select the record and expand it if applicable
    /// Returns true if a rebuild is needed
    fn navigate_to_root(&mut self, root_index: usize) -> bool {
        let _ = root_index;
        false // Default: no-op
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
        loader: &mut dyn FileViewerLoader,
    ) -> Option<String> {
        let _ = (selected, loader);
        None // Default: no-op
    }

    /// Copy the entire object of the currently selected item to clipboard (formatted JSON)
    /// Returns the text to copy, or None if not applicable
    fn copy_selected_object(
        &self,
        selected: &Option<String>,
        loader: &mut dyn FileViewerLoader,
    ) -> Option<String> {
        let _ = (selected, loader);
        None // Default: no-op
    }

    /// Copy the path of the currently selected item to clipboard
    /// Returns the text to copy, or None if not applicable
    fn copy_selected_path(&self, selected: &Option<String>) -> Option<String> {
        selected.clone() // Default: return the path itself
    }
}
