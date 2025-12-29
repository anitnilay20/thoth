use eframe::{App, Frame, egui};

use crate::{components, components::traits::ContextComponent, settings, state};

use super::{
    ShortcutAction, persistent_state::PersistentState, search_handler::SearchHandler,
    shortcut_handler::ShortcutHandler, update_handler::UpdateHandler,
};
use crate::components::central_panel::CentralPanelProps;

pub struct ThothApp {
    // Settings for this window
    pub settings: settings::Settings,

    // Persistent state (shared across app, saved to disk)
    pub persistent_state: PersistentState,

    // Window state (per-window, not persisted)
    pub window_state: state::WindowState,

    // Update state
    pub update_state: state::ApplicationUpdateState,

    // Settings dialog
    settings_dialog: components::settings_dialog::SettingsDialog,

    // Clipboard text to copy (set by shortcuts, copied in update loop)
    clipboard_text: Option<String>,

    // Track if settings need to be saved
    settings_changed: bool,
}

impl ThothApp {
    /// Create a new ThothApp with loaded settings
    pub fn new(settings: settings::Settings) -> Self {
        // Load persistent state (recent files, sidebar width, etc.)
        let persistent_state = PersistentState::default();

        // Initialize window state with saved sidebar state if remember_sidebar_state is enabled
        let mut window_state = state::WindowState::default();
        if settings.ui.remember_sidebar_state {
            window_state.sidebar_expanded = persistent_state.get_sidebar_expanded();
        }

        // Initialize navigation history with configured size
        window_state.navigation_history =
            state::NavigationHistory::with_capacity(settings.performance.navigation_history_size);

        Self {
            settings,
            persistent_state,
            window_state,
            update_state: state::ApplicationUpdateState::default(),
            settings_dialog: components::settings_dialog::SettingsDialog::default(),
            clipboard_text: None,
            settings_changed: false,
        }
    }

    /// Create a new window as an independent process
    pub fn create_new_window(&mut self) {
        use std::process::Command;

        // Get the current executable path
        if let Ok(exe_path) = std::env::current_exe() {
            // Spawn a new instance of Thoth as an independent process
            match Command::new(exe_path).spawn() {
                Ok(_) => {
                    eprintln!("New Thoth window spawned successfully");
                }
                Err(e) => {
                    eprintln!("Failed to spawn new window: {}", e);
                }
            }
        } else {
            eprintln!("Failed to get current executable path");
        }
    }

    /// Open settings dialog as a separate viewport window
    fn open_settings_window(&mut self, ctx: &egui::Context) {
        self.settings_dialog.open(&self.settings);

        // Request a repaint to trigger viewport creation
        ctx.request_repaint();
    }
}

