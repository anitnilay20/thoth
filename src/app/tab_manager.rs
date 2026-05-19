use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui;
use egui_dock::{DockState, tab_viewer::OnCloseResponse};

use crate::{
    app::persistent_state::PersistentState,
    components::central_panel::{CentralPanel, CentralPanelProps},
    components::traits::ContextComponent,
    error::ThothError,
    file::lazy_loader::FileKind,
    plugin::render_node::UiOutput,
    settings::Settings,
    state::{ActivePluginPane, NavigationHistory, SearchEngineState},
};

pub type TabId = usize;

/// Per-tab state — everything that belongs to one open file/plugin pane.
pub struct TabState {
    pub id: TabId,
    pub file_path: Option<PathBuf>,
    pub file_type: FileKind,
    pub error: Option<ThothError>,
    pub total_items: usize,
    pub search_engine_state: SearchEngineState,
    pub navigation_history: NavigationHistory,
    pub pending_navigation: Option<String>,
    pub active_plugin_pane: Option<ActivePluginPane>,
    pub plugin_sidebar_output: Option<UiOutput>,
    pub central_panel: CentralPanel,
}

impl TabState {
    pub fn new(id: TabId, file_path: Option<PathBuf>, nav_capacity: usize) -> Self {
        Self {
            id,
            file_path,
            file_type: FileKind::default(),
            error: None,
            total_items: 0,
            search_engine_state: SearchEngineState::default(),
            navigation_history: NavigationHistory::with_capacity(nav_capacity),
            pending_navigation: None,
            active_plugin_pane: None,
            plugin_sidebar_output: None,
            central_panel: CentralPanel::default(),
        }
    }

    pub fn title(&self) -> String {
        self.file_path
            .as_ref()
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|s| s.to_string())
            .unwrap_or_else(|| "New Tab".to_string())
    }

    pub fn is_empty(&self) -> bool {
        self.file_path.is_none() && self.active_plugin_pane.is_none()
    }
}

/// Events emitted from ThothTabViewer to ThothApp, drained after DockArea::show_inside.
pub enum TabEvent {
    FileOpened {
        tab_id: TabId,
        path: PathBuf,
        file_type: FileKind,
        total_items: usize,
    },
    FileOpenError {
        tab_id: TabId,
        error: ThothError,
    },
    FileClosed {
        tab_id: TabId,
    },
    FileTypeChanged {
        tab_id: TabId,
        file_type: FileKind,
    },
    ErrorCleared {
        tab_id: TabId,
    },
    PluginUiEvent {
        tab_id: TabId,
        event: crate::plugin::render_node::UiEvent,
    },
    NavigationPush {
        tab_id: TabId,
        path: String,
    },
    TabClosed(TabId),
}

/// Implements egui_dock::TabViewer. Holds mutable refs to tabs and settings so each
/// tab's CentralPanel can be rendered without the app needing to split borrows manually.
pub struct ThothTabViewer<'a> {
    pub tabs: &'a mut HashMap<TabId, TabState>,
    pub settings: &'a Settings,
    pub persistent_state: &'a mut PersistentState,
    pub nav_capacity: usize,
    /// Search message for the focused tab, consumed by the first matching tab::ui call.
    pub search_msg: Option<(TabId, crate::search::SearchMessage)>,
    /// Outbound events collected during show_inside, drained by ThothApp afterwards.
    pub events: Vec<TabEvent>,
    /// Current theme colors for per-tab style overrides.
    pub colors: Option<crate::theme::ThemeColors>,
}

impl egui_dock::TabViewer for ThothTabViewer<'_> {
    type Tab = TabId;

    fn title(&mut self, tab_id: &mut TabId) -> egui::WidgetText {
        self.tabs
            .get(tab_id)
            .map(|t| t.title())
            .unwrap_or_else(|| "Tab".to_string())
            .into()
    }

    fn id(&mut self, tab_id: &mut TabId) -> egui::Id {
        egui::Id::new(*tab_id)
    }

    fn ui(&mut self, ui: &mut egui::Ui, tab_id: &mut TabId) {
        // Take the search message if it belongs to this tab (consumes it exactly once).
        let search_msg = if self.search_msg.as_ref().is_some_and(|(tid, _)| *tid == *tab_id) {
            self.search_msg.take().map(|(_, msg)| msg)
        } else {
            None
        };

        let Some(tab) = self.tabs.get_mut(tab_id) else {
            return;
        };

        let previous_path = tab.central_panel.get_selected_path().cloned();

        // Copy primitive settings values before the mutable borrow of tab.
        let cache_size = self.settings.performance.cache_size;
        let syntax_highlighting = self.settings.viewer.syntax_highlighting;
        let plugin_ui = tab.active_plugin_pane.as_ref().map(|p| &p.ui_output);

        let output = tab.central_panel.render(
            ui,
            CentralPanelProps {
                file_path: &tab.file_path,
                file_type: tab.file_type,
                error: &tab.error,
                search_message: search_msg,
                cache_size,
                syntax_highlighting,
                plugin_ui,
            },
        );

        // Navigation history: push if selection changed.
        let current_path = tab.central_panel.get_selected_path();
        if current_path != previous_path.as_ref() {
            if let Some(path) = current_path {
                self.events.push(TabEvent::NavigationPush {
                    tab_id: *tab_id,
                    path: path.clone(),
                });
            }
        }

        // Translate CentralPanelEvents to TabEvents.
        for event in output.events {
            use crate::components::central_panel::CentralPanelEvent;
            match event {
                CentralPanelEvent::FileOpened {
                    path,
                    file_type,
                    total_items,
                } => {
                    self.events.push(TabEvent::FileOpened {
                        tab_id: *tab_id,
                        path,
                        file_type,
                        total_items,
                    });
                }
                CentralPanelEvent::FileOpenError(err) => {
                    self.events.push(TabEvent::FileOpenError {
                        tab_id: *tab_id,
                        error: err,
                    });
                }
                CentralPanelEvent::FileClosed => {
                    self.events.push(TabEvent::FileClosed { tab_id: *tab_id });
                }
                CentralPanelEvent::FileTypeChanged(ft) => {
                    self.events.push(TabEvent::FileTypeChanged {
                        tab_id: *tab_id,
                        file_type: ft,
                    });
                }
                CentralPanelEvent::ErrorCleared => {
                    self.events.push(TabEvent::ErrorCleared { tab_id: *tab_id });
                }
                CentralPanelEvent::PluginUiEvent(evt) => {
                    self.events.push(TabEvent::PluginUiEvent {
                        tab_id: *tab_id,
                        event: evt,
                    });
                }
            }
        }
    }

    fn on_close(&mut self, tab_id: &mut TabId) -> OnCloseResponse {
        self.tabs.remove(tab_id);
        self.events.push(TabEvent::TabClosed(*tab_id));
        OnCloseResponse::Close
    }

    fn scroll_bars(&self, _tab: &TabId) -> [bool; 2] {
        [false, false]
    }

    fn clear_background(&self, _tab: &TabId) -> bool {
        false
    }

    fn tab_style_override(
        &self,
        tab_id: &TabId,
        global_style: &egui_dock::TabStyle,
    ) -> Option<egui_dock::TabStyle> {
        let c = self.colors?;
        let is_plugin = self
            .tabs
            .get(tab_id)
            .is_some_and(|t| t.active_plugin_pane.is_some());
        let accent = if is_plugin { c.accent_secondary } else { c.accent };

        let mut style = global_style.clone();
        // Active/focused: colored top accent strip via outline_color.
        style.active.outline_color = accent;
        style.focused.outline_color = accent;
        // Inactive: suppress the outline so only active tabs show the accent.
        style.inactive.outline_color = eframe::egui::Color32::TRANSPARENT;
        style.hovered.outline_color = accent.gamma_multiply(0.5);
        Some(style)
    }
}

