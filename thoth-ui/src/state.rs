//! egui-only window state. `NavigationHistory` and `ApplicationUpdateState` are
//! display-independent and live in `thoth-core`; re-exported here.

pub use thoth_core::state::*;

use crate::{
    app::tab_manager::TabManager,
    components,
    plugin::{plugin_ui_host::PluginCore, render_node::UiOutput},
};

/// Holds the active plugin pane shown in the main area / a dock tab.
/// Created when the user activates a data-source plugin section in the sidebar,
/// opens a ui-component plugin, or a plugin requests a new tab.
pub struct ActivePluginPane {
    pub plugin_id: String,
    pub display_url: String,
    /// Current interactive UI tree (from `render_ui` / `handle_event`).
    pub ui_output: UiOutput,
    /// The loader is kept here so the central panel can forward UI events.
    /// Boxed so a pane can hold either a data-source or a ui-component loader.
    pub loader: Box<dyn PluginCore>,
    /// Cached tab title/icon from the plugin's `tab-host` export, refreshed after
    /// `render_ui` and each `handle_event`. Read cheaply by the dock tab label
    /// (which is queried every frame, so it must not lock the WASM store).
    pub cached_tab_title: Option<String>,
    pub cached_tab_icon: Option<String>,
}

// ============================================================================
// Window State - Per-window state (UI chrome + tab manager)
// ============================================================================

/// Per-window state. File and navigation state lives inside each tab via TabManager.
pub struct WindowState {
    // Tab manager owns all per-tab state and the dock layout.
    pub tab_manager: TabManager,

    // Sidebar state (global — one sidebar for all tabs)
    pub sidebar_expanded: bool,
    pub sidebar_selected_section: Option<components::sidebar::SidebarSection>,
    /// Track previous expanded state to detect sidebar reopening
    // UI components (global)
    pub sidebar: components::sidebar::Sidebar,
    pub toolbar: components::toolbar::Toolbar,
    pub status_bar: components::status_bar::StatusBar,
    pub error_modal: components::error_modal::ErrorModal,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            tab_manager: TabManager::new(NavigationHistory::DEFAULT_CAPACITY),
            sidebar_expanded: true,
            sidebar_selected_section: Some(components::sidebar::SidebarSection::RecentFiles),
            sidebar: components::sidebar::Sidebar::default(),
            toolbar: components::toolbar::Toolbar::default(),
            status_bar: components::status_bar::StatusBar::default(),
            error_modal: components::error_modal::ErrorModal::default(),
        }
    }
}

impl WindowState {
    /// Convenience accessor — delegates to the tab manager.
    pub fn active_tab_mut(&mut self) -> Option<&mut crate::app::tab_manager::TabState> {
        self.tab_manager.active_tab_mut()
    }
}
