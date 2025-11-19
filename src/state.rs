use std::path::PathBuf;

use crate::{components, file, recent_files, search, update};

// ============================================================================
// Window State - Per-window state (file, search, UI)
// ============================================================================

/// Per-window state - each window has its own file, search, and UI components
#[derive(Default)]
pub struct WindowState {
    // File state
    pub file_path: Option<PathBuf>,
    pub file_type: file::lazy_loader::FileType,
    pub error: Option<String>,
    pub total_items: usize,

    // Search state
    pub search_engine_state: SearchEngineState,

    // Recent files (loaded from disk on first access via Default)
    pub recent_files: recent_files::RecentFiles,

    // UI components
    pub sidebar: components::sidebar::Sidebar,
    pub toolbar: components::toolbar::Toolbar,
    pub central_panel: components::central_panel::CentralPanel,
    pub status_bar: components::status_bar::StatusBar,
    pub search_dropdown: components::search_dropdown::SearchDropdown,
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