impl App for ThothApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        // Handle keyboard shortcuts
        let shortcut_actions = ShortcutHandler::handle_shortcuts(ctx, &self.settings.shortcuts);
        self.handle_shortcut_actions(ctx, shortcut_actions);

        // Handle clipboard operations
        if let Some(text) = self.clipboard_text.take() {
            ctx.copy_text(text);
        }

        // Check for updates based on settings
        if UpdateHandler::should_check_updates(&self.update_state, &self.settings) {
            UpdateHandler::check_for_updates(&mut self.update_state);
        }

        // Handle update messages
        let should_show_updates =
            UpdateHandler::handle_update_messages(&mut self.update_state, ctx);

        // Auto-open settings on Updates tab if a new update is available
        if should_show_updates && !self.settings_dialog.open {
            self.settings_dialog.open_updates(&self.settings);
        }

        // Handle file drops
        self.handle_file_drop(ctx);

        // Update window title
        self.update_window_title(ctx);

        // Get user's action from Toolbar (if enabled)
        if self.settings.ui.show_toolbar {
            self.render_toolbar(ctx);
        }

        // Render status bar (before sidebar so it spans full width) (if enabled)
        if self.settings.ui.show_status_bar {
            self.render_status_bar(ctx);
        }

        // Render sidebar and handle events (may return search message)
        let sidebar_msg = self.render_sidebar(ctx);

        // Handle search messages from sidebar
        let (msg_to_central, search_error) = SearchHandler::handle_search_messages(
            sidebar_msg,
            &mut self.window_state.search_engine_state,
            &self.window_state.file_path,
            &self.window_state.file_type,
            ctx,
        );

        // Handle search errors
        if let Some(error) = search_error {
            self.window_state.error = Some(error);
        }

        // Render settings dialog using ContextComponent trait
        use crate::components::settings_dialog::{SettingsDialogEvent, SettingsDialogProps};
        use crate::components::traits::ContextComponent;

        let settings_output = self.settings_dialog.render(
            ctx,
            SettingsDialogProps {
                update_state: Some(&self.update_state.update_status.state),
                current_version: crate::update::UpdateManager::get_current_version(),
            },
        );

        // Apply theme (draft settings are applied inside render() when viewport is open)
        if !self.settings_dialog.open {
            crate::theme::apply_theme(ctx, &self.settings);
        }

        // Handle settings changes
        if let Some(new_settings) = settings_output.new_settings {
            self.settings = new_settings;
            self.settings_changed = true;
        }

        // Handle settings dialog events
        for event in settings_output.events {
            match event {
                SettingsDialogEvent::CheckForUpdates => {
                    UpdateHandler::check_for_updates(&mut self.update_state);
                }
                SettingsDialogEvent::DownloadUpdate => {
                    // Clone the latest release to avoid borrow checker issues
                    let latest_release =
                        if let crate::update::UpdateState::UpdateAvailable { releases, .. } =
                            &self.update_state.update_status.state
                        {
                            releases.first().cloned()
                        } else {
                            None
                        };

                    if let Some(latest) = latest_release {
                        self.update_state.pending_download_release = Some(latest.clone());
                        self.update_state.update_status.state =
                            crate::update::UpdateState::Downloading {
                                progress: 0.0,
                                version: latest.tag_name.clone(),
                            };
                        self.update_state.update_manager.download_update(&latest);
                        ctx.request_repaint();
                    }
                }
                SettingsDialogEvent::InstallUpdate => {
                    if let Some(path) = self.update_state.pending_install_path.take() {
                        self.update_state.update_status.state =
                            crate::update::UpdateState::Installing;
                        self.update_state.update_manager.install_update(path);
                    }
                }
            }
        }

        // Save settings when they have changed
        self.save_settings_if_changed();

        // Render the central panel and handle events
        self.render_central_panel(ctx, msg_to_central);

        // Render error modal if there's an error
        self.render_error_modal(ctx);

        // Render go-to-path dialog if open
        self.render_go_to_path_dialog(ctx);

        // Show profiler if enabled (only when profiling feature is enabled)
        #[cfg(feature = "profiling")]
        if self.settings.dev.show_profiler {
            // Enable puffin profiling
            puffin::GlobalProfiler::lock().new_frame();

            egui::Window::new(format!(
                "{} Profiler",
                egui_phosphor::regular::MAGNIFYING_GLASS
            ))
            .default_open(true)
            .show(ctx, |ui| {
                // Memory profiling info
                ui.collapsing("Memory Profiling (dhat)", |ui| {
                    ui.label("📊 Memory allocations are being tracked.");
                    ui.label("When you close the app, dhat-heap.json will be generated.");
                    ui.separator();
                    ui.label("To view per-component memory usage:");
                    ui.label("1. Close the app normally");
                    ui.label("2. Open https://nnethercote.github.io/dh_view/dh_view.html");
                    ui.label("3. Load dhat-heap.json");
                    ui.separator();
                    ui.label("The viewer shows which components allocate the most memory,");
                    ui.label("with full call stacks for each allocation.");
                });

                ui.separator();

                // Show puffin profiler UI with per-component breakdown
                puffin_egui::profiler_ui(ui);

                ui.separator();

                // Show frame statistics
                ui.collapsing("Frame Stats", |ui| {
                    ctx.inspection_ui(ui);
                });

                ui.separator();

                // Show additional egui settings
                ui.collapsing("Advanced Settings", |ui| {
                    ctx.settings_ui(ui);
                });
            });
        }
    }
}

