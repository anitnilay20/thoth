use eframe::egui::Ui;

use crate::file::loaders::LazyJsonFile;
use crate::helpers::{LruCache, get_context_menu_shortcuts};

use serde_json::Value;

/// Context menu actions that can be performed on selected items
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(clippy::enum_variant_names)]
pub enum ContextMenuAction {
    CopyKey,
    CopyValue,
    CopyObject,
    CopyPath,
}

/// Configuration for which context menu items should be shown
#[derive(Debug, Clone)]
pub struct ContextMenuConfig {
    /// Always show Copy Key
    pub show_copy_key: bool,
    /// Show Copy Value for simple values (not arrays/objects)
    pub show_copy_value: bool,
    /// Show Copy Object for arrays and objects
    pub show_copy_object: bool,
    /// Always show Copy Path
    pub show_copy_path: bool,
}

impl Default for ContextMenuConfig {
    fn default() -> Self {
        Self {
            show_copy_key: true,
            show_copy_value: false,
            show_copy_object: false,
            show_copy_path: true,
        }
    }
}

impl ContextMenuConfig {
    /// Create a config based on the display text of a row
    ///
    /// # Arguments
    /// * `is_key_display` - Whether this row has a key:value format
    /// * `display2` - The value part of the display (after the colon)
    pub fn from_display(is_key_display: bool, display2: &str) -> Self {
        let show_value_menu = is_key_display
            && !display2.starts_with(" [")
            && !display2.starts_with(" {")
            && !display2.starts_with(" (");

        let show_object_menu =
            is_key_display && (display2.starts_with(" [") || display2.starts_with(" {"));

        Self {
            show_copy_key: true,
            show_copy_value: show_value_menu,
            show_copy_object: show_object_menu,
            show_copy_path: true,
        }
    }
}

/// Renders a context menu for file viewer items
///
/// # Arguments
/// * `ui` - The egui UI context
/// * `config` - Configuration for which menu items to show
/// * `on_action` - Callback that receives the selected action
///
/// # Returns
/// `true` if an action was selected, `false` otherwise
pub fn render_context_menu<F>(ui: &mut Ui, config: &ContextMenuConfig, mut on_action: F) -> bool
where
    F: FnMut(ContextMenuAction),
{
    let (copy_key_sc, copy_value_sc, copy_object_sc, copy_path_sc) = get_context_menu_shortcuts();

    let mut action_selected = false;

    // Copy Key
    if config.show_copy_key && ui.button(format!("Copy Key ({})", copy_key_sc)).clicked() {
        on_action(ContextMenuAction::CopyKey);
        ui.close();
        action_selected = true;
    }

    // Copy Value (only show for simple values)
    if config.show_copy_value
        && ui
            .button(format!("Copy Value ({})", copy_value_sc))
            .clicked()
    {
        on_action(ContextMenuAction::CopyValue);
        ui.close();
        action_selected = true;
    }

    // Copy Object (only show for bracket values - objects and arrays)
    if config.show_copy_object
        && ui
            .button(format!("Copy Object ({})", copy_object_sc))
            .clicked()
    {
        on_action(ContextMenuAction::CopyObject);
        ui.close();
        action_selected = true;
    }

    // Copy Path
    if config.show_copy_path && ui.button(format!("Copy Path ({})", copy_path_sc)).clicked() {
        on_action(ContextMenuAction::CopyPath);
        ui.close();
        action_selected = true;
    }

    action_selected
}

/// Helper trait for handling context menu actions
///
/// Implement this trait to provide clipboard operations for your viewer
pub trait ContextMenuHandler {
    /// Copy the key of the selected item
    fn copy_selected_key(&self, selected: &Option<String>) -> Option<String>;

    /// Copy the value of the selected item
    fn copy_selected_value(
        &self,
        selected: &Option<String>,
        cache: &mut LruCache<usize, Value>,
        loader: &mut LazyJsonFile,
    ) -> Option<String>;

    /// Copy the entire object/array of the selected item
    fn copy_selected_object(
        &self,
        selected: &Option<String>,
        cache: &mut LruCache<usize, Value>,
        loader: &mut LazyJsonFile,
    ) -> Option<String>;

    /// Copy the path of the selected item
    fn copy_selected_path(&self, selected: &Option<String>) -> Option<String>;
}

/// Execute a context menu action using a handler
///
/// # Arguments
/// * `action` - The action to execute
/// * `handler` - The handler that implements the clipboard operations
/// * `selected` - The currently selected path
/// * `cache` - The LRU cache for values
/// * `loader` - The lazy file loader
///
/// # Returns
/// The text to copy to clipboard, or None if the action failed
pub fn execute_context_menu_action(
    action: ContextMenuAction,
    handler: &impl ContextMenuHandler,
    selected: &Option<String>,
    cache: &mut LruCache<usize, Value>,
    loader: &mut LazyJsonFile,
) -> Option<String> {
    match action {
        ContextMenuAction::CopyKey => handler.copy_selected_key(selected),
        ContextMenuAction::CopyValue => handler.copy_selected_value(selected, cache, loader),
        ContextMenuAction::CopyObject => handler.copy_selected_object(selected, cache, loader),
        ContextMenuAction::CopyPath => handler.copy_selected_path(selected),
    }
}
