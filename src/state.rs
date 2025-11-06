use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::{components, file, search, settings, update};

// ============================================================================
// Shared State - Shared across all windows
// ============================================================================

/// Shared state across all windows (settings, theme, etc.)
#[derive(Clone)]
pub struct SharedState {
    pub settings: Arc<Mutex<settings::Settings>>,
}

impl SharedState {
    pub fn new(settings: settings::Settings) -> Self {
        Self {
            settings: Arc::new(Mutex::new(settings)),
        }
    }
}

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

    // Search state
    pub search_engine_state: SearchEngineState,

    // UI components
    pub toolbar: components::toolbar::Toolbar,
    pub central_panel: components::central_panel::CentralPanel,
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
