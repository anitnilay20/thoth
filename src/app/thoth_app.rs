use eframe::{App, Frame, egui};
use std::path::PathBuf;

use crate::{
    NOTIFICATION_MANAGER, PLUGIN_MANAGER,
    app::{file_picker, pick_file, tab_manager::TabEvent},
    components::{self, traits::ContextComponent},
    settings, state,
};

use super::{
    ShortcutAction, persistent_state::PersistentState, search_handler::SearchHandler,
    shortcut_handler::ShortcutHandler, update_handler::UpdateHandler,
};

pub struct ThothApp {
    pub settings: settings::Settings,
    pub persistent_state: PersistentState,
    pub window_state: state::WindowState,
    pub update_state: state::ApplicationUpdateState,
    settings_dialog: components::settings_dialog::SettingsDialog,
    clipboard_text: Option<String>,
    settings_changed: bool,
}

impl ThothApp {
    pub fn new(settings: settings::Settings, file_to_open: Option<PathBuf>) -> Self {
        let persistent_state = PersistentState::default();

        let mut window_state = state::WindowState::default();
        if settings.ui.remember_sidebar_state {
            window_state.sidebar_expanded = persistent_state.get_sidebar_expanded();
        }

        // Replace the default TabManager with one that uses the configured nav history size.
        window_state.tab_manager =
            crate::app::TabManager::new(settings.performance.navigation_history_size);

        if let Some(path) = file_to_open {
            window_state
                .tab_manager
                .open_file(path, settings.performance.navigation_history_size);
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

    pub fn create_new_window(&mut self) {
        use std::process::Command;
        if let Ok(exe_path) = std::env::current_exe() {
            match Command::new(exe_path).spawn() {
                Ok(_) => {}
                Err(e) => eprintln!("Failed to spawn new window: {}", e),
            }
        }
    }

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

        if let Some(tab) = self.window_state.tab_manager.active_tab_mut()
            && let Some(pane) = tab.active_plugin_pane.as_mut() {
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
                .with_toast(true),
            );
        }
    }

    fn open_settings_window(&mut self, ctx: &egui::Context) {
        self.settings_dialog.open(&self.settings);
        ctx.request_repaint();
    }
}

impl App for ThothApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        if UpdateHandler::should_check_updates(&self.update_state, &self.settings) {
            UpdateHandler::check_for_updates(&mut self.update_state);
        }

        let should_show_updates =
            UpdateHandler::handle_update_messages(&mut self.update_state, ctx);

        if should_show_updates && !self.settings_dialog.open {
            self.settings_dialog.open_updates(&self.settings);
        }

        if let Some(nm) = NOTIFICATION_MANAGER.get()
            && let Ok(mut nm) = nm.lock() {
                nm.show_notifications(ctx);
            }

        // Handle OS-dispatched file opens (e.g. macOS Apple Events / Finder)
        self.poll_os_open_requests();

        // Handle file drops
        self.handle_file_drop(ctx);
        self.update_window_title(ctx);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut Frame) {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        let ctx = ui.ctx().clone();

        settings::Settings::store(&ctx, &self.settings);

        self.poll_plugin_http_results(&ctx);

        if self.settings.ui.show_toolbar {
            self.render_toolbar(ui);
        }

        if self.settings.ui.show_status_bar {
            self.render_status_bar(ui);
        }

        if let Some(text) = self.clipboard_text.take() {
            ctx.copy_text(text);
        }

        let sidebar_msg = self.render_sidebar(ui);

        // Handle search messages from sidebar against the active tab.
        let (msg_to_central, search_error) = if let Some(tab) =
            self.window_state.tab_manager.active_tab_mut()
        {
            SearchHandler::handle_search_messages(
                sidebar_msg,
                &mut tab.search_engine_state,
                &tab.file_path,
                &tab.file_type,
                &ctx,
            )
        } else {
            (None, None)
        };

        if let Some(error) = search_error
            && let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                tab.error = Some(error);
            }

        let shortcut_actions =
            ShortcutHandler::handle_shortcuts(ui.ctx(), &self.settings.shortcuts);
        self.handle_shortcut_actions(ui.ctx(), shortcut_actions);

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

        if !self.settings_dialog.open {
            crate::theme::apply_theme(&ctx, &self.settings);
        }

        if let Some(new_settings) = settings_output.new_settings {
            self.apply_new_settings(new_settings);
        }

        for event in settings_output.events {
            match event {
                SettingsDialogEvent::CheckForUpdates => {
                    UpdateHandler::check_for_updates(&mut self.update_state);
                }
                SettingsDialogEvent::DownloadUpdate => {
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
                            if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                                tab.error = Some(e);
                            }
                        }
                    }
                }
                SettingsDialogEvent::UnregisterFromPath => {
                    match crate::platform::path_registry::unregister_from_path() {
                        Ok(()) => {
                            ctx.request_repaint();
                        }
                        Err(e) => {
                            if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                                tab.error = Some(e);
                            }
                        }
                    }
                }
            }
        }

        if self.window_state.sidebar_selected_section
            == Some(components::sidebar::SidebarSection::MarketPlace)
        {
            use crate::components::marketplace::{MarketplaceDetail, MarketplaceDetailProps};
            use crate::components::traits::StatelessComponent;
            MarketplaceDetail::render(ui, MarketplaceDetailProps);
        } else {
            self.render_central_panel(ui, msg_to_central);
        }

        self.render_error_modal(&ctx);

        if let Some(new_settings) = settings::Settings::take_if_dirty(&ctx) {
            self.apply_new_settings(new_settings);
        }
        self.save_settings_if_changed();

        #[cfg(feature = "profiling")]
        if self.settings.dev.show_profiler {
            puffin::GlobalProfiler::lock().new_frame();

            egui::Window::new(format!(
                "{} Profiler",
                egui_phosphor::regular::MAGNIFYING_GLASS
            ))
            .default_open(true)
            .show(&ctx, |ui| {
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

                ui.collapsing("Frame Stats", |ui| {
                    ctx.inspection_ui(ui);
                });

                ui.separator();

                ui.collapsing("Advanced Settings", |ui| {
                    ctx.settings_ui(ui);
                });
            });
        }
    }
}

