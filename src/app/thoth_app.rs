use eframe::{App, Frame, egui};
use std::path::PathBuf;

use crate::{
    NOTIFICATION_MANAGER, PLUGIN_MANAGER,
    app::{file_picker, pick_file},
    components::{self, traits::ContextComponent},
    settings, state,
};

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
    /// Create a new ThothApp with loaded settings and optional file to open
    pub fn new(settings: settings::Settings, file_to_open: Option<PathBuf>) -> Self {
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

        // If a file path was provided via command line, set it up to load
        if let Some(path) = file_to_open {
            window_state.file_path = Some(path);
            // The file will be loaded in the first update() call via the file loading logic
        }

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

    /// Apply a new `Settings` value, running all required side-effects.
    /// Called both from the settings dialog output and from `Settings::take_if_dirty`.
    fn apply_new_settings(&mut self, new_settings: settings::Settings) {
        let prev_remember_sidebar = self.settings.ui.remember_sidebar_state;
        let prev_plugins_enabled = self.settings.plugins.enabled;
        let prev_disabled_plugin_ids = self.settings.plugins.disabled_plugin_ids.clone();

        self.settings = new_settings;
        self.settings_changed = true;

        if !prev_remember_sidebar && self.settings.ui.remember_sidebar_state {
            self.window_state.sidebar_expanded = self.persistent_state.get_sidebar_expanded();
        }

        if let Some(Some(pm)) = PLUGIN_MANAGER.get() {
            pm.update_plugin_settings(self.settings.plugins.plugin_settings.clone());
        }

        if let Some(pane) = self.window_state.active_plugin_pane.as_mut() {
            let updated = self
                .settings
                .plugins
                .plugin_settings
                .get(&pane.plugin_id)
                .cloned()
                .unwrap_or_default();
            if let Err(e) = pane.loader.on_setting_change(&updated) {
                eprintln!(
                    "Failed to notify plugin '{}' of setting change: {e}",
                    pane.plugin_id
                );
            }
        }

        let plugins_changed = self.settings.plugins.enabled != prev_plugins_enabled
            || self.settings.plugins.disabled_plugin_ids != prev_disabled_plugin_ids;
        if plugins_changed {
            crate::notification::NotificationManager::notify(
                crate::notification::Notification::new(
                    "Restart required",
                    "Plugin changes take effect after restarting the app.",
                )
                .with_id("plugin_restart_required")
                .with_action(
                    "Restart Now",
                    std::sync::Arc::new(|| {
                        if let Ok(exe) = std::env::current_exe() {
                            let _ = std::process::Command::new(exe).spawn();
                        }
                        std::process::exit(0);
                    }),
                )
                .with_action("Later", std::sync::Arc::new(|| {}))
                .with_toast(true)
                .with_status(crate::notification::NotificationStatus::ConsentRequired),
            );
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

        if let Some(nm) = NOTIFICATION_MANAGER.get() {
            if let Ok(mut nm) = nm.lock() {
                nm.show_notifications(ctx);
            }
        }

        // Handle file drops
        self.handle_file_drop(ctx);

        // Update window title
        self.update_window_title(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut Frame) {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        let ctx = ui.ctx().clone();

        // Publish settings into egui context so any component can read or mutate them.
        settings::Settings::store(&ctx, &self.settings);

        // Poll completed async HTTP requests from the active plugin pane.
        // Must happen before rendering so the updated ui_output is used this frame.
        self.poll_plugin_http_results(&ctx);

        // Get user's action from Toolbar (if enabled)
        if self.settings.ui.show_toolbar {
            self.render_toolbar(ui);
        }

        // Render status bar (before sidebar so it spans full width) (if enabled)
        if self.settings.ui.show_status_bar {
            self.render_status_bar(ui);
        }

        // Handle clipboard operations
        if let Some(text) = self.clipboard_text.take() {
            ctx.copy_text(text);
        }

        // Render sidebar and handle events (may return search message)
        let sidebar_msg = self.render_sidebar(ui);

        // Handle search messages from sidebar
        let (msg_to_central, search_error) = SearchHandler::handle_search_messages(
            sidebar_msg,
            &mut self.window_state.search_engine_state,
            &self.window_state.file_path,
            &self.window_state.file_type,
            &ctx,
        );

        // Handle search errors
        if let Some(error) = search_error {
            self.window_state.error = Some(error);
        }

        // Handle keyboard shortcuts
        let shortcut_actions =
            ShortcutHandler::handle_shortcuts(ui.ctx(), &self.settings.shortcuts);
        self.handle_shortcut_actions(ui.ctx(), shortcut_actions);

        // Render settings dialog using ContextComponent trait
        use crate::components::settings_dialog::{SettingsDialogEvent, SettingsDialogProps};
        use crate::components::traits::ContextComponent;

        let settings_output = self.settings_dialog.render(
            ui,
            SettingsDialogProps {
                update_state: Some(&self.update_state.update_status.state),
                last_check: self.update_state.update_status.last_check,
                current_version: crate::update::UpdateManager::get_current_version(),
            },
        );

        // Apply theme (draft settings are applied inside render() when viewport is open)
        if !self.settings_dialog.open {
            crate::theme::apply_theme(&ctx, &self.settings);
        }

        // Handle settings changes from the dialog
        if let Some(new_settings) = settings_output.new_settings {
            self.apply_new_settings(new_settings);
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
                SettingsDialogEvent::RegisterInPath => {
                    match crate::platform::path_registry::register_in_path() {
                        Ok(()) => {
                            ctx.request_repaint();
                        }
                        Err(e) => {
                            self.window_state.error = Some(e);
                        }
                    }
                }
                SettingsDialogEvent::UnregisterFromPath => {
                    match crate::platform::path_registry::unregister_from_path() {
                        Ok(()) => {
                            ctx.request_repaint();
                        }
                        Err(e) => {
                            self.window_state.error = Some(e);
                        }
                    }
                }
            }
        }

        // Render the central panel and handle events.
        // When the marketplace section is active, show the plugin detail pane instead.
        if self.window_state.sidebar_selected_section
            == Some(components::sidebar::SidebarSection::MarketPlace)
        {
            use crate::components::marketplace::{MarketplaceDetail, MarketplaceDetailProps};
            use crate::components::traits::StatelessComponent;
            MarketplaceDetail::render(ui, MarketplaceDetailProps);
        } else {
            self.render_central_panel(ui, msg_to_central);
        }

        // Render error modal if there's an error
        self.render_error_modal(&ctx);

        // Collect settings mutations from any component rendered this frame,
        // then save. Done last so every component has had a chance to call
        // Settings::update() before we drain and persist.
        if let Some(new_settings) = settings::Settings::take_if_dirty(&ctx) {
            self.apply_new_settings(new_settings);
        }
        self.save_settings_if_changed();

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
            .show(&ctx, |ui| {
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
        for action in actions {
            match action {
                ShortcutAction::OpenFile => {
                    if let Some(path) = file_picker::pick_file(self.settings.plugins.enabled) {
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
                        && self.window_state.sidebar_selected_section == Some(section.clone())
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
                ShortcutAction::OpenBookmarks => {
                    // Open bookmarks panel in sidebar
                    self.window_state.sidebar_expanded = true;
                    self.window_state.sidebar_selected_section =
                        Some(components::sidebar::SidebarSection::Bookmarks);

                    // Save sidebar state if remember_sidebar_state is enabled
                    if self.settings.ui.remember_sidebar_state {
                        self.persistent_state.set_sidebar_expanded(true);
                        let _ = self.persistent_state.save();
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

    /// Forward `event` to the active plugin pane, refresh `ui_output`, and
    /// update the sidebar. Sets `window_state.error` on failure.
    fn dispatch_plugin_event(&mut self, event: crate::plugin::render_node::UiEvent) {
        if let Some(pane) = self.window_state.active_plugin_pane.as_mut() {
            match pane.loader.handle_event(event) {
                Ok(new_output) => {
                    pane.ui_output = new_output;
                    if let Ok(sidebar) = pane.loader.render_sidebar() {
                        self.window_state.plugin_sidebar_output = sidebar;
                    }
                }
                Err(e) => {
                    self.window_state.error = Some(crate::error::ThothError::Unknown {
                        message: e.to_string(),
                    });
                }
            }
        }
    }

    /// Drain completed async HTTP requests from the active plugin pane and
    /// forward each result to the plugin via `handle_event`.  Must be called
    /// before any rendering so the updated `ui_output` is used in this frame.
    fn poll_plugin_http_results(&mut self, ctx: &egui::Context) {
        use crate::plugin::render_node::UiEvent;

        // Collect all pending results first so we can drop the pane borrow
        // before calling dispatch_plugin_event (which also borrows self).
        let (http_events, retry_requests, needs_repaint) = {
            let Some(pane) = self.window_state.active_plugin_pane.as_mut() else {
                return;
            };
            let http_events: Vec<UiEvent> = pane
                .loader
                .drain_http_results()
                .into_iter()
                .map(|(request_id, outcome)| {
                    let value = match outcome {
                        Ok(raw) => {
                            let body = String::from_utf8_lossy(&raw.body).to_string();
                            serde_json::json!({
                                "ok": {
                                    "status": raw.status,
                                    "headers": raw.headers,
                                    "body": body,
                                    "duration_ms": raw.duration_ms
                                }
                            })
                            .to_string()
                        }
                        Err(msg) => {
                            // Use a structured code for consent so plugins can
                            // detect it reliably without string-matching the message.
                            let code = if msg.contains("waiting for user consent") {
                                "consent_pending"
                            } else {
                                "error"
                            };
                            serde_json::json!({"err": {"code": code, "message": msg}}).to_string()
                        }
                    };
                    UiEvent {
                        widget_id: request_id,
                        kind: "http-response".to_string(),
                        value,
                    }
                })
                .collect();

            let retry_requests = pane.loader.drain_retry_requests();
            let needs_repaint = pane.loader.has_pending_http();
            (http_events, retry_requests, needs_repaint)
        };

        for event in http_events {
            self.dispatch_plugin_event(event);
        }

        // Re-dispatch any requests the user approved via consent notifications.
        for (request_id, req) in retry_requests {
            if let Some(pane) = self.window_state.active_plugin_pane.as_mut() {
                pane.loader.dispatch_approved_request(request_id, req);
            }
            // Notify the plugin so it can switch its spinner text from
            // "Waiting for consent" to "Sending request…".
            self.dispatch_plugin_event(UiEvent {
                widget_id: "consent-approved".to_string(),
                kind: "notify".to_string(),
                value: String::new(),
            });
            ctx.request_repaint();
        }

        if needs_repaint {
            ctx.request_repaint();
        }
    }

    /// Render toolbar
    fn render_toolbar(&mut self, ui: &mut egui::Ui) {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        // Render toolbar using ContextComponent trait with one-way binding
        let can_go_back = self.window_state.navigation_history.can_go_back();
        let can_go_forward = self.window_state.navigation_history.can_go_forward();

        let output = self.window_state.toolbar.render(
            ui,
            components::toolbar::ToolbarProps {
                file_type: &self.window_state.file_type,
                dark_mode: self.settings.dark_mode,
                shortcuts: &self.settings.shortcuts,
                file_path: self.window_state.file_path.as_deref(),
                is_fullscreen: ui
                    .ctx()
                    .input(|i: &egui::InputState| i.viewport().fullscreen.unwrap_or(false)),
                can_go_back,
                can_go_forward,
                plugins_enabled: self.settings.plugins.enabled,
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
                components::toolbar::ToolbarEvent::ToggleTheme => {
                    self.settings.dark_mode = !self.settings.dark_mode;
                    self.settings_changed = true;
                }
                components::toolbar::ToolbarEvent::OpenSettings => {
                    // Open settings in a new window
                    self.open_settings_window(ui.ctx());
                }
                components::toolbar::ToolbarEvent::NavigateBack => {
                    // Navigate back in history
                    if let Some(path) = self.window_state.navigation_history.back() {
                        self.window_state.central_panel.navigate_to_path(path);
                    }
                }
                components::toolbar::ToolbarEvent::NavigateForward => {
                    // Navigate forward in history
                    if let Some(path) = self.window_state.navigation_history.forward() {
                        self.window_state.central_panel.navigate_to_path(path);
                    }
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
    fn render_status_bar(&mut self, ui: &mut egui::Ui) {
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

        // Get selected path for breadcrumbs
        let selected_path = self.window_state.central_panel.get_selected_path();

        let status_bar_output = self.window_state.status_bar.render(
            ui,
            components::status_bar::StatusBarProps {
                file_path: self.window_state.file_path.as_deref(),
                file_type: &self.window_state.file_type,
                item_count,
                filtered_count,
                status,
                selected_path: selected_path.as_ref().map(|s| s.as_str()),
            },
        );

        // Handle status bar events
        for event in status_bar_output.events {
            match event {
                components::status_bar::StatusBarEvent::NavigateToPath(path) => {
                    // Track in navigation history before navigating
                    self.window_state.navigation_history.push(path.clone());
                    self.window_state.central_panel.navigate_to_path(path);
                }
            }
        }
    }

    /// Render central panel and handle events
    fn render_central_panel(
        &mut self,
        ui: &mut egui::Ui,
        search_message: Option<crate::search::SearchMessage>,
    ) {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        // Track path selection changes for navigation history
        let previous_path = self.window_state.central_panel.get_selected_path().cloned();

        // Render central panel using ContextComponent trait with one-way binding
        let output = self.window_state.central_panel.render(
            ui,
            CentralPanelProps {
                file_path: &self.window_state.file_path,
                file_type: self.window_state.file_type,
                error: &self.window_state.error,
                search_message,
                cache_size: self.settings.performance.cache_size,
                syntax_highlighting: self.settings.viewer.syntax_highlighting,
                plugin_ui: self
                    .window_state
                    .active_plugin_pane
                    .as_ref()
                    .map(|p| &p.ui_output),
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
                    // Clear any plugin pane so the file viewer takes over.
                    self.window_state.active_plugin_pane = None;
                    self.window_state.plugin_sidebar_output = None;

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
                components::central_panel::CentralPanelEvent::PluginUiEvent(evt) => {
                    self.dispatch_plugin_event(evt);
                }
            }
        }
    }

    /// Render sidebar and handle its events
    fn render_sidebar(&mut self, ui: &mut egui::Ui) -> Option<crate::search::SearchMessage> {
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

        // Collect data-source plugins before constructing SidebarProps so the
        // Vec<&Plugin> lives long enough for the borrow in the struct literal.
        let ds_plugins: Vec<&crate::plugin::Plugin> = PLUGIN_MANAGER
            .get()
            .and_then(|m| m.as_ref())
            .map(|m| m.get_data_source_plugins())
            .unwrap_or_default();

        // Build plugin sidebar info if there's an active plugin pane with sidebar content.
        // Capture owned strings so they outlive the borrows in SidebarProps.
        let plugin_sidebar_strings: Option<(String, String, Option<String>)> = self
            .window_state
            .active_plugin_pane
            .as_ref()
            .and_then(|pane| {
                PLUGIN_MANAGER
                    .get()
                    .and_then(|m| m.as_ref())
                    .and_then(|m| m.registry.get_by_id(&pane.plugin_id))
                    .map(|p| (p.id.clone(), p.name.clone(), p.icon.clone()))
            });
        let plugin_sidebar_prop: Option<components::sidebar::PluginSidebarInfo<'_>> = match (
            &plugin_sidebar_strings,
            &self.window_state.plugin_sidebar_output,
        ) {
            (Some((id, name, icon)), Some(output)) => {
                Some(components::sidebar::PluginSidebarInfo {
                    plugin_id: id.as_str(),
                    plugin_name: name.as_str(),
                    icon: icon.as_deref(),
                    output,
                })
            }
            _ => None,
        };

        // Render sidebar
        let output = self.window_state.sidebar.render(
            ui,
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
                selected_section: self.window_state.sidebar_selected_section.clone(),
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
                data_source_plugins: &ds_plugins,
                active_datasource_plugin_id: self
                    .window_state
                    .active_plugin_pane
                    .as_ref()
                    .map(|p| p.plugin_id.as_str()),
                plugin_sidebar: plugin_sidebar_prop,
            },
        );

        // Update previous states after rendering so focus_search is only true for one frame
        if focus_search {
            self.window_state.previous_sidebar_section =
                self.window_state.sidebar_selected_section.clone();
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
                    if let Some(path) = pick_file(self.settings.plugins.enabled) {
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
                    if let components::sidebar::SidebarSection::DataSource { ref plugin_id } =
                        section
                    {
                        if self
                            .window_state
                            .active_plugin_pane
                            .as_ref()
                            .is_none_or(|p| &p.plugin_id != plugin_id)
                        {
                            // Open: create a loader, call render_ui(), store in active_plugin_pane.
                            if let Some(manager) = PLUGIN_MANAGER.get().and_then(|m| m.as_ref()) {
                                if let Some(plugin) = manager.registry.get_by_id(plugin_id) {
                                    use crate::plugin::network_policy::NetworkPolicy;
                                    let user_policy = self
                                        .settings
                                        .plugins
                                        .network_policies
                                        .get(plugin_id)
                                        .cloned()
                                        .unwrap_or_default();
                                    let policy = NetworkPolicy::from_plugin_and_settings(
                                        &plugin.network.clone().unwrap_or_default(),
                                        &user_policy,
                                    );
                                    match manager.open_data_source(plugin_id, policy) {
                                        Ok(loader) => {
                                            match loader.render_ui() {
                                                Ok(ui_output) => {
                                                    self.window_state.file_path = None;
                                                    // Fetch initial sidebar output
                                                    let sidebar_output =
                                                        loader.render_sidebar().ok().flatten();
                                                    let has_sidebar = sidebar_output.is_some();
                                                    self.window_state.plugin_sidebar_output =
                                                        sidebar_output;
                                                    self.window_state.active_plugin_pane =
                                                        Some(crate::state::ActivePluginPane {
                                                            plugin_id: plugin_id.clone(),
                                                            display_url: String::new(),
                                                            ui_output,
                                                            loader,
                                                        });
                                                    // Open sidebar to plugin section if it has content, otherwise close it
                                                    if has_sidebar {
                                                        self.window_state.sidebar_expanded = true;
                                                        self.window_state.sidebar_selected_section =
                                                            Some(components::sidebar::SidebarSection::PluginSidebar {
                                                                plugin_id: plugin_id.clone(),
                                                            });
                                                    } else {
                                                        self.window_state.sidebar_expanded = false;
                                                        self.window_state
                                                            .sidebar_selected_section = None;
                                                    }
                                                }
                                                Err(e) => {
                                                    self.window_state.error =
                                                        Some(crate::error::ThothError::Unknown {
                                                            message: format!(
                                                                "Plugin UI error: {e}"
                                                            ),
                                                        });
                                                }
                                            }
                                        }
                                        Err(e) => {
                                            self.window_state.error =
                                                Some(crate::error::ThothError::Unknown {
                                                    message: format!("Failed to load plugin: {e}"),
                                                });
                                        }
                                    }
                                }
                            }
                        }
                    } else {
                        // Toggle logic: if clicking same section while expanded, collapse; otherwise open to that section
                        let is_plugin_sidebar = matches!(
                            section,
                            components::sidebar::SidebarSection::PluginSidebar { .. }
                        );
                        if self.window_state.sidebar_expanded
                            && self.window_state.sidebar_selected_section == Some(section.clone())
                        {
                            self.window_state.sidebar_expanded = false;
                            self.window_state.previous_sidebar_section =
                                self.window_state.sidebar_selected_section.clone();
                        } else {
                            self.window_state.previous_sidebar_section =
                                self.window_state.sidebar_selected_section.clone();
                            self.window_state.sidebar_expanded = true;
                            self.window_state.sidebar_selected_section = Some(section);
                            // Don't close the plugin pane when switching to its own sidebar
                            if !is_plugin_sidebar {
                                self.window_state.active_plugin_pane = None;
                                self.window_state.plugin_sidebar_output = None;
                            }
                        }
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
                        // Track in navigation history before navigating
                        self.window_state.navigation_history.push(path.clone());
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
                components::sidebar::SidebarEvent::JumpToPath(path) => {
                    // Jump to the specified path in the current file
                    // Track in navigation history before navigating
                    self.window_state.navigation_history.push(path.clone());
                    self.window_state.central_panel.navigate_to_path(path);
                }
                components::sidebar::SidebarEvent::DataSourceQueryResult { .. } => {
                    // No-op: data-source plugins now interact entirely through the main pane
                    // via ActivePluginPane. This event is no longer used in the primary path.
                }
                components::sidebar::SidebarEvent::DataSourceConsentNeeded(consent_request) => {
                    // TODO: Show a consent dialog asking the user to approve/deny the domain
                    // For now, just log it
                    eprintln!(
                        "Data source plugin {} requests consent for domain: {}",
                        consent_request.plugin_id, consent_request.domain
                    );
                }
                components::sidebar::SidebarEvent::DataSourceError(err) => {
                    // Display the error in the error modal
                    self.window_state.error = Some(crate::error::ThothError::Unknown {
                        message: format!("Data source error: {}", err),
                    });
                }
                components::sidebar::SidebarEvent::DataSourceLoading(is_loading) => {
                    if is_loading {
                        ui.spinner();
                    }
                }
                components::sidebar::SidebarEvent::PluginSidebarEvent(evt) => {
                    self.dispatch_plugin_event(evt);
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