impl ThothApp {
    /// Handle keyboard shortcut actions
    fn handle_shortcut_actions(&mut self, ctx: &egui::Context, actions: Vec<ShortcutAction>) {
        use rfd::FileDialog;

        for action in actions {
            match action {
                ShortcutAction::OpenFile => {
                    if let Some(path) = FileDialog::new()
                        .add_filter("JSON", &["json", "ndjson"])
                        .pick_file()
                    {
                        // Add to recent files
                        if let Some(path_str) = path.to_str() {
                            self.persistent_state.add_recent_file(
                                path_str.to_string(),
                                self.settings.performance.max_recent_files,
                            );
                            let _ = self.persistent_state.save();
                        }

                        self.window_state.file_path = Some(path);
                        self.window_state.error = None;
                    }
                }
                ShortcutAction::ClearFile => {
                    if self.window_state.file_path.is_some() {
                        // If a file is open, clear it
                        self.window_state.file_path = None;
                        self.window_state.error = None;
                    } else {
                        // If no file is open, close the window using egui's proper mechanism
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                }
                ShortcutAction::NewWindow => {
                    self.create_new_window();
                }
                ShortcutAction::Settings => {
                    // Open settings in a new window
                    self.open_settings_window(ctx);
                }
                ShortcutAction::ToggleTheme => {
                    self.settings.dark_mode = !self.settings.dark_mode;
                    self.settings_changed = true;
                }
                ShortcutAction::ToggleProfiler => {
                    self.settings.dev.show_profiler = !self.settings.dev.show_profiler;
                    self.settings_changed = true;
                }
                // Navigation shortcuts - handled by JSON viewer or search
                ShortcutAction::FocusSearch => {
                    // Toggle search section
                    let section = components::sidebar::SidebarSection::Search;
                    if self.window_state.sidebar_expanded
                        && self.window_state.sidebar_selected_section == Some(section)
                    {
                        self.window_state.sidebar_expanded = false;
                    } else {
                        self.window_state.sidebar_expanded = true;
                        self.window_state.sidebar_selected_section = Some(section);
                    }

                    // Save sidebar state if remember_sidebar_state is enabled
                    if self.settings.ui.remember_sidebar_state {
                        self.persistent_state
                            .set_sidebar_expanded(self.window_state.sidebar_expanded);
                        let _ = self.persistent_state.save();
                    }
                }
                ShortcutAction::NextMatch => {
                    // TODO: Implement next match navigation
                }
                ShortcutAction::PrevMatch => {
                    // TODO: Implement previous match navigation
                }
                ShortcutAction::NavBack => {
                    // Navigate back in history
                    if let Some(path) = self.window_state.navigation_history.back() {
                        self.window_state.central_panel.navigate_to_path(path);
                    }
                }
                ShortcutAction::NavForward => {
                    // Navigate forward in history
                    if let Some(path) = self.window_state.navigation_history.forward() {
                        self.window_state.central_panel.navigate_to_path(path);
                    }
                }
                ShortcutAction::Escape => {
                    // Close sidebar if open
                    if self.window_state.sidebar_expanded {
                        self.window_state.sidebar_expanded = false;

                        // Save sidebar state if remember_sidebar_state is enabled
                        if self.settings.ui.remember_sidebar_state {
                            self.persistent_state.set_sidebar_expanded(false);
                            let _ = self.persistent_state.save();
                        }
                    }
                }
                ShortcutAction::ToggleBookmark => {
                    // Toggle bookmark for currently selected path
                    if let Some(selected_path) = self.window_state.central_panel.get_selected_path()
                    {
                        if let Some(file_path) = &self.window_state.file_path {
                            if let Some(file_path_str) = file_path.to_str() {
                                let added = self.persistent_state.toggle_bookmark(
                                    selected_path.clone(),
                                    file_path_str.to_string(),
                                );

                                // Save bookmarks
                                if let Err(e) = self.persistent_state.save() {
                                    eprintln!("Failed to save bookmarks: {}", e);
                                }

                                // Optional: Show feedback to user
                                if added {
                                    // Could show a toast notification: "Bookmark added"
                                } else {
                                    // Could show a toast notification: "Bookmark removed"
                                }
                            }
                        }
                    }
                }
                ShortcutAction::GoToPath => {
                    // Open the go-to-path dialog
                    self.window_state.go_to_path_dialog_open = true;
                }
                // Tree operations
                ShortcutAction::ExpandNode => {
                    self.window_state.central_panel.expand_selected_node();
                }
                ShortcutAction::CollapseNode => {
                    self.window_state.central_panel.collapse_selected_node();
                }
                ShortcutAction::ExpandAll => {
                    self.window_state.central_panel.expand_all_nodes();
                }
                ShortcutAction::CollapseAll => {
                    self.window_state.central_panel.collapse_all_nodes();
                }
                // Movement operations
                ShortcutAction::MoveUp => {
                    self.window_state.central_panel.move_selection_up();
                }
                ShortcutAction::MoveDown => {
                    self.window_state.central_panel.move_selection_down();
                }
                // Clipboard operations
                ShortcutAction::CopyKey => {
                    if let Some(text) = self.window_state.central_panel.copy_selected_key() {
                        self.clipboard_text = Some(text);
                    }
                }
                ShortcutAction::CopyValue => {
                    if let Some(text) = self.window_state.central_panel.copy_selected_value() {
                        self.clipboard_text = Some(text);
                    }
                }
                ShortcutAction::CopyObject => {
                    if let Some(text) = self.window_state.central_panel.copy_selected_object() {
                        self.clipboard_text = Some(text);
                    }
                }
                ShortcutAction::CopyPath => {
                    if let Some(text) = self.window_state.central_panel.copy_selected_path() {
                        self.clipboard_text = Some(text);
                    }
                }
            }
        }
    }

    /// Update window title based on current file
    fn update_window_title(&self, ctx: &egui::Context) {
        let title = if let Some(path) = &self.window_state.file_path {
            let file_name = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown file");
            format!("Thoth — {}", file_name)
        } else {
            "Thoth — JSON & NDJSON Viewer".to_owned()
        };
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
    }

    /// Render toolbar
    fn render_toolbar(&mut self, ctx: &egui::Context) {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        // Render toolbar using ContextComponent trait with one-way binding
        let output = self.window_state.toolbar.render(
            ctx,
            components::toolbar::ToolbarProps {
                file_type: &self.window_state.file_type,
                dark_mode: self.settings.dark_mode,
                shortcuts: &self.settings.shortcuts,
                file_path: self.window_state.file_path.as_deref(),
                is_fullscreen: ctx.input(|i| i.viewport().fullscreen.unwrap_or(false)),
            },
        );

        // Handle events emitted by the toolbar (bottom-to-top communication)
        for event in output.events {
            match event {
                components::toolbar::ToolbarEvent::FileOpen { path, file_type } => {
                    // Add to recent files
                    if let Some(path_str) = path.to_str() {
                        self.persistent_state.add_recent_file(
                            path_str.to_string(),
                            self.settings.performance.max_recent_files,
                        );
                        let _ = self.persistent_state.save();
                    }

                    self.window_state.file_path = Some(path);
                    self.window_state.file_type = file_type;
                    self.window_state.error = None;
                }
                components::toolbar::ToolbarEvent::FileClear => {
                    self.window_state.file_path = None;
                    self.window_state.error = None;
                }
                components::toolbar::ToolbarEvent::NewWindow => {
                    self.create_new_window();
                }
                components::toolbar::ToolbarEvent::FileTypeChange(file_type) => {
                    self.window_state.file_type = file_type;
                }
                components::toolbar::ToolbarEvent::ToggleTheme => {
                    self.settings.dark_mode = !self.settings.dark_mode;
                    self.settings_changed = true;
                }
                components::toolbar::ToolbarEvent::OpenSettings => {
                    // Open settings in a new window
                    self.open_settings_window(ctx);
                }
            }
        }
    }

    /// Save settings if they have changed
    fn save_settings_if_changed(&mut self) {
        if self.settings_changed {
            if let Err(e) = self.settings.save() {
                eprintln!("Failed to save settings: {}", e);
            }
            self.settings_changed = false;
        }
    }

    /// Render status bar
    fn render_status_bar(&mut self, ctx: &egui::Context) {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        // Determine status based on search state
        let search = &self.window_state.search_engine_state.search;
        let status = if search.scanning {
            components::status_bar::StatusBarStatus::Searching
        } else if !search.query.is_empty() && !search.results.is_empty() {
            components::status_bar::StatusBarStatus::Filtered
        } else if self.window_state.error.is_some() {
            components::status_bar::StatusBarStatus::Error
        } else {
            components::status_bar::StatusBarStatus::Ready
        };

        // Get item counts from window state
        let item_count = self.window_state.total_items;
        let filtered_count = if !search.results.is_empty() {
            Some(search.results.len())
        } else {
            None
        };

        self.window_state.status_bar.render(
            ctx,
            components::status_bar::StatusBarProps {
                file_path: self.window_state.file_path.as_deref(),
                file_type: &self.window_state.file_type,
                item_count,
                filtered_count,
                status,
            },
        );
    }

    /// Render central panel and handle events
    fn render_central_panel(
        &mut self,
        ctx: &egui::Context,
        search_message: Option<crate::search::SearchMessage>,
    ) {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        // Track path selection changes for navigation history
        let previous_path = self.window_state.central_panel.get_selected_path().cloned();

        // Render central panel using ContextComponent trait with one-way binding
        let output = self.window_state.central_panel.render(
            ctx,
            CentralPanelProps {
                file_path: &self.window_state.file_path,
                file_type: self.window_state.file_type,
                error: &self.window_state.error,
                search_message,
                cache_size: self.settings.performance.cache_size,
                syntax_highlighting: self.settings.viewer.syntax_highlighting,
            },
        );

        // After rendering, check if path changed and add to navigation history
        let current_path = self.window_state.central_panel.get_selected_path();
        if current_path != previous_path.as_ref() {
            if let Some(path) = current_path {
                self.window_state.navigation_history.push(path.clone());
            }
        }

        // Handle events emitted by the central panel (bottom-to-top communication)
        for event in output.events {
            match event {
                components::central_panel::CentralPanelEvent::FileOpened {
                    path,
                    file_type,
                    total_items,
                } => {
                    // Add to recent files
                    if let Some(path_str) = path.to_str() {
                        self.persistent_state.add_recent_file(
                            path_str.to_string(),
                            self.settings.performance.max_recent_files,
                        );
                        let _ = self.persistent_state.save();
                    }

                    self.window_state.file_path = Some(path);
                    self.window_state.file_type = file_type;
                    self.window_state.total_items = total_items;

                    // Apply pending navigation if exists
                    if let Some(pending_path) = self.window_state.pending_navigation.take() {
                        self.window_state
                            .central_panel
                            .navigate_to_path(pending_path);
                    }
                }
                components::central_panel::CentralPanelEvent::FileOpenError(msg) => {
                    self.window_state.error = Some(msg);
                }
                components::central_panel::CentralPanelEvent::FileClosed => {
                    self.window_state.file_path = None;
                    self.window_state.total_items = 0;
                }
                components::central_panel::CentralPanelEvent::FileTypeChanged(file_type) => {
                    self.window_state.file_type = file_type;
                }
                components::central_panel::CentralPanelEvent::ErrorCleared => {
                    self.window_state.error = None;
                }
            }
        }
    }

    /// Render sidebar and handle its events
    fn render_sidebar(&mut self, ctx: &egui::Context) -> Option<crate::search::SearchMessage> {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        use crate::components::traits::ContextComponent;

        // Determine if search should receive focus
        // Focus when:
        // 1. Section changed to Search (from a different section)
        // 2. OR sidebar was just expanded with Search section (reopening)
        let section_changed_to_search = self.window_state.sidebar_selected_section
            == Some(components::sidebar::SidebarSection::Search)
            && self.window_state.previous_sidebar_section
                != Some(components::sidebar::SidebarSection::Search);

        let sidebar_reopened_with_search = self.window_state.sidebar_expanded
            && !self.window_state.previous_sidebar_expanded
            && self.window_state.sidebar_selected_section
                == Some(components::sidebar::SidebarSection::Search);

        let focus_search = section_changed_to_search || sidebar_reopened_with_search;

        // Render sidebar
        let output = self.window_state.sidebar.render(
            ctx,
            components::sidebar::SidebarProps {
                recent_files: self.persistent_state.get_recent_files(),
                bookmarks: self.persistent_state.get_bookmarks(),
                current_file_path: self
                    .window_state
                    .file_path
                    .as_ref()
                    .and_then(|path| path.to_str()),
                expanded: self.window_state.sidebar_expanded,
                sidebar_width: self.persistent_state.get_sidebar_width(),
                selected_section: self.window_state.sidebar_selected_section,
                focus_search,
                search_state: &self.window_state.search_engine_state.search,
                search_history: self
                    .window_state
                    .file_path
                    .as_ref()
                    .and_then(|path| path.to_str())
                    .and_then(|path_str| {
                        super::persistent_state::PersistentState::load_search_history(path_str).ok()
                    })
                    .as_ref(),
            },
        );

        // Update previous states after rendering so focus_search is only true for one frame
        if focus_search {
            self.window_state.previous_sidebar_section = self.window_state.sidebar_selected_section;
        }
        self.window_state.previous_sidebar_expanded = self.window_state.sidebar_expanded;

        // Handle sidebar events
        for event in output.events {
            match event {
                components::sidebar::SidebarEvent::OpenFile(file_path) => {
                    // Open the file by setting the path
                    let path = std::path::PathBuf::from(&file_path);
                    self.window_state.file_path = Some(path);
                    self.window_state.error = None;
                }
                components::sidebar::SidebarEvent::RemoveRecentFile(file_path) => {
                    // Remove from recent files
                    self.persistent_state.remove_recent_file(&file_path);
                    if let Err(e) = self.persistent_state.save() {
                        eprintln!("Failed to save recent files: {}", e);
                    }
                }
                components::sidebar::SidebarEvent::OpenFilePicker => {
                    // Open file picker dialog
                    use rfd::FileDialog;
                    if let Some(path) = FileDialog::new()
                        .add_filter("JSON", &["json", "ndjson"])
                        .pick_file()
                    {
                        // Add to recent files
                        if let Some(path_str) = path.to_str() {
                            self.persistent_state.add_recent_file(
                                path_str.to_string(),
                                self.settings.performance.max_recent_files,
                            );
                            let _ = self.persistent_state.save();
                        }

                        self.window_state.file_path = Some(path);
                        self.window_state.error = None;
                    }
                }
                components::sidebar::SidebarEvent::SectionToggled(section) => {
                    // Toggle logic: if clicking same section while expanded, collapse; otherwise open to that section
                    if self.window_state.sidebar_expanded
                        && self.window_state.sidebar_selected_section == Some(section)
                    {
                        self.window_state.sidebar_expanded = false;
                        self.window_state.previous_sidebar_section =
                            self.window_state.sidebar_selected_section;
                    } else {
                        self.window_state.previous_sidebar_section =
                            self.window_state.sidebar_selected_section;
                        self.window_state.sidebar_expanded = true;
                        self.window_state.sidebar_selected_section = Some(section);
                    }

                    // Save sidebar state if remember_sidebar_state is enabled
                    if self.settings.ui.remember_sidebar_state {
                        self.persistent_state
                            .set_sidebar_expanded(self.window_state.sidebar_expanded);
                        let _ = self.persistent_state.save();
                    }
                }
                components::sidebar::SidebarEvent::WidthChanged(new_width) => {
                    // Save the new sidebar width
                    self.persistent_state.set_sidebar_width(new_width);
                    let _ = self.persistent_state.save();
                }
                components::sidebar::SidebarEvent::Search(msg) => {
                    // Save search query to history
                    if let Some(file_path) = &self.window_state.file_path {
                        if let Some(path_str) = file_path.to_str() {
                            if let Some(entry) = msg.history_entry() {
                                let _ = super::persistent_state::PersistentState::add_search_query(
                                    path_str, entry,
                                );
                            }
                        }
                    }
                    // Handle search from sidebar
                    return Some(msg);
                }
                components::sidebar::SidebarEvent::NavigateToSearchResult { record_index } => {
                    // Navigate to the selected search result in the main view
                    self.window_state
                        .central_panel
                        .navigate_to_record(record_index);
                }
                components::sidebar::SidebarEvent::ClearSearchHistory => {
                    // Clear search history for the current file
                    if let Some(file_path) = &self.window_state.file_path {
                        if let Some(path_str) = file_path.to_str() {
                            let _ = super::persistent_state::PersistentState::clear_search_history(
                                path_str,
                            );
                        }
                    }
                }
                components::sidebar::SidebarEvent::NavigateToBookmark { file_path, path } => {
                    // Check if we need to open a different file
                    let current_file = self
                        .window_state
                        .file_path
                        .as_ref()
                        .and_then(|p| p.to_str());

                    if current_file != Some(file_path.as_str()) {
                        // Open the bookmarked file
                        let path_buf = std::path::PathBuf::from(&file_path);
                        self.window_state.file_path = Some(path_buf);
                        self.window_state.error = None;

                        // Store pending navigation to apply after file loads
                        self.window_state.pending_navigation = Some(path.clone());

                        // Add to recent files
                        self.persistent_state.add_recent_file(
                            file_path.clone(),
                            self.settings.performance.max_recent_files,
                        );
                        let _ = self.persistent_state.save();
                    } else {
                        // Navigate immediately if same file
                        self.window_state.central_panel.navigate_to_path(path);
                    }
                }
                components::sidebar::SidebarEvent::RemoveBookmark(index) => {
                    // Remove the bookmark
                    self.persistent_state.remove_bookmark(index);
                    if let Err(e) = self.persistent_state.save() {
                        eprintln!("Failed to save bookmarks: {}", e);
                    }
                }
            }
        }

        None
    }

    fn render_error_modal(&mut self, ctx: &egui::Context) {
        use crate::components::traits::StatefulComponent;
        use crate::error::RecoveryAction;

        // Only render if there's an error
        if let Some(error) = &self.window_state.error {
            // Create a temporary UI to pass to the modal
            let mut output = None;
            egui::Area::new("error_modal_area".into())
                .movable(false)
                .interactable(false)
                .show(ctx, |ui| {
                    output = Some(self.window_state.error_modal.render(
                        ui,
                        components::error_modal::ErrorModalProps { error, open: true },
                    ));
                });

            let Some(output) = output else { return };

            // Handle error modal events and recovery actions
            for event in output.events {
                match event {
                    components::error_modal::ErrorModalEvent::Close => {
                        self.window_state.error = None;
                    }
                    components::error_modal::ErrorModalEvent::Retry => {
                        // Clear error and retry the operation
                        // For file operations, trigger a reload by clearing and restoring the path
                        if let Some(path) = self.window_state.file_path.take() {
                            self.window_state.error = None;
                            // Setting the path again triggers the reload logic in central_panel
                            self.window_state.file_path = Some(path);
                        } else {
                            self.window_state.error = None;
                        }
                    }
                    components::error_modal::ErrorModalEvent::Reset => {
                        // Reset to initial state
                        self.window_state.error = None;
                        self.window_state.file_path = None;
                        self.window_state.total_items = 0;
                        self.window_state.search_engine_state.search =
                            crate::search::Search::default();
                    }
                }
            }

            // Handle automatic recovery actions
            if let Some(recovery_action) = output.recovery_action {
                match recovery_action {
                    RecoveryAction::ClearError => {
                        self.window_state.error = None;
                    }
                    RecoveryAction::Reset => {
                        self.window_state.error = None;
                        self.window_state.file_path = None;
                        self.window_state.total_items = 0;
                    }
                    _ => {}
                }
            }
        }
    }

    fn render_go_to_path_dialog(&mut self, ctx: &egui::Context) {
        use crate::components::traits::StatefulComponent;

        // Get theme colors
        let theme_colors = ctx.memory(|mem| {
            mem.data
                .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| {
                    crate::theme::Theme::for_dark_mode(ctx.style().visuals.dark_mode).colors()
                })
        });

        // Render the dialog using Area so it overlays everything
        let mut output = None;
        egui::Area::new("go_to_path_dialog_area".into())
            .movable(false)
            .interactable(true)
            .show(ctx, |ui| {
                output = Some(self.window_state.go_to_path_dialog.render(
                    ui,
                    components::go_to_path_dialog::GoToPathDialogProps {
                        open: self.window_state.go_to_path_dialog_open,
                        theme_colors: &theme_colors,
                    },
                ));
            });

        let Some(output) = output else { return };

        // Handle dialog events
        for event in output.events {
            match event {
                components::go_to_path_dialog::GoToPathDialogEvent::NavigateToPath(path) => {
                    // Navigate to the specified path
                    self.window_state.central_panel.navigate_to_path(path);
                    self.window_state.go_to_path_dialog_open = false;
                }
                components::go_to_path_dialog::GoToPathDialogEvent::Close => {
                    self.window_state.go_to_path_dialog_open = false;
                }
            }
        }
    }
}
