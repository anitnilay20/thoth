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

        Self {
            settings,
            persistent_state,
            window_state: state::WindowState::default(),
            update_state: state::ApplicationUpdateState::default(),
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

        // Handle update messages (if update available, open settings in sidebar)
        if UpdateHandler::handle_update_messages(&mut self.update_state, ctx) {
            self.window_state.sidebar_expanded = true;
            self.window_state.sidebar_selected_section =
                Some(components::sidebar::SidebarSection::Settings);
        }

        // Handle file drops
        self.handle_file_drop(ctx);

        // Update window title
        self.update_window_title(ctx);

        // Get user's action from Toolbar
        self.render_toolbar(ctx);

        // Render status bar (before sidebar so it spans full width)
        self.render_status_bar(ctx);

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

        // Apply theme and font settings
        crate::theme::apply_theme(ctx, &self.settings);

        // Save settings when they have changed
        self.save_settings_if_changed();

        // Render the central panel and handle events
        self.render_central_panel(ctx, msg_to_central);

        // Render error modal if there's an error
        self.render_error_modal(ctx);

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
                            self.persistent_state.add_recent_file(path_str.to_string());
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
                    // Toggle settings section
                    let section = components::sidebar::SidebarSection::Settings;
                    if self.window_state.sidebar_expanded
                        && self.window_state.sidebar_selected_section == Some(section)
                    {
                        self.window_state.sidebar_expanded = false;
                    } else {
                        self.window_state.sidebar_expanded = true;
                        self.window_state.sidebar_selected_section = Some(section);
                    }
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
                }
                ShortcutAction::NextMatch => {
                    // TODO: Implement next match navigation
                }
                ShortcutAction::PrevMatch => {
                    // TODO: Implement previous match navigation
                }
                ShortcutAction::Escape => {
                    // Close sidebar if open
                    if self.window_state.sidebar_expanded {
                        self.window_state.sidebar_expanded = false;
                    }
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
                        self.persistent_state.add_recent_file(path_str.to_string());
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

        // Render central panel using ContextComponent trait with one-way binding
        let output = self.window_state.central_panel.render(
            ctx,
            CentralPanelProps {
                file_path: &self.window_state.file_path,
                file_type: self.window_state.file_type,
                error: &self.window_state.error,
                search_message,
            },
        );

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
                        self.persistent_state.add_recent_file(path_str.to_string());
                        let _ = self.persistent_state.save();
                    }

                    self.window_state.file_path = Some(path);
                    self.window_state.file_type = file_type;
                    self.window_state.total_items = total_items;
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
                expanded: self.window_state.sidebar_expanded,
                sidebar_width: self.persistent_state.get_sidebar_width(),
                selected_section: self.window_state.sidebar_selected_section,
                focus_search,
                update_status: &self.update_state.update_status,
                current_version: env!("CARGO_PKG_VERSION"),
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
                            self.persistent_state.add_recent_file(path_str.to_string());
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
                            if let Some(query) = msg.query() {
                                let _ = super::persistent_state::PersistentState::add_search_query(
                                    path_str,
                                    query.to_string(),
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
                components::sidebar::SidebarEvent::CheckForUpdates => {
                    // Trigger update check
                    UpdateHandler::handle_settings_action(
                        components::settings_panel::SettingsPanelEvent::CheckForUpdates,
                        &mut self.update_state,
                        ctx,
                    );
                }
                components::sidebar::SidebarEvent::DownloadUpdate => {
                    // Trigger update download
                    UpdateHandler::handle_settings_action(
                        components::settings_panel::SettingsPanelEvent::DownloadUpdate,
                        &mut self.update_state,
                        ctx,
                    );
                }
                components::sidebar::SidebarEvent::InstallUpdate => {
                    // Trigger update installation
                    UpdateHandler::handle_settings_action(
                        components::settings_panel::SettingsPanelEvent::InstallUpdate,
                        &mut self.update_state,
                        ctx,
                    );
                }
                components::sidebar::SidebarEvent::RetryUpdate => {
                    // Retry update check
                    UpdateHandler::handle_settings_action(
                        components::settings_panel::SettingsPanelEvent::RetryUpdate,
                        &mut self.update_state,
                        ctx,
                    );
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
}