/// Manages the dock layout and all per-tab state.
pub struct TabManager {
    pub dock_state: DockState<TabId>,
    pub tabs: HashMap<TabId, TabState>,
    next_id: usize,
}

impl TabManager {
    pub fn new(nav_capacity: usize) -> Self {
        let mut tabs = HashMap::new();
        let id: TabId = 0;
        tabs.insert(id, TabState::new(id, None, nav_capacity));
        Self {
            dock_state: DockState::new(vec![id]),
            tabs,
            next_id: 1,
        }
    }

    /// Open a file: reuse active tab if empty, otherwise create a new tab.
    pub fn open_file(&mut self, path: PathBuf, nav_capacity: usize) -> TabId {
        if let Some(id) = self.active_tab_id() {
            if self.tabs.get(&id).is_some_and(|t| t.is_empty()) {
                if let Some(tab) = self.tabs.get_mut(&id) {
                    tab.file_path = Some(path);
                    tab.error = None;
                }
                return id;
            }
        }
        let id = self.next_id;
        self.next_id += 1;
        self.tabs
            .insert(id, TabState::new(id, Some(path), nav_capacity));
        self.dock_state.push_to_focused_leaf(id);
        id
    }

    /// Get the ID of the currently focused tab, if any.
    ///
    /// Falls back to the smallest-ID tab when the dock has no focus set yet
    /// (i.e. before the first `DockArea::show_inside` call).
    pub fn active_tab_id(&mut self) -> Option<TabId> {
        if let Some((_, tab_id)) = self.dock_state.find_active_focused() {
            return Some(*tab_id);
        }
        // No focus set yet — return the first (oldest) open tab.
        self.tabs.keys().copied().min()
    }

    /// Get a mutable reference to the currently focused tab's state.
    pub fn active_tab_mut(&mut self) -> Option<&mut TabState> {
        let id = self.active_tab_id()?;
        self.tabs.get_mut(&id)
    }

    /// If all tabs were closed, insert a new empty one so the window is never blank.
    pub fn ensure_non_empty(&mut self, nav_capacity: usize) {
        if self.tabs.is_empty() {
            let id = self.next_id;
            self.next_id += 1;
            self.tabs.insert(id, TabState::new(id, None, nav_capacity));
            self.dock_state = DockState::new(vec![id]);
        }
    }

    /// Cycle the active tab by `delta` (+1 = next, -1 = previous).
    pub fn cycle_tab(&mut self, delta: i32) {
        // Get the active tab's ID (terminates the mutable borrow of dock_state).
        let active = match self.active_tab_id() {
            Some(id) => id,
            None => return,
        };

        // Find the focused leaf and set the adjacent tab active.
        let Some(path) = self.dock_state.focused_leaf() else {
            return;
        };
        if let Ok(leaf) = self.dock_state.leaf_mut(path) {
            let tabs = leaf.tabs();
            if tabs.len() < 2 {
                return;
            }
            let pos = tabs.iter().position(|&id| id == active).unwrap_or(0);
            let next_pos = (pos as i32 + delta).rem_euclid(tabs.len() as i32) as usize;
            let _ = leaf.set_active_tab(egui_dock::TabIndex(next_pos));
        }
    }

    /// Split-borrow helper: returns `(dock_state, tabs)` as independent mutable refs so
    /// `DockArea::new(dock_state)` and `ThothTabViewer { tabs, .. }` can coexist.
    pub fn borrow_parts(&mut self) -> (&mut DockState<TabId>, &mut HashMap<TabId, TabState>) {
        (&mut self.dock_state, &mut self.tabs)
    }
}
