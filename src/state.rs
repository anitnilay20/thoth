use std::path::PathBuf;

use crate::{components, error::ThothError, file, search, update};

// ============================================================================
// Window State - Per-window state (file, search, UI)
// ============================================================================

/// Per-window state - each window has its own file, search, and UI components
/// Note: This is independent of PersistentState which is shared application-wide
pub struct WindowState {
    // File state
    pub file_path: Option<PathBuf>,
    pub file_type: file::lazy_loader::FileType,
    pub error: Option<ThothError>,
    pub total_items: usize,

    // Search state
    pub search_engine_state: SearchEngineState,

    // UI state
    pub sidebar_expanded: bool,
    pub sidebar_selected_section: Option<components::sidebar::SidebarSection>,
    /// Track previous section to determine when to focus search
    pub previous_sidebar_section: Option<components::sidebar::SidebarSection>,
    /// Track previous expanded state to detect sidebar reopening
    pub previous_sidebar_expanded: bool,

    // UI components
    pub sidebar: components::sidebar::Sidebar,
    pub toolbar: components::toolbar::Toolbar,
    pub central_panel: components::central_panel::CentralPanel,
    pub status_bar: components::status_bar::StatusBar,
    pub error_modal: components::error_modal::ErrorModal,
}

impl Default for WindowState {
    fn default() -> Self {
        Self {
            file_path: None,
            file_type: file::lazy_loader::FileType::default(),
            error: None,
            total_items: 0,
            search_engine_state: SearchEngineState::default(),
            sidebar_expanded: true,
            sidebar_selected_section: Some(components::sidebar::SidebarSection::RecentFiles),
            previous_sidebar_section: None,
            previous_sidebar_expanded: false,
            sidebar: components::sidebar::Sidebar::default(),
            toolbar: components::toolbar::Toolbar::default(),
            central_panel: components::central_panel::CentralPanel::default(),
            status_bar: components::status_bar::StatusBar,
            error_modal: components::error_modal::ErrorModal,
        }
    }
}

// ============================================================================
// Helper States - Used by WindowState and application logic
// ============================================================================

#[derive(Default)]
pub struct SearchEngineState {
    pub search: search::Search,
    pub search_rx: Option<std::sync::mpsc::Receiver<search::Search>>,
}

#[derive(Default)]
pub struct ApplicationUpdateState {
    pub update_manager: update::UpdateManager,
    pub update_status: update::UpdateStatus,
    pub pending_download_release: Option<update::ReleaseInfo>,
    pub pending_install_path: Option<PathBuf>,
    pub update_notification_shown: bool,
}
