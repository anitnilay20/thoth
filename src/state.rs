use std::path::PathBuf;

use crate::{components, error::ThothError, file, search, update};

#[cfg(test)]
mod tests;

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

    // Navigation state
    pub navigation_history: NavigationHistory,

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
            navigation_history: NavigationHistory::default(),
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

/// Navigation history for back/forward navigation through viewed JSON paths
#[derive(Debug, Clone)]
pub struct NavigationHistory {
    /// Stack of visited paths (e.g., "0.user.name", "1.items[2]")
    history: Vec<String>,
    /// Current position in history (index into history vec)
    current_index: Option<usize>,
    /// Maximum history size to prevent unbounded growth
    max_history: usize,
}

impl NavigationHistory {
    /// Create a new navigation history with default max size
    pub fn new() -> Self {
        Self::with_capacity(100)
    }

    /// Create a new navigation history with specified max size
    pub fn with_capacity(max_history: usize) -> Self {
        Self {
            history: Vec::new(),
            current_index: None,
            max_history,
        }
    }

    /// Add a new path to history
    /// If we're not at the end of history, this truncates forward history
    pub fn push(&mut self, path: String) {
        // Don't add if it's the same as the current path
        if let Some(idx) = self.current_index {
            if idx < self.history.len() && self.history[idx] == path {
                return;
            }
        }

        // If we're in the middle of history, truncate everything after current
        if let Some(idx) = self.current_index {
            self.history.truncate(idx + 1);
        }

        // Add the new path
        self.history.push(path);

        // Maintain max size (remove oldest entries)
        if self.history.len() > self.max_history {
            self.history.remove(0);
            // Adjust current_index since we removed from front
            if let Some(idx) = self.current_index {
                self.current_index = Some(idx.saturating_sub(1));
            }
        }

        // Update current index to point to the new entry
        self.current_index = Some(self.history.len() - 1);
    }

    /// Navigate back in history, returns the previous path if available
    pub fn back(&mut self) -> Option<String> {
        let idx = self.current_index?;

        if idx > 0 {
            self.current_index = Some(idx - 1);
            Some(self.history[idx - 1].clone())
        } else {
            None
        }
    }

    /// Navigate forward in history, returns the next path if available
    pub fn forward(&mut self) -> Option<String> {
        let idx = self.current_index?;

        if idx + 1 < self.history.len() {
            self.current_index = Some(idx + 1);
            Some(self.history[idx + 1].clone())
        } else {
            None
        }
    }

    /// Check if we can navigate back
    pub fn can_go_back(&self) -> bool {
        self.current_index.map_or(false, |idx| idx > 0)
    }

    /// Check if we can navigate forward
    pub fn can_go_forward(&self) -> bool {
        self.current_index
            .map_or(false, |idx| idx + 1 < self.history.len())
    }

    /// Get current path
    pub fn current(&self) -> Option<&String> {
        self.current_index.and_then(|idx| self.history.get(idx))
    }

    /// Clear all history
    pub fn clear(&mut self) {
        self.history.clear();
        self.current_index = None;
    }

    /// Get the size of the history
    pub fn len(&self) -> usize {
        self.history.len()
    }

    /// Check if history is empty
    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }
}

impl Default for NavigationHistory {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default)]
pub struct ApplicationUpdateState {
    pub update_manager: update::UpdateManager,
    pub update_status: update::UpdateStatus,
    pub pending_download_release: Option<update::ReleaseInfo>,
    pub pending_install_path: Option<PathBuf>,
    pub update_notification_shown: bool,
}