impl ThothApp {
    fn handle_shortcut_actions(&mut self, ctx: &egui::Context, actions: Vec<ShortcutAction>) {
        let nav_capacity = self.settings.performance.navigation_history_size;

        for action in actions {
            match action {
                ShortcutAction::OpenFile => {
                    if let Some(path) = file_picker::pick_file(self.settings.plugins.enabled) {
                        if let Some(path_str) = path.to_str() {
                            self.persistent_state.add_recent_file(
                                path_str.to_string(),
                                self.settings.performance.max_recent_files,
                            );
                            let _ = self.persistent_state.save();
                        }
                        self.window_state.tab_manager.open_file(path, nav_capacity);
                    }
                }
                ShortcutAction::NewWindow => {
                    self.create_new_window();
                }
                ShortcutAction::Settings => {
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
                ShortcutAction::FocusSearch => {
                    let section = components::sidebar::SidebarSection::Search;
                    if self.window_state.sidebar_expanded
                        && self.window_state.sidebar_selected_section == Some(section.clone())
                    {
                        self.window_state.sidebar_expanded = false;
                    } else {
                        self.window_state.sidebar_expanded = true;
                        self.window_state.sidebar_selected_section = Some(section);
                    }

                    if self.settings.ui.remember_sidebar_state {
                        self.persistent_state
                            .set_sidebar_expanded(self.window_state.sidebar_expanded);
                        let _ = self.persistent_state.save();
                    }
                }
                ShortcutAction::NextMatch => {}
                ShortcutAction::PrevMatch => {}
                ShortcutAction::NavBack => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut()
                        && let Some(path) = tab.navigation_history.back() {
                            tab.central_panel.navigate_to_path(path);
                        }
                }
                ShortcutAction::NavForward => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut()
                        && let Some(path) = tab.navigation_history.forward() {
                            tab.central_panel.navigate_to_path(path);
                        }
                }
                ShortcutAction::Escape => {
                    if self.window_state.sidebar_expanded {
                        self.window_state.sidebar_expanded = false;

                        if self.settings.ui.remember_sidebar_state {
                            self.persistent_state.set_sidebar_expanded(false);
                            let _ = self.persistent_state.save();
                        }
                    }
                }
                ShortcutAction::ToggleBookmark => {
                    let info = self.window_state.tab_manager.active_tab_mut().and_then(|tab| {
                        let path = tab.central_panel.get_selected_path()?.clone();
                        let file_path = tab.file_path.as_ref()?.to_str()?.to_string();
                        Some((path, file_path))
                    });
                    if let Some((selected_path, file_path_str)) = info {
                        self.persistent_state
                            .toggle_bookmark(selected_path, file_path_str);
                        if let Err(e) = self.persistent_state.save() {
                            eprintln!("Failed to save bookmarks: {}", e);
                        }
                    }
                }
                ShortcutAction::OpenBookmarks => {
                    self.window_state.sidebar_expanded = true;
                    self.window_state.sidebar_selected_section =
                        Some(components::sidebar::SidebarSection::Bookmarks);

                    if self.settings.ui.remember_sidebar_state {
                        self.persistent_state.set_sidebar_expanded(true);
                        let _ = self.persistent_state.save();
                    }
                }
                ShortcutAction::ExpandNode => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                        tab.central_panel.expand_selected_node();
                    }
                }
                ShortcutAction::CollapseNode => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                        tab.central_panel.collapse_selected_node();
                    }
                }
                ShortcutAction::ExpandAll => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                        tab.central_panel.expand_all_nodes();
                    }
                }
                ShortcutAction::CollapseAll => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                        tab.central_panel.collapse_all_nodes();
                    }
                }
                ShortcutAction::MoveUp => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                        tab.central_panel.move_selection_up();
                    }
                }
                ShortcutAction::MoveDown => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                        tab.central_panel.move_selection_down();
                    }
                }
                ShortcutAction::CopyKey => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut()
                        && let Some(text) = tab.central_panel.copy_selected_key() {
                            self.clipboard_text = Some(text);
                        }
                }
                ShortcutAction::CopyValue => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut()
                        && let Some(text) = tab.central_panel.copy_selected_value() {
                            self.clipboard_text = Some(text);
                        }
                }
                ShortcutAction::CopyObject => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut()
                        && let Some(text) = tab.central_panel.copy_selected_object() {
                            self.clipboard_text = Some(text);
                        }
                }
                ShortcutAction::CopyPath => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut()
                        && let Some(text) = tab.central_panel.copy_selected_path() {
                            self.clipboard_text = Some(text);
                        }
                }
                ShortcutAction::CloseTab => {
                    let was_empty = self.window_state.tab_manager.close_active_tab();
                    let now_empty = self.window_state.tab_manager.tabs.is_empty();
                    if was_empty && now_empty {
                        // Last tab was already the welcome screen — close the window.
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    } else {
                        self.window_state.tab_manager.ensure_non_empty(nav_capacity);
                    }
                }
                ShortcutAction::NewTab => {
                    self.window_state.tab_manager.open_new_tab(nav_capacity);
                }
                ShortcutAction::NextTab => {
                    self.window_state.tab_manager.cycle_tab(1);
                }
                ShortcutAction::PrevTab => {
                    self.window_state.tab_manager.cycle_tab(-1);
                }
                ShortcutAction::SwitchToTab(idx) => {
                    self.window_state.tab_manager.switch_to_tab_by_index(idx);
                }
            }
        }
    }

    fn update_window_title(&mut self, ctx: &egui::Context) {
        // active_tab_id() borrows mutably; store result before the immutable tabs lookup.
        let active_id = self.window_state.tab_manager.active_tab_id();
        let title = active_id
            .and_then(|id| self.window_state.tab_manager.tabs.get(&id))
            .and_then(|tab| tab.file_path.as_deref())
            .and_then(|p| p.file_name())
            .and_then(|n| n.to_str())
            .map(|name| format!("Thoth — {}", name))
            .unwrap_or_else(|| "Thoth — JSON & NDJSON Viewer".to_owned());
        ctx.send_viewport_cmd(egui::ViewportCommand::Title(title));
    }

    fn dispatch_plugin_event(&mut self, event: crate::plugin::render_node::UiEvent) {
        if let Some(tab) = self.window_state.tab_manager.active_tab_mut()
            && let Some(pane) = tab.active_plugin_pane.as_mut() {
                match pane.loader.handle_event(event) {
                    Ok(new_output) => {
                        pane.ui_output = new_output;
                        if let Ok(sidebar) = pane.loader.render_sidebar() {
                            tab.plugin_sidebar_output = sidebar;
                        }
                    }
                    Err(e) => {
                        tab.error = Some(crate::error::ThothError::Unknown {
                            message: e.to_string(),
                        });
                    }
                }
            }
    }

    /// Drain OS-dispatched file open requests (e.g. macOS Apple Events) and
    /// load the most recent one. Called once per frame from `update()`.
    ///
    /// This mirrors the existing `poll_plugin_http_results` pattern: a
    /// platform-specific handler enqueues paths from a callback thread, and
    /// we drain them on the UI thread each frame.
    pub fn poll_os_open_requests(&mut self) {
        let paths = crate::platform::drain_open_requests();
        if let Some(path) = paths.into_iter().last() {
            // Add to recent files (same as toolbar / sidebar open-file paths)
            if let Some(path_str) = path.to_str() {
                self.persistent_state.add_recent_file(
                    path_str.to_string(),
                    self.settings.performance.max_recent_files,
                );
                let _ = self.persistent_state.save();
            }

            let nav_capacity = self.settings.performance.navigation_history_size;
            self.window_state.tab_manager.open_file(path, nav_capacity);
            if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                tab.error = None;
            }
        }
    }

    /// Drain completed async HTTP requests from the active plugin pane and
    /// forward each result to the plugin via `handle_event`.  Must be called
    /// before any rendering so the updated `ui_output` is used in this frame.
    fn poll_plugin_http_results(&mut self, ctx: &egui::Context) {
        use crate::plugin::render_node::UiEvent;

        let (http_events, retry_requests, needs_repaint) = {
            let Some(tab) = self.window_state.tab_manager.active_tab_mut() else {
                return;
            };
            let Some(pane) = tab.active_plugin_pane.as_mut() else {
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

        for (request_id, req) in retry_requests {
            if let Some(tab) = self.window_state.tab_manager.active_tab_mut()
                && let Some(pane) = tab.active_plugin_pane.as_mut() {
                    pane.loader.dispatch_approved_request(request_id, req);
                }
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

    fn render_toolbar(&mut self, ui: &mut egui::Ui) {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        let (file_type, file_path_opt, can_go_back, can_go_forward) =
            if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                let back = tab.navigation_history.can_go_back();
                let fwd = tab.navigation_history.can_go_forward();
                (tab.file_type, tab.file_path.clone(), back, fwd)
            } else {
                (
                    crate::file::lazy_loader::FileKind::default(),
                    None,
                    false,
                    false,
                )
            };

        let output = self.window_state.toolbar.render(
            ui,
            components::toolbar::ToolbarProps {
                file_type: &file_type,
                dark_mode: self.settings.dark_mode,
                shortcuts: &self.settings.shortcuts,
                file_path: file_path_opt.as_deref(),
                is_fullscreen: ui
                    .ctx()
                    .input(|i: &egui::InputState| i.viewport().fullscreen.unwrap_or(false)),
                can_go_back,
                can_go_forward,
                plugins_enabled: self.settings.plugins.enabled,
            },
        );

        let nav_capacity = self.settings.performance.navigation_history_size;

        for event in output.events {
            match event {
                components::toolbar::ToolbarEvent::FileOpen { path, file_type } => {
                    if let Some(path_str) = path.to_str() {
                        self.persistent_state.add_recent_file(
                            path_str.to_string(),
                            self.settings.performance.max_recent_files,
                        );
                        let _ = self.persistent_state.save();
                    }
                    let id = self.window_state.tab_manager.open_file(path, nav_capacity);
                    if let Some(tab) = self.window_state.tab_manager.tabs.get_mut(&id) {
                        tab.file_type = file_type;
                        tab.error = None;
                    }
                }
                components::toolbar::ToolbarEvent::FileClear => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                        tab.file_path = None;
                        tab.error = None;
                    }
                }
                components::toolbar::ToolbarEvent::NewWindow => {
                    self.create_new_window();
                }
                components::toolbar::ToolbarEvent::ToggleTheme => {
                    self.settings.dark_mode = !self.settings.dark_mode;
                    self.settings_changed = true;
                }
                components::toolbar::ToolbarEvent::OpenSettings => {
                    self.open_settings_window(ui.ctx());
                }
                components::toolbar::ToolbarEvent::NavigateBack => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut()
                        && let Some(path) = tab.navigation_history.back() {
                            tab.central_panel.navigate_to_path(path);
                        }
                }
                components::toolbar::ToolbarEvent::NavigateForward => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut()
                        && let Some(path) = tab.navigation_history.forward() {
                            tab.central_panel.navigate_to_path(path);
                        }
                }
            }
        }
    }

    fn save_settings_if_changed(&mut self) {
        if self.settings_changed {
            if let Err(e) = self.settings.save() {
                eprintln!("Failed to save settings: {}", e);
            }
            self.settings_changed = false;
        }
    }

    fn render_status_bar(&mut self, ui: &mut egui::Ui) {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        let (file_path_opt, file_type, total_items, error_present, search_scanning, _search_results_len, filtered_count, selected_path) =
            if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                let search = &tab.search_engine_state.search;
                let scanning = search.scanning;
                let results_len = search.results.len();
                let query_non_empty = !search.query.is_empty();
                let filtered = if query_non_empty && results_len > 0 {
                    Some(results_len)
                } else {
                    None
                };
                let sel_path = tab.central_panel.get_selected_path().cloned();
                (
                    tab.file_path.clone(),
                    tab.file_type,
                    tab.total_items,
                    tab.error.is_some(),
                    scanning,
                    results_len,
                    filtered,
                    sel_path,
                )
            } else {
                (
                    None,
                    crate::file::lazy_loader::FileKind::default(),
                    0,
                    false,
                    false,
                    0,
                    None,
                    None,
                )
            };

        let status = if search_scanning {
            components::status_bar::StatusBarStatus::Searching
        } else if filtered_count.is_some() {
            components::status_bar::StatusBarStatus::Filtered
        } else if error_present {
            components::status_bar::StatusBarStatus::Error
        } else {
            components::status_bar::StatusBarStatus::Ready
        };

        let status_bar_output = self.window_state.status_bar.render(
            ui,
            components::status_bar::StatusBarProps {
                file_path: file_path_opt.as_deref(),
                file_type: &file_type,
                item_count: total_items,
                filtered_count,
                status,
                selected_path: selected_path.as_deref(),
            },
        );

        for event in status_bar_output.events {
            match event {
                components::status_bar::StatusBarEvent::NavigateToPath(path) => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                        tab.navigation_history.push(path.clone());
                        tab.central_panel.navigate_to_path(path);
                    }
                }
            }
        }
    }

    /// Render the DockArea that hosts all open tabs.
    fn render_central_panel(
        &mut self,
        ui: &mut egui::Ui,
        search_message: Option<crate::search::SearchMessage>,
    ) {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        let nav_capacity = self.settings.performance.navigation_history_size;
        let focused_id = self.window_state.tab_manager.active_tab_id();

        let colors = ui
            .ctx()
            .memory(|m| m.data.get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors")));

        let dock_style = colors
            .map(|c| c.dock_style(ui.style()))
            .unwrap_or_else(|| egui_dock::Style::from_egui(ui.style()));

        let (dock_state, tabs) = self.window_state.tab_manager.borrow_parts();

        let mut viewer = crate::app::tab_manager::ThothTabViewer {
            tabs,
            settings: &self.settings,
            persistent_state: &mut self.persistent_state,
            nav_capacity,
            search_msg: search_message.zip(focused_id).map(|(msg, id)| (id, msg)),
            events: Vec::new(),
            colors,
        };

        // Use a smaller font for tab labels (egui_dock hardcodes TextStyle::Button).
        // Scoped so nothing outside the DockArea is affected.
        let events = ui
            .scope(|ui| {
                ui.style_mut().text_styles.insert(
                    egui::TextStyle::Button,
                    egui::FontId::new(12.0, egui::FontFamily::Proportional),
                );
                egui_dock::DockArea::new(dock_state)
                    .style(dock_style)
                    .show_leaf_collapse_buttons(false)
                    .show_inside(ui, &mut viewer);
                viewer.events.drain(..).collect::<Vec<_>>()
            })
            .inner;

        // Drain and process events emitted during rendering.
        let events: Vec<TabEvent> = events;
        for event in events {
            self.handle_tab_event(event, nav_capacity);
        }
    }

    fn handle_tab_event(&mut self, event: TabEvent, nav_capacity: usize) {
        match event {
            TabEvent::FileOpened {
                tab_id,
                path,
                file_type,
                total_items,
            } => {
                if let Some(path_str) = path.to_str() {
                    self.persistent_state.add_recent_file(
                        path_str.to_string(),
                        self.settings.performance.max_recent_files,
                    );
                    let _ = self.persistent_state.save();
                }
                if let Some(tab) = self.window_state.tab_manager.tabs.get_mut(&tab_id) {
                    tab.file_path = Some(path);
                    tab.file_type = file_type;
                    tab.total_items = total_items;
                    tab.active_plugin_pane = None;
                    tab.plugin_sidebar_output = None;
                    if let Some(pending_path) = tab.pending_navigation.take() {
                        tab.central_panel.navigate_to_path(pending_path);
                    }
                }
            }
            TabEvent::FileOpenError { tab_id, error } => {
                if let Some(tab) = self.window_state.tab_manager.tabs.get_mut(&tab_id) {
                    tab.error = Some(error);
                }
            }
            TabEvent::FileClosed { tab_id } => {
                if let Some(tab) = self.window_state.tab_manager.tabs.get_mut(&tab_id) {
                    tab.file_path = None;
                    tab.total_items = 0;
                }
            }
            TabEvent::FileTypeChanged { tab_id, file_type } => {
                if let Some(tab) = self.window_state.tab_manager.tabs.get_mut(&tab_id) {
                    tab.file_type = file_type;
                }
            }
            TabEvent::ErrorCleared { tab_id } => {
                if let Some(tab) = self.window_state.tab_manager.tabs.get_mut(&tab_id) {
                    tab.error = None;
                }
            }
            TabEvent::PluginUiEvent { event, .. } => {
                self.dispatch_plugin_event(event);
            }
            TabEvent::NavigationPush { tab_id, path } => {
                if let Some(tab) = self.window_state.tab_manager.tabs.get_mut(&tab_id) {
                    tab.navigation_history.push(path);
                }
            }
            TabEvent::TabClosed(id) => {
                self.window_state.tab_manager.ensure_non_empty(nav_capacity);
                let _ = id;
            }
            TabEvent::OpenFilePicker => {
                let nav_cap = self.settings.performance.navigation_history_size;
                if let Some(path) = pick_file(self.settings.plugins.enabled) {
                    self.window_state.tab_manager.open_file(path, nav_cap);
                }
            }
            TabEvent::OpenRecentFile(path) => {
                self.window_state
                    .tab_manager
                    .open_file(path, nav_capacity);
            }
        }
    }

    fn render_sidebar(&mut self, ui: &mut egui::Ui) -> Option<crate::search::SearchMessage> {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        use crate::components::traits::ContextComponent;

        let section_changed_to_search = self.window_state.sidebar_selected_section
            == Some(components::sidebar::SidebarSection::Search)
            && self.window_state.previous_sidebar_section
                != Some(components::sidebar::SidebarSection::Search);

        let sidebar_reopened_with_search = self.window_state.sidebar_expanded
            && !self.window_state.previous_sidebar_expanded
            && self.window_state.sidebar_selected_section
                == Some(components::sidebar::SidebarSection::Search);

        let focus_search = section_changed_to_search || sidebar_reopened_with_search;

        let ds_plugins: Vec<&crate::plugin::Plugin> = PLUGIN_MANAGER
            .get()
            .and_then(|m| m.as_ref())
            .map(|m| m.get_data_source_plugins())
            .unwrap_or_default();

        // Snapshot per-tab data we need for SidebarProps (avoids complex lifetime issues).
        let (current_file_path, search_state_clone, active_datasource_plugin_id) =
            if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                (
                    tab.file_path.clone(),
                    tab.search_engine_state.search.clone(),
                    tab.active_plugin_pane
                        .as_ref()
                        .map(|p| p.plugin_id.clone()),
                )
            } else {
                (None, crate::search::Search::default(), None)
            };

        let plugin_sidebar_strings: Option<(String, String, Option<String>)> = active_datasource_plugin_id
            .as_ref()
            .and_then(|plugin_id| {
                PLUGIN_MANAGER
                    .get()
                    .and_then(|m| m.as_ref())
                    .and_then(|m| m.registry.get_by_id(plugin_id))
                    .map(|p| (p.id.clone(), p.name.clone(), p.icon.clone()))
            });

        let plugin_sidebar_output = self
            .window_state
            .tab_manager
            .active_tab_mut()
            .and_then(|tab| tab.plugin_sidebar_output.clone());

        let plugin_sidebar_prop: Option<components::sidebar::PluginSidebarInfo<'_>> =
            match (&plugin_sidebar_strings, &plugin_sidebar_output) {
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

        let search_history = current_file_path
            .as_ref()
            .and_then(|p| p.to_str())
            .and_then(|path_str| {
                super::persistent_state::PersistentState::load_search_history(path_str).ok()
            });

        let output = self.window_state.sidebar.render(
            ui,
            components::sidebar::SidebarProps {
                recent_files: self.persistent_state.get_recent_files(),
                bookmarks: self.persistent_state.get_bookmarks(),
                current_file_path: current_file_path.as_ref().and_then(|p| p.to_str()),
                expanded: self.window_state.sidebar_expanded,
                sidebar_width: self.persistent_state.get_sidebar_width(),
                selected_section: self.window_state.sidebar_selected_section.clone(),
                focus_search,
                search_state: &search_state_clone,
                search_history: search_history.as_ref(),
                data_source_plugins: &ds_plugins,
                active_datasource_plugin_id: active_datasource_plugin_id.as_deref(),
                plugin_sidebar: plugin_sidebar_prop,
            },
        );

        if focus_search {
            self.window_state.previous_sidebar_section =
                self.window_state.sidebar_selected_section.clone();
        }
        self.window_state.previous_sidebar_expanded = self.window_state.sidebar_expanded;

        let nav_capacity = self.settings.performance.navigation_history_size;

        for event in output.events {
            match event {
                components::sidebar::SidebarEvent::OpenFile(file_path) => {
                    let path = std::path::PathBuf::from(&file_path);
                    self.window_state.tab_manager.open_file(path, nav_capacity);
                }
                components::sidebar::SidebarEvent::RemoveRecentFile(file_path) => {
                    self.persistent_state.remove_recent_file(&file_path);
                    if let Err(e) = self.persistent_state.save() {
                        eprintln!("Failed to save recent files: {}", e);
                    }
                }
                components::sidebar::SidebarEvent::OpenFilePicker => {
                    if let Some(path) = pick_file(self.settings.plugins.enabled) {
                        if let Some(path_str) = path.to_str() {
                            self.persistent_state.add_recent_file(
                                path_str.to_string(),
                                self.settings.performance.max_recent_files,
                            );
                            let _ = self.persistent_state.save();
                        }
                        self.window_state.tab_manager.open_file(path, nav_capacity);
                    }
                }
                components::sidebar::SidebarEvent::SectionToggled(section) => {
                    if let components::sidebar::SidebarSection::DataSource { ref plugin_id } =
                        section
                    {
                        let same_plugin_active =
                            self.window_state.tab_manager.active_tab_mut().is_some_and(|tab| {
                                tab.active_plugin_pane
                                    .as_ref()
                                    .is_some_and(|p| &p.plugin_id == plugin_id)
                            });

                        if same_plugin_active {
                            if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                                tab.active_plugin_pane = None;
                                tab.plugin_sidebar_output = None;
                            }
                            self.window_state.sidebar_expanded = false;
                            self.window_state.sidebar_selected_section = None;
                        } else {
                            if let Some(manager) = PLUGIN_MANAGER.get().and_then(|m| m.as_ref())
                                && let Some(plugin) = manager.registry.get_by_id(plugin_id) {
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
                                        Ok(loader) => match loader.render_ui() {
                                            Ok(ui_output) => {
                                                let sidebar_output =
                                                    loader.render_sidebar().ok().flatten();
                                                let has_sidebar = sidebar_output.is_some();
                                                if let Some(tab) = self
                                                    .window_state
                                                    .tab_manager
                                                    .active_tab_mut()
                                                {
                                                    tab.file_path = None;
                                                    tab.plugin_sidebar_output = sidebar_output;
                                                    tab.active_plugin_pane =
                                                        Some(crate::state::ActivePluginPane {
                                                            plugin_id: plugin_id.clone(),
                                                            display_url: String::new(),
                                                            ui_output,
                                                            loader,
                                                        });
                                                }
                                                if has_sidebar {
                                                    self.window_state.sidebar_expanded = true;
                                                    self.window_state.sidebar_selected_section =
                                                        Some(components::sidebar::SidebarSection::PluginSidebar {
                                                            plugin_id: plugin_id.clone(),
                                                        });
                                                } else {
                                                    self.window_state.sidebar_expanded = false;
                                                    self.window_state.sidebar_selected_section =
                                                        None;
                                                }
                                            }
                                            Err(e) => {
                                                if let Some(tab) = self
                                                    .window_state
                                                    .tab_manager
                                                    .active_tab_mut()
                                                {
                                                    tab.error =
                                                        Some(crate::error::ThothError::Unknown {
                                                            message: format!(
                                                                "Plugin UI error: {e}"
                                                            ),
                                                        });
                                                }
                                            }
                                        },
                                        Err(e) => {
                                            if let Some(tab) = self
                                                .window_state
                                                .tab_manager
                                                .active_tab_mut()
                                            {
                                                tab.error = Some(
                                                    crate::error::ThothError::Unknown {
                                                        message: format!(
                                                            "Failed to load plugin: {e}"
                                                        ),
                                                    },
                                                );
                                            }
                                        }
                                    }
                                }
                        }
                    } else {
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
                            if !is_plugin_sidebar
                                && let Some(tab) =
                                    self.window_state.tab_manager.active_tab_mut()
                                {
                                    tab.active_plugin_pane = None;
                                    tab.plugin_sidebar_output = None;
                                }
                        }
                    }

                    if self.settings.ui.remember_sidebar_state {
                        self.persistent_state
                            .set_sidebar_expanded(self.window_state.sidebar_expanded);
                        let _ = self.persistent_state.save();
                    }
                }
                components::sidebar::SidebarEvent::WidthChanged(new_width) => {
                    self.persistent_state.set_sidebar_width(new_width);
                    let _ = self.persistent_state.save();
                }
                components::sidebar::SidebarEvent::Search(msg) => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut()
                        && let Some(file_path) = &tab.file_path
                            && let Some(path_str) = file_path.to_str()
                                && let Some(entry) = msg.history_entry() {
                                    let _ =
                                        super::persistent_state::PersistentState::add_search_query(
                                            path_str, entry,
                                        );
                                }
                    return Some(msg);
                }
                components::sidebar::SidebarEvent::NavigateToSearchResult { record_index } => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                        tab.central_panel.navigate_to_record(record_index);
                    }
                }
                components::sidebar::SidebarEvent::ClearSearchHistory => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut()
                        && let Some(file_path) = &tab.file_path
                            && let Some(path_str) = file_path.to_str() {
                                let _ =
                                    super::persistent_state::PersistentState::clear_search_history(
                                        path_str,
                                    );
                            }
                }
                components::sidebar::SidebarEvent::NavigateToBookmark { file_path, path } => {
                    let current_file = self
                        .window_state
                        .tab_manager
                        .active_tab_mut()
                        .and_then(|tab| tab.file_path.as_ref().and_then(|p| p.to_str()).map(|s| s.to_string()));

                    if current_file.as_deref() != Some(file_path.as_str()) {
                        let path_buf = std::path::PathBuf::from(&file_path);
                        let id = self.window_state.tab_manager.open_file(path_buf, nav_capacity);
                        if let Some(tab) = self.window_state.tab_manager.tabs.get_mut(&id) {
                            tab.error = None;
                            tab.pending_navigation = Some(path.clone());
                        }
                        self.persistent_state.add_recent_file(
                            file_path.clone(),
                            self.settings.performance.max_recent_files,
                        );
                        let _ = self.persistent_state.save();
                    } else {
                        if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                            tab.navigation_history.push(path.clone());
                            tab.central_panel.navigate_to_path(path);
                        }
                    }
                }
                components::sidebar::SidebarEvent::RemoveBookmark(index) => {
                    self.persistent_state.remove_bookmark(index);
                    if let Err(e) = self.persistent_state.save() {
                        eprintln!("Failed to save bookmarks: {}", e);
                    }
                }
                components::sidebar::SidebarEvent::JumpToPath(path) => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                        tab.navigation_history.push(path.clone());
                        tab.central_panel.navigate_to_path(path);
                    }
                }
                components::sidebar::SidebarEvent::DataSourceQueryResult { .. } => {}
                components::sidebar::SidebarEvent::DataSourceConsentNeeded(consent_request) => {
                    eprintln!(
                        "Data source plugin {} requests consent for domain: {}",
                        consent_request.plugin_id, consent_request.domain
                    );
                }
                components::sidebar::SidebarEvent::DataSourceError(err) => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                        tab.error = Some(crate::error::ThothError::Unknown {
                            message: format!("Data source error: {}", err),
                        });
                    }
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

        let error = self
            .window_state
            .tab_manager
            .active_tab_mut()
            .and_then(|t| t.error.as_ref())
            .cloned();

        if let Some(error) = error {
            let mut output = None;
            egui::Area::new("error_modal_area".into())
                .movable(false)
                .interactable(false)
                .show(ctx, |ui| {
                    output = Some(self.window_state.error_modal.render(
                        ui,
                        components::error_modal::ErrorModalProps { error: &error, open: true },
                    ));
                });

            let Some(output) = output else { return };

            for event in output.events {
                match event {
                    components::error_modal::ErrorModalEvent::Close => {
                        if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                            tab.error = None;
                        }
                    }
                    components::error_modal::ErrorModalEvent::Retry => {
                        if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                            let path = tab.file_path.take();
                            tab.error = None;
                            tab.file_path = path;
                        }
                    }
                    components::error_modal::ErrorModalEvent::Reset => {
                        if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                            tab.error = None;
                            tab.file_path = None;
                            tab.total_items = 0;
                            tab.search_engine_state.search = crate::search::Search::default();
                        }
                    }
                }
            }

            if let Some(recovery_action) = output.recovery_action {
                match recovery_action {
                    RecoveryAction::ClearError => {
                        if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                            tab.error = None;
                        }
                    }
                    RecoveryAction::Reset => {
                        if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                            tab.error = None;
                            tab.file_path = None;
                            tab.total_items = 0;
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
