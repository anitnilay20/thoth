use eframe::{App, Frame, egui};
use std::path::PathBuf;

use crate::{
    NOTIFICATION_MANAGER, PLUGIN_MANAGER,
    app::{file_picker, pick_file, tab_manager::TabEvent},
    components::{self, traits::ContextComponent},
    plugin::plugin_ui_host::PluginUiHost,
    settings, state,
    theme::ThemeColorsExt,
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
    session_dirty: bool,
    show_update_consent: bool,
    /// Holds the live native menu bar (muda) so it isn't dropped.
    _native_menu: Option<crate::platform::native_menu::NativeMenu>,
    /// Plugin IDs from the persisted session that couldn't be restored yet because
    /// PLUGIN_MANAGER was still initializing on the background thread. Drained each
    /// frame once the manager becomes available.
    pending_plugin_restores: Vec<(String, Option<String>)>,
    /// Active-tab index to switch to once all deferred session tabs are open.
    /// `None` once the switch has been applied (or when no session is being restored).
    session_restore_active_index: Option<usize>,
    /// The plugin tab that was active last frame. Used to fire on-tab-focused /
    /// on-tab-blurred lifecycle callbacks when the active tab changes.
    last_active_plugin_tab: Option<crate::app::tab_manager::TabId>,
    /// The plugin whose sidebar is currently mounted (independent of any tab).
    sidebar_plugin: Option<SidebarPluginRuntime>,
    /// Built-in chart view — consumes the dataset bus (#113/#133).
    chart: components::chart_window::ChartWindow,
}

/// Build the synthetic `http-response` UiEvent delivered to a plugin when an
/// async `submit()` request completes (or fails / awaits consent).
fn build_http_response_event(
    request_id: String,
    outcome: crate::plugin::plugin_ui_host::HttpCallResult,
) -> crate::plugin::render_node::UiEvent {
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
    crate::plugin::render_node::UiEvent {
        widget_id: request_id,
        kind: "http-response".to_string(),
        value,
    }
}

/// Build the synthetic `query-result` UiEvent delivered to a plugin when an async
/// `db-runtime::submit-query` completes. `value` is JSON `{"ok": <rows-json>}` or
/// `{"err": {"message": "..."}}`.
fn build_query_result_event(
    request_id: String,
    result: std::result::Result<String, String>,
) -> crate::plugin::render_node::UiEvent {
    let value = match result {
        Ok(rows) => serde_json::json!({ "ok": rows }).to_string(),
        Err(msg) => serde_json::json!({ "err": { "message": msg } }).to_string(),
    };
    crate::plugin::render_node::UiEvent {
        widget_id: request_id,
        kind: "query-result".to_string(),
        value,
    }
}

/// Build the plugin UiEvent for a WebSocket lifecycle/message event. The
/// `widget_id` is the connection id; `kind`/`value` mirror the `websocket` WIT
/// contract (ws-open / ws-message / ws-error / ws-close).
fn build_ws_event(
    conn_id: String,
    event: crate::plugin::websocket::WsEvent,
) -> crate::plugin::render_node::UiEvent {
    use crate::plugin::websocket::WsEvent;
    let (kind, value) = match event {
        WsEvent::Open => ("ws-open", String::new()),
        WsEvent::Text(t) => ("ws-message", serde_json::json!({ "text": t }).to_string()),
        WsEvent::Binary(b) => {
            // Hex-encode binary frames for display (no extra dependency).
            let hex: String = b.iter().map(|byte| format!("{byte:02x}")).collect();
            (
                "ws-message",
                serde_json::json!({ "binary": hex, "len": b.len() }).to_string(),
            )
        }
        WsEvent::Error(m) => ("ws-error", m),
        WsEvent::Closed { code, reason } => (
            "ws-close",
            serde_json::json!({ "code": code, "reason": reason }).to_string(),
        ),
    };
    crate::plugin::render_node::UiEvent {
        widget_id: conn_id,
        kind: kind.to_string(),
        value,
    }
}

/// A plugin's sidebar instance, independent of any dock tab. Created when the user
/// opens the plugin's sidebar and kept alive while the sidebar is toggled, so its
/// state survives collapse/expand and never affects (or requires) a tab. Tabs are
/// spawned from it explicitly via the `ui-tabs` `open-tab` import.
struct SidebarPluginRuntime {
    plugin_id: String,
    loader: Box<dyn PluginUiHost>,
    output: Option<crate::plugin::render_node::UiOutput>,
}

impl ThothApp {
    pub fn new(settings: settings::Settings, file_to_open: Option<PathBuf>) -> Self {
        let persistent_state = PersistentState::default();

        let mut window_state = state::WindowState::default();
        if settings.ui.remember_sidebar_state {
            window_state.sidebar_expanded = persistent_state.get_sidebar_expanded();
        }

        // Replace the default TabManager with one that uses the configured nav history size.
        let nav_capacity = settings.performance.navigation_history_size;
        window_state.tab_manager = crate::app::TabManager::new(nav_capacity);

        let (pending_plugin_restores, session_restore_active_index) =
            if let Some(path) = file_to_open {
                // A file was passed via CLI / OS file association — open it directly,
                // skipping session restore so the user sees exactly what they asked for.
                window_state.tab_manager.open_file(path, nav_capacity);
                (Vec::new(), None)
            } else {
                // Restore the previous session (file tabs whose paths still exist, plugin tabs
                // that can be re-instantiated). Plugin tabs that can't be opened yet (because
                // PLUGIN_MANAGER is still initializing on a background thread) are returned
                // here and retried via poll_pending_plugin_restores() each frame.
                let (deferred, active_index) = Self::restore_tab_session(
                    &mut window_state.tab_manager,
                    &persistent_state,
                    &settings,
                );
                // Switch to the previously-active tab immediately if there are no deferred
                // plugins; otherwise defer until poll_pending_plugin_restores() finishes.
                let restore_index = if deferred.is_empty() {
                    window_state
                        .tab_manager
                        .switch_to_tab_by_index(active_index);
                    None
                } else {
                    Some(active_index)
                };
                (deferred, restore_index)
            };

        Self {
            settings,
            persistent_state,
            window_state,
            update_state: state::ApplicationUpdateState::default(),
            settings_dialog: components::settings_dialog::SettingsDialog::default(),
            clipboard_text: None,
            settings_changed: false,
            session_dirty: false,
            show_update_consent: false,
            _native_menu: None,
            pending_plugin_restores,
            session_restore_active_index,
            last_active_plugin_tab: None,
            sidebar_plugin: None,
            chart: components::chart_window::ChartWindow::default(),
        }
    }

    pub fn setup_native_menu(&mut self, cc: &eframe::CreationContext<'_>) {
        use raw_window_handle::HasWindowHandle as _;
        self._native_menu = cc
            .window_handle()
            .ok()
            .map(|h| h.as_raw())
            .and_then(|raw| crate::platform::native_menu::setup(raw, &self.settings.shortcuts));
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
            && let Some(pane) = tab.active_plugin_pane.as_mut()
        {
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

        // Publish the context once so off-thread workers (WebSocket tasks) can
        // request a repaint when data arrives.
        let _ = crate::EGUI_CTX.set(ctx.clone());

        if UpdateHandler::should_check_updates(&self.update_state, &self.settings) {
            UpdateHandler::check_for_updates(&mut self.update_state);
        }

        let should_show_updates =
            UpdateHandler::handle_update_messages(&mut self.update_state, ctx);

        if should_show_updates {
            self.show_update_consent = true;
            UpdateHandler::post_update_notification(&self.update_state);
        }

        if crate::OPEN_UPDATES_REQUESTED.swap(false, std::sync::atomic::Ordering::Relaxed)
            && !self.settings_dialog.open
        {
            self.settings_dialog.open_updates(&self.settings);
        }

        if let Some(nm) = NOTIFICATION_MANAGER.get()
            && let Ok(mut nm) = nm.lock()
        {
            nm.show_notifications(ctx);
        }

        // Restore plugin tabs that were deferred at startup (PLUGIN_MANAGER not ready yet).
        self.poll_pending_plugin_restores();

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

        self.poll_plugin_panes(&ctx);

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
        let (msg_to_central, search_error) =
            if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
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
            && let Some(tab) = self.window_state.tab_manager.active_tab_mut()
        {
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
        self.render_chart_window(&ctx);
        self.render_update_consent_modal(ui);

        if let Some(new_settings) = settings::Settings::take_if_dirty(&ctx) {
            self.apply_new_settings(new_settings);
        }
        self.save_settings_if_changed();
        self.save_session_if_dirty();

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
                        && let Some(path) = tab.navigation_history.back()
                    {
                        tab.central_panel.navigate_to_path(path);
                    }
                }
                ShortcutAction::NavForward => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut()
                        && let Some(path) = tab.navigation_history.forward()
                    {
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
                    let info = self
                        .window_state
                        .tab_manager
                        .active_tab_mut()
                        .and_then(|tab| {
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
                        && let Some(text) = tab.central_panel.copy_selected_key()
                    {
                        self.clipboard_text = Some(text);
                    }
                }
                ShortcutAction::CopyValue => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut()
                        && let Some(text) = tab.central_panel.copy_selected_value()
                    {
                        self.clipboard_text = Some(text);
                    }
                }
                ShortcutAction::CopyObject => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut()
                        && let Some(text) = tab.central_panel.copy_selected_object()
                    {
                        self.clipboard_text = Some(text);
                    }
                }
                ShortcutAction::CopyPath => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut()
                        && let Some(text) = tab.central_panel.copy_selected_path()
                    {
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
                    self.session_dirty = true;
                }
                ShortcutAction::NewTab => {
                    self.window_state.tab_manager.open_new_tab(nav_capacity);
                    // Empty tabs are not persisted, so no session_dirty needed here.
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

    /// Build an `ActivePluginPane`, snapshotting the plugin's tab title/icon so the
    /// dock label can be rendered cheaply without locking the WASM store each frame.
    fn make_plugin_pane(
        plugin_id: String,
        loader: Box<dyn PluginUiHost>,
        ui_output: crate::plugin::render_node::UiOutput,
    ) -> crate::state::ActivePluginPane {
        let cached_tab_title = loader.tab_title();
        let cached_tab_icon = loader.tab_icon();
        crate::state::ActivePluginPane {
            plugin_id,
            display_url: String::new(),
            ui_output,
            loader,
            cached_tab_title,
            cached_tab_icon,
        }
    }

    /// Forward a UI event to the plugin pane on a specific tab (used so async HTTP
    /// results land in the tab that originated them, not whichever tab is active).
    fn dispatch_plugin_event_for(
        &mut self,
        tab_id: crate::app::tab_manager::TabId,
        event: crate::plugin::render_node::UiEvent,
    ) {
        let mut handled = false;
        if let Some(tab) = self.window_state.tab_manager.tabs.get_mut(&tab_id)
            && let Some(pane) = tab.active_plugin_pane.as_mut()
        {
            match pane.loader.handle_event(event) {
                Ok(new_output) => {
                    pane.ui_output = new_output;
                    // Title/icon may change in response to the event.
                    pane.cached_tab_title = pane.loader.tab_title();
                    pane.cached_tab_icon = pane.loader.tab_icon();
                    if let Ok(sidebar) = pane.loader.render_sidebar() {
                        tab.plugin_sidebar_output = sidebar;
                    }
                    handled = true;
                }
                Err(e) => {
                    tab.error = Some(crate::error::ThothError::Unknown {
                        message: e.to_string(),
                    });
                }
            }
        }
        // A plugin interaction may have changed its persisted state (e.g. edited
        // SQL, switched database), so mark the session dirty. The actual write is
        // skipped by `save_session_if_dirty` when the serialized tabs are unchanged.
        if handled {
            self.session_dirty = true;
        }
    }

    /// Open a pure ui-component plugin in a tab (reusing an empty active tab or a
    /// new one), optionally seeding it with a saved state blob.
    /// Instantiate the right loader for a plugin based on its capabilities:
    /// data-source plugins (which import http-client) use the data-source loader;
    /// pure ui-component plugins use the ui-component loader.
    fn build_plugin_loader(
        manager: &crate::plugin::manager::PluginManager,
        plugin_id: &str,
        settings: &settings::Settings,
    ) -> Option<Box<dyn PluginUiHost>> {
        use crate::plugin::Capability;
        let plugin = manager.registry.get_by_id(plugin_id)?;
        if plugin.capabilities.contains(&Capability::DataSource) {
            use crate::plugin::network_policy::NetworkPolicy;
            let user_policy = settings
                .plugins
                .network_policies
                .get(plugin_id)
                .cloned()
                .unwrap_or_default();
            let policy = NetworkPolicy::from_plugin_and_settings(
                &plugin.network.clone().unwrap_or_default(),
                &user_policy,
            );
            manager
                .open_data_source(plugin_id, policy)
                .ok()
                .map(|l| Box::new(l) as Box<dyn PluginUiHost>)
        } else if plugin.capabilities.contains(&Capability::NewUIComponent) {
            manager
                .open_ui_component(plugin_id)
                .ok()
                .map(|l| Box::new(l) as Box<dyn PluginUiHost>)
        } else {
            None
        }
    }

    fn open_ui_component_tab(&mut self, plugin_id: &str, initial_state: Option<&str>) {
        let Some(manager) = PLUGIN_MANAGER.get().and_then(|m| m.as_ref()) else {
            return;
        };
        let Some(loader) = Self::build_plugin_loader(manager, plugin_id, &self.settings) else {
            if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                tab.error = Some(crate::error::ThothError::Unknown {
                    message: format!("Failed to load plugin '{plugin_id}'"),
                });
            }
            return;
        };
        if let Some(state) = initial_state {
            let _ = loader.init_with_state(state);
        }
        let Ok(ui_output) = loader.render_ui() else {
            return;
        };
        let sidebar_output = loader.render_sidebar().ok().flatten();
        let nav_capacity = self.settings.performance.navigation_history_size;
        let tab_id = if self
            .window_state
            .tab_manager
            .active_tab_mut()
            .is_some_and(|t| t.is_empty())
        {
            self.window_state.tab_manager.active_tab_id().unwrap()
        } else {
            self.window_state.tab_manager.open_new_tab(nav_capacity)
        };
        if let Some(t) = self.window_state.tab_manager.tabs.get_mut(&tab_id) {
            t.active_plugin_pane = Some(Self::make_plugin_pane(
                plugin_id.to_string(),
                loader,
                ui_output,
            ));
            t.plugin_sidebar_output = sidebar_output;
        }
        self.session_dirty = true;
    }

    /// Handle a plugin-initiated `open-tab` request: always open a fresh instance
    /// in a new tab, seeded with the requested initial state.
    fn open_ui_component_tab_from_request(
        &mut self,
        req: crate::plugin::plugin_ui_host::TabOpenRequest,
    ) {
        let Some(manager) = PLUGIN_MANAGER.get().and_then(|m| m.as_ref()) else {
            return;
        };
        let Some(loader) = Self::build_plugin_loader(manager, &req.plugin_id, &self.settings)
        else {
            return;
        };
        if let Some(state) = req.initial_state.as_deref() {
            let _ = loader.init_with_state(state);
        }
        let Ok(ui_output) = loader.render_ui() else {
            return;
        };
        let sidebar_output = loader.render_sidebar().ok().flatten();
        let nav_capacity = self.settings.performance.navigation_history_size;
        let tab_id = self.window_state.tab_manager.open_new_tab(nav_capacity);
        if let Some(t) = self.window_state.tab_manager.tabs.get_mut(&tab_id) {
            let mut pane = Self::make_plugin_pane(req.plugin_id.clone(), loader, ui_output);
            // Prefer the title/icon the plugin passed to open-tab if it didn't
            // override them via tab-title/tab-icon.
            if pane.cached_tab_title.is_none() && !req.title.is_empty() {
                pane.cached_tab_title = Some(req.title);
            }
            if pane.cached_tab_icon.is_none() {
                pane.cached_tab_icon = req.icon;
            }
            t.active_plugin_pane = Some(pane);
            t.plugin_sidebar_output = sidebar_output;
        }
        self.session_dirty = true;
    }

    /// Ensure a tab-independent sidebar runtime is mounted for `plugin_id`,
    /// reusing the existing one if it already hosts this plugin.
    fn ensure_sidebar_plugin(&mut self, plugin_id: &str) {
        if self
            .sidebar_plugin
            .as_ref()
            .is_some_and(|s| s.plugin_id == plugin_id)
        {
            return;
        }
        let Some(manager) = PLUGIN_MANAGER.get().and_then(|m| m.as_ref()) else {
            return;
        };
        let Some(loader) = Self::build_plugin_loader(manager, plugin_id, &self.settings) else {
            return;
        };
        let output = loader.render_sidebar().ok().flatten();
        self.sidebar_plugin = Some(SidebarPluginRuntime {
            plugin_id: plugin_id.to_string(),
            loader,
            output,
        });
    }

    /// Forward a widget event from the mounted plugin sidebar to its loader and
    /// refresh the cached sidebar output. (Tab-open requests it raises are drained
    /// in `poll_plugin_panes`.)
    fn dispatch_sidebar_event(&mut self, event: crate::plugin::render_node::UiEvent) {
        if let Some(rt) = self.sidebar_plugin.as_mut() {
            match rt.loader.handle_event(event) {
                Ok(_) => {
                    rt.output = rt.loader.render_sidebar().ok().flatten();
                }
                Err(e) => {
                    eprintln!("Plugin sidebar event error: {e}");
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

    /// Drive every open plugin tab once per frame, before rendering:
    ///  * deliver completed async HTTP results to the tab that originated them,
    ///  * re-dispatch consent-approved retries,
    ///  * open any tabs the plugins requested via `open-tab`,
    ///  * fire on-tab-focused / on-tab-blurred when the active tab changes.
    fn poll_plugin_panes(&mut self, ctx: &egui::Context) {
        use crate::app::tab_manager::TabId;
        use crate::plugin::plugin_ui_host::{PluginHttpRequest, TabOpenRequest};
        use crate::plugin::render_node::UiEvent;

        // Collect drained items into owned vecs BEFORE mutating the tab manager
        // (opening tabs / dispatching events borrows it mutably).
        let ids: Vec<TabId> = self.window_state.tab_manager.tabs.keys().copied().collect();
        let mut http_dispatch: Vec<(TabId, UiEvent)> = Vec::new();
        let mut query_dispatch: Vec<(TabId, UiEvent)> = Vec::new();
        let mut ws_dispatch: Vec<(TabId, UiEvent)> = Vec::new();
        let mut retry_dispatch: Vec<(TabId, String, PluginHttpRequest)> = Vec::new();
        let mut tab_open_reqs: Vec<TabOpenRequest> = Vec::new();
        let mut needs_repaint = false;

        for id in &ids {
            let Some(tab) = self.window_state.tab_manager.tabs.get(id) else {
                continue;
            };
            let Some(pane) = tab.active_plugin_pane.as_ref() else {
                continue;
            };
            // Spawn workers for newly-submitted queries (always — just enqueues).
            pane.loader.pump_queries();
            // Fold async results only when no query worker holds the Store mutex:
            // dispatching calls `handle_event`, which takes that mutex, so draining
            // while busy would block the UI thread. Results stay queued (and
            // `has_pending_*` keeps a repaint scheduled) until the worker frees it.
            if !pane.loader.busy() {
                for (request_id, outcome) in pane.loader.drain_http_results() {
                    http_dispatch.push((*id, build_http_response_event(request_id, outcome)));
                }
                for (request_id, result) in pane.loader.drain_query_results() {
                    query_dispatch.push((*id, build_query_result_event(request_id, result)));
                }
                for (conn_id, event) in pane.loader.drain_ws_events() {
                    ws_dispatch.push((*id, build_ws_event(conn_id, event)));
                }
            }
            for (request_id, req) in pane.loader.drain_retry_requests() {
                retry_dispatch.push((*id, request_id, req));
            }
            tab_open_reqs.extend(pane.loader.drain_tab_open_requests());
            if pane.loader.has_pending_http() || pane.loader.has_pending_query() {
                needs_repaint = true;
            }
        }

        // The mounted plugin sidebar runs independently of tabs — drive it too.
        let mut sidebar_http: Vec<UiEvent> = Vec::new();
        let mut sidebar_query: Vec<UiEvent> = Vec::new();
        let mut sidebar_ws: Vec<UiEvent> = Vec::new();
        let mut sidebar_retry: Vec<(String, PluginHttpRequest)> = Vec::new();
        if let Some(rt) = self.sidebar_plugin.as_ref() {
            rt.loader.pump_queries();
            // Same as the tab panes: defer folding async results while a query
            // worker owns the Store, so the sidebar never blocks the UI thread.
            if !rt.loader.busy() {
                for (request_id, outcome) in rt.loader.drain_http_results() {
                    sidebar_http.push(build_http_response_event(request_id, outcome));
                }
                for (request_id, result) in rt.loader.drain_query_results() {
                    sidebar_query.push(build_query_result_event(request_id, result));
                }
                for (conn_id, event) in rt.loader.drain_ws_events() {
                    sidebar_ws.push(build_ws_event(conn_id, event));
                }
            }
            // Consent-approved retries must be replayed here too, or a sidebar
            // plugin's submit()/query stalls after the user approves the host.
            for (request_id, req) in rt.loader.drain_retry_requests() {
                sidebar_retry.push((request_id, req));
            }
            tab_open_reqs.extend(rt.loader.drain_tab_open_requests());
            if rt.loader.has_pending_http() || rt.loader.has_pending_query() {
                needs_repaint = true;
            }
        }

        for (id, event) in http_dispatch {
            self.dispatch_plugin_event_for(id, event);
        }
        for (id, event) in query_dispatch {
            self.dispatch_plugin_event_for(id, event);
        }
        for (id, event) in ws_dispatch {
            self.dispatch_plugin_event_for(id, event);
        }
        for event in sidebar_http {
            self.dispatch_sidebar_event(event);
        }
        for event in sidebar_query {
            self.dispatch_sidebar_event(event);
        }
        for event in sidebar_ws {
            self.dispatch_sidebar_event(event);
        }

        for (request_id, req) in sidebar_retry {
            if let Some(rt) = self.sidebar_plugin.as_ref() {
                rt.loader.dispatch_approved_request(request_id, req);
            }
            self.dispatch_sidebar_event(UiEvent {
                widget_id: "consent-approved".to_string(),
                kind: "notify".to_string(),
                value: String::new(),
            });
            ctx.request_repaint();
        }

        for (id, request_id, req) in retry_dispatch {
            if let Some(tab) = self.window_state.tab_manager.tabs.get(&id)
                && let Some(pane) = tab.active_plugin_pane.as_ref()
            {
                pane.loader.dispatch_approved_request(request_id, req);
            }
            self.dispatch_plugin_event_for(
                id,
                UiEvent {
                    widget_id: "consent-approved".to_string(),
                    kind: "notify".to_string(),
                    value: String::new(),
                },
            );
            ctx.request_repaint();
        }

        for req in tab_open_reqs {
            self.open_ui_component_tab_from_request(req);
        }

        // Tab focus/blur lifecycle: notify plugins when the active tab changes.
        let active = self.window_state.tab_manager.active_tab_id();
        if active != self.last_active_plugin_tab {
            if let Some(prev) = self.last_active_plugin_tab
                && let Some(tab) = self.window_state.tab_manager.tabs.get(&prev)
                && let Some(pane) = tab.active_plugin_pane.as_ref()
            {
                pane.loader.on_tab_blurred();
            }
            if let Some(cur) = active
                && let Some(tab) = self.window_state.tab_manager.tabs.get(&cur)
                && let Some(pane) = tab.active_plugin_pane.as_ref()
            {
                pane.loader.on_tab_focused();
            }
            self.last_active_plugin_tab = active;
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
                components::toolbar::ToolbarEvent::CloseTab => {
                    let was_empty = self.window_state.tab_manager.close_active_tab();
                    let now_empty = self.window_state.tab_manager.tabs.is_empty();
                    if was_empty && now_empty {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    } else {
                        self.window_state.tab_manager.ensure_non_empty(nav_capacity);
                    }
                    self.session_dirty = true;
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
                        && let Some(path) = tab.navigation_history.back()
                    {
                        tab.central_panel.navigate_to_path(path);
                    }
                }
                components::toolbar::ToolbarEvent::NavigateForward => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut()
                        && let Some(path) = tab.navigation_history.forward()
                    {
                        tab.central_panel.navigate_to_path(path);
                    }
                }
            }
        }

        // Poll native menu bar events (macOS / Windows).
        // Linux falls back to the egui in-window menu bar rendered by toolbar.rs.
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        for action in crate::platform::native_menu::poll_events() {
            use crate::platform::native_menu::MenuAction;
            match action {
                MenuAction::OpenFile => {
                    let plugins_enabled = self.settings.plugins.enabled;
                    if let Some(path) = crate::app::pick_file(plugins_enabled)
                        && let Some(file_type) =
                            crate::components::toolbar::infer_file_type_pub(&path)
                    {
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
                }
                MenuAction::NewWindow => self.create_new_window(),
                MenuAction::CloseTab => {
                    let was_empty = self.window_state.tab_manager.close_active_tab();
                    let now_empty = self.window_state.tab_manager.tabs.is_empty();
                    if was_empty && now_empty {
                        ui.ctx().send_viewport_cmd(egui::ViewportCommand::Close);
                    } else {
                        self.window_state.tab_manager.ensure_non_empty(nav_capacity);
                    }
                    self.session_dirty = true;
                }
                MenuAction::OpenSettings => self.open_settings_window(ui.ctx()),
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

    /// Re-open tabs saved from the previous session.
    /// Returns `(deferred_plugin_ids, active_tab_index)`.
    /// - `deferred_plugin_ids`: plugin IDs that couldn't be opened yet because
    ///   PLUGIN_MANAGER was still initializing; retried via `poll_pending_plugin_restores()`.
    /// - `active_tab_index`: the dock-order index to switch to after all tabs are open.
    fn restore_tab_session(
        tab_manager: &mut crate::app::TabManager,
        persistent_state: &PersistentState,
        settings: &settings::Settings,
    ) -> (Vec<(String, Option<String>)>, usize) {
        use crate::app::persistent_state::PersistedTabKind;

        let nav_capacity = settings.performance.navigation_history_size;
        let persisted = persistent_state.get_open_tabs().to_vec();
        let active_tab_index = persistent_state.get_active_tab_index();
        let mut deferred_plugins = Vec::new();

        for tab in &persisted {
            match &tab.kind {
                PersistedTabKind::File { path } => {
                    let p = std::path::PathBuf::from(path);
                    if p.exists() {
                        tab_manager.open_file(p, nav_capacity);
                    }
                }
                PersistedTabKind::Plugin { plugin_id, state } => {
                    // PLUGIN_MANAGER is set by a background thread; it may not be ready yet.
                    match PLUGIN_MANAGER.get() {
                        Some(manager_opt) => {
                            if let Some(manager) = manager_opt.as_ref() {
                                Self::open_plugin_tab(
                                    tab_manager,
                                    manager,
                                    plugin_id,
                                    state.as_deref(),
                                    settings,
                                );
                            }
                            // manager_opt == None means plugins are disabled — skip silently.
                        }
                        None => {
                            // Manager not initialized yet — defer to poll loop.
                            deferred_plugins.push((plugin_id.clone(), state.clone()));
                        }
                    }
                }
            }
        }

        (deferred_plugins, active_tab_index)
    }

    /// Try to open a single plugin tab. Shared by initial restore and the deferred poll loop.
    /// Picks the loader by capability: data-source plugins (which need http-client) use the
    /// data-source loader; pure ui-component plugins use the ui-component loader. `state` is
    /// the persisted per-tab blob, replayed via `init-with-state` after instantiation.
    fn open_plugin_tab(
        tab_manager: &mut crate::app::TabManager,
        manager: &crate::plugin::manager::PluginManager,
        plugin_id: &str,
        state: Option<&str>,
        settings: &settings::Settings,
    ) {
        let nav_capacity = settings.performance.navigation_history_size;
        let Some(loader) = Self::build_plugin_loader(manager, plugin_id, settings) else {
            return;
        };

        if let Some(s) = state {
            let _ = loader.init_with_state(s);
        }
        let Ok(ui_output) = loader.render_ui() else {
            return;
        };
        let sidebar_output = loader.render_sidebar().ok().flatten();
        let tab_id = if tab_manager.active_tab_mut().is_some_and(|t| t.is_empty()) {
            tab_manager.active_tab_id().unwrap()
        } else {
            tab_manager.open_new_tab(nav_capacity)
        };
        if let Some(t) = tab_manager.tabs.get_mut(&tab_id) {
            t.active_plugin_pane = Some(Self::make_plugin_pane(
                plugin_id.to_string(),
                loader,
                ui_output,
            ));
            t.plugin_sidebar_output = sidebar_output;
        }
    }

    /// Called each frame from `update()`. Once PLUGIN_MANAGER is ready, drains
    /// `pending_plugin_restores` and opens each plugin tab, then applies the
    /// saved active-tab index.
    fn poll_pending_plugin_restores(&mut self) {
        if self.pending_plugin_restores.is_empty() {
            return;
        }
        // Wait until the background init thread has called PLUGIN_MANAGER.set().
        let Some(manager_opt) = PLUGIN_MANAGER.get() else {
            return;
        };
        let plugin_ids = std::mem::take(&mut self.pending_plugin_restores);
        if let Some(manager) = manager_opt.as_ref() {
            for (plugin_id, state) in &plugin_ids {
                Self::open_plugin_tab(
                    &mut self.window_state.tab_manager,
                    manager,
                    plugin_id,
                    state.as_deref(),
                    &self.settings,
                );
            }
            self.session_dirty = true;
        }
        // If manager is None (plugins disabled), plugin_ids is simply dropped.

        // All deferred tabs are now open — apply the saved active-tab index.
        if let Some(idx) = self.session_restore_active_index.take() {
            self.window_state.tab_manager.switch_to_tab_by_index(idx);
        }
    }

    /// Snapshot the current open tabs and write them to persistent_state, then save to disk.
    fn save_session_if_dirty(&mut self) {
        if !self.session_dirty {
            return;
        }

        use crate::app::persistent_state::{PersistedTab, PersistedTabKind};

        let ordered_ids = self.window_state.tab_manager.ordered_tab_ids();
        let active_id = self.window_state.tab_manager.active_tab_id();

        let mut active_tab_index: usize = 0;
        let mut persisted_index: usize = 0;

        let tabs: Vec<PersistedTab> = ordered_ids
            .into_iter()
            .filter_map(|id| {
                let tab = self.window_state.tab_manager.tabs.get(&id)?;
                let entry = tab
                    .file_path
                    .as_ref()
                    .map(|path| PersistedTab {
                        kind: PersistedTabKind::File {
                            path: path.to_string_lossy().into_owned(),
                        },
                    })
                    .or_else(|| {
                        tab.active_plugin_pane.as_ref().map(|pane| PersistedTab {
                            kind: PersistedTabKind::Plugin {
                                plugin_id: pane.plugin_id.clone(),
                                state: pane.loader.get_state().ok().flatten(),
                            },
                        })
                    });
                if entry.is_some() {
                    if Some(id) == active_id {
                        active_tab_index = persisted_index;
                    }
                    persisted_index += 1;
                }
                entry
            })
            .collect();

        // Nothing structural or state-wise changed since the last write (a plugin
        // event that didn't touch persisted state, e.g. opening a dropdown) — clear
        // the flag without touching disk so typing/interaction doesn't churn I/O.
        if tabs == self.persistent_state.get_open_tabs()
            && active_tab_index == self.persistent_state.get_active_tab_index()
        {
            self.session_dirty = false;
            return;
        }

        self.persistent_state.set_open_tabs(tabs, active_tab_index);
        if let Err(e) = self.persistent_state.save() {
            eprintln!("Failed to save tab session: {e}");
        } else {
            self.session_dirty = false;
        }
    }

    fn render_status_bar(&mut self, ui: &mut egui::Ui) {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        // Reconcile the process-global signal registry with the plugin instances
        // that still have a live pane, so a closed pane's signals stop showing.
        let open_instances: std::collections::HashSet<String> = self
            .window_state
            .tab_manager
            .tabs
            .iter()
            .filter_map(|(id, t)| match t.active_plugin_pane.as_ref() {
                // Plugin producer/consumer instances.
                Some(p) => Some(p.loader.instance_id().to_string()),
                // Core file tabs act as dataset producers under a stable marker.
                None if t.file_path.is_some() => Some(format!("core#{id}")),
                None => None,
            })
            .collect();
        crate::plugin::signals::retain_instances(&open_instances);
        // Datasets are cleared when their producing instance closes too.
        crate::plugin::datasets::retain_instances(&open_instances);

        let (
            file_path_opt,
            file_type,
            total_items,
            error_present,
            search_scanning,
            _search_results_len,
            filtered_count,
            selected_path,
            active_plugin_id,
        ) = if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
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
            // A plugin pane tab: (plugin_id, instance_id) drives the
            // instance-scoped status bar.
            let plugin_id = tab
                .active_plugin_pane
                .as_ref()
                .map(|p| (p.plugin_id.clone(), p.loader.instance_id().to_string()));
            (
                tab.file_path.clone(),
                tab.file_type,
                tab.total_items,
                tab.error.is_some(),
                scanning,
                results_len,
                filtered,
                sel_path,
                plugin_id,
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
                active_plugin: active_plugin_id
                    .as_ref()
                    .map(|(p, i)| (p.as_str(), i.as_str())),
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

        let colors = ui.ctx().memory(|m| {
            m.data
                .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
        });

        let mut dock_style = colors
            .map(|c| c.dock_style(ui.style()))
            .unwrap_or_else(|| egui_dock::Style::from_egui(ui.style()));
        // Hide egui_dock's thick (hardcoded 7.5px) tab-bar overflow scroll bar.
        // Overflowing tabs still scroll via wheel/trackpad while hovering the bar.
        dock_style.tab_bar.show_scroll_bar_on_overflow = false;

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
                self.session_dirty = true;
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
                self.session_dirty = true;
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
            TabEvent::PluginUiEvent { tab_id, event } => {
                self.dispatch_plugin_event_for(tab_id, event);
            }
            TabEvent::NavigationPush { tab_id, path } => {
                if let Some(tab) = self.window_state.tab_manager.tabs.get_mut(&tab_id) {
                    tab.navigation_history.push(path);
                }
            }
            TabEvent::TabClosed(id) => {
                self.window_state.tab_manager.ensure_non_empty(nav_capacity);
                let _ = id;
                self.session_dirty = true;
            }
            TabEvent::OpenFilePicker => {
                let nav_cap = self.settings.performance.navigation_history_size;
                if let Some(path) = pick_file(self.settings.plugins.enabled) {
                    self.window_state.tab_manager.open_file(path, nav_cap);
                }
            }
            TabEvent::OpenRecentFile(path) => {
                self.window_state.tab_manager.open_file(path, nav_capacity);
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

        let ui_plugins: Vec<&crate::plugin::Plugin> = PLUGIN_MANAGER
            .get()
            .and_then(|m| m.as_ref())
            .map(|m| m.get_ui_component_plugins())
            .unwrap_or_default();

        // Snapshot per-tab data we need for SidebarProps (avoids complex lifetime issues).
        let (current_file_path, search_state_clone) =
            if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                (
                    tab.file_path.clone(),
                    tab.search_engine_state.search.clone(),
                )
            } else {
                (None, crate::search::Search::default())
            };

        // The mounted plugin sidebar (independent of any tab) drives the sidebar
        // panel and the icon-highlight state.
        let sidebar_plugin_id: Option<String> =
            self.sidebar_plugin.as_ref().map(|s| s.plugin_id.clone());

        let plugin_sidebar_strings: Option<(String, String, Option<String>)> =
            sidebar_plugin_id.as_ref().and_then(|plugin_id| {
                PLUGIN_MANAGER
                    .get()
                    .and_then(|m| m.as_ref())
                    .and_then(|m| m.registry.get_by_id(plugin_id))
                    .map(|p| (p.id.clone(), p.name.clone(), p.icon.clone()))
            });

        let plugin_sidebar_output = self.sidebar_plugin.as_ref().and_then(|s| s.output.clone());

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
                ui_component_plugins: &ui_plugins,
                active_datasource_plugin_id: sidebar_plugin_id.as_deref(),
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
                        // Opening/closing a plugin's sidebar must NOT touch its tab.
                        // Mount a tab-independent sidebar runtime and toggle only the
                        // sidebar's visibility. Tabs are spawned from the sidebar's
                        // own "New tab" control (the ui-tabs open-tab import).
                        let already_showing = self.window_state.sidebar_expanded
                            && matches!(
                                &self.window_state.sidebar_selected_section,
                                Some(components::sidebar::SidebarSection::PluginSidebar { plugin_id: p })
                                    if p == plugin_id
                            );
                        if already_showing {
                            self.window_state.sidebar_expanded = false;
                            self.window_state.sidebar_selected_section = None;
                        } else {
                            self.ensure_sidebar_plugin(plugin_id);
                            if self.sidebar_plugin.is_some() {
                                self.window_state.sidebar_expanded = true;
                                self.window_state.sidebar_selected_section =
                                    Some(components::sidebar::SidebarSection::PluginSidebar {
                                        plugin_id: plugin_id.clone(),
                                    });
                            }
                        }
                    } else {
                        // Non-plugin sections: pure sidebar visibility toggle —
                        // never disturb open plugin tabs.
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
                        }
                    }

                    if self.settings.ui.remember_sidebar_state {
                        self.persistent_state
                            .set_sidebar_expanded(self.window_state.sidebar_expanded);
                        let _ = self.persistent_state.save();
                    }
                }
                components::sidebar::SidebarEvent::OpenUiComponentTab(plugin_id) => {
                    self.open_ui_component_tab(&plugin_id, None);
                }
                components::sidebar::SidebarEvent::WidthChanged(new_width) => {
                    self.persistent_state.set_sidebar_width(new_width);
                    let _ = self.persistent_state.save();
                }
                components::sidebar::SidebarEvent::Search(msg) => {
                    if let Some(tab) = self.window_state.tab_manager.active_tab_mut()
                        && let Some(file_path) = &tab.file_path
                        && let Some(path_str) = file_path.to_str()
                        && let Some(entry) = msg.history_entry()
                    {
                        let _ = super::persistent_state::PersistentState::add_search_query(
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
                        && let Some(path_str) = file_path.to_str()
                    {
                        let _ = super::persistent_state::PersistentState::clear_search_history(
                            path_str,
                        );
                    }
                }
                components::sidebar::SidebarEvent::NavigateToBookmark { file_path, path } => {
                    let current_file =
                        self.window_state
                            .tab_manager
                            .active_tab_mut()
                            .and_then(|tab| {
                                tab.file_path
                                    .as_ref()
                                    .and_then(|p| p.to_str())
                                    .map(|s| s.to_string())
                            });

                    if current_file.as_deref() != Some(file_path.as_str()) {
                        let path_buf = std::path::PathBuf::from(&file_path);
                        let id = self
                            .window_state
                            .tab_manager
                            .open_file(path_buf, nav_capacity);
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
                    self.dispatch_sidebar_event(evt);
                }
                components::sidebar::SidebarEvent::OpenSettings => {
                    self.settings_dialog.open(&self.settings);
                }
                components::sidebar::SidebarEvent::NewChart => {
                    let producers = self.gather_producers();
                    self.chart.open_picker(producers);
                }
            }
        }

        None
    }

    /// Open tabs eligible to provide a dataset for the picker: plugin panes
    /// whose manifest declares the `data-producer` capability (and whose loader
    /// supports the call), plus every open file tab (core JSON/NDJSON and
    /// file-loader plugins are producers by default).
    fn gather_producers(&self) -> Vec<crate::components::chart_window::ProducerRef> {
        use crate::components::chart_window::{ProducerKind, ProducerRef};
        self.window_state
            .tab_manager
            .tabs
            .iter()
            .filter_map(|(id, tab)| {
                // Plugin producer tabs: must declare the capability in their
                // manifest AND export a working provide-dataset.
                if let Some(pane) = tab.active_plugin_pane.as_ref() {
                    if !pane.loader.is_data_producer()
                        || !self.plugin_declares_producer(&pane.plugin_id)
                    {
                        return None;
                    }
                    let label = pane
                        .cached_tab_title
                        .clone()
                        .unwrap_or_else(|| pane.plugin_id.clone());
                    return Some(ProducerRef {
                        tab_id: *id,
                        label,
                        kind: ProducerKind::Plugin,
                    });
                }
                // Core producer: any open file tab. This includes files loaded
                // by a file-loader plugin (csv-loader, …), because the tab's
                // live loader exposes records uniformly.
                if let Some(path) = tab.file_path.as_ref() {
                    let label = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("file")
                        .to_string();
                    return Some(ProducerRef {
                        tab_id: *id,
                        label,
                        kind: ProducerKind::File,
                    });
                }
                None
            })
            .collect()
    }

    /// Whether the plugin with `plugin_id` declares the `data-producer`
    /// capability in its manifest.
    fn plugin_declares_producer(&self, plugin_id: &str) -> bool {
        matches!(PLUGIN_MANAGER.get(), Some(Some(pm))
            if pm
                .get_plugin_by_id(plugin_id)
                .is_some_and(|p| p.capabilities.contains(&crate::plugin::Capability::DataProducer)))
    }

    /// Route a dataset request to the chosen producer tab: call its
    /// `provide-dataset`, store the single copy in the registry, and bind the
    /// handle to the chart view.
    fn chart_fetch_from(&mut self, tab_id: crate::app::tab_manager::TabId) {
        let Some(tab) = self.window_state.tab_manager.tabs.get_mut(&tab_id) else {
            return;
        };

        // Core producer: an open file tab (host-native or loaded by a
        // file-loader plugin). Read records straight from the tab's live
        // loader so CSV and every other file-loader format works uniformly.
        if tab.active_plugin_pane.is_none() {
            let Some(path) = tab.file_path.clone() else {
                return;
            };
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("file")
                .to_string();
            match tab.central_panel.to_dataset() {
                Some((cols, rows)) => {
                    let columns: Vec<crate::plugin::datasets::DatasetColumn> = cols
                        .into_iter()
                        .map(|(name, type_hint)| crate::plugin::datasets::DatasetColumn {
                            name,
                            type_hint,
                        })
                        .collect();
                    let col_count = columns.len();
                    let handle = crate::plugin::datasets::publish(
                        "com.thoth.core",
                        &format!("core#{tab_id}"),
                        name.clone(),
                        "file".to_string(),
                        Vec::new(),
                        columns,
                        rows,
                    );
                    self.chart.set_dataset(handle, name, col_count);
                }
                None => {
                    crate::notification::NotificationManager::notify_error(
                        crate::notification::Notification::new(
                            "Dataset unavailable",
                            "Could not read this file as a table.",
                        ),
                    );
                }
            }
            return;
        }

        let Some(pane) = tab.active_plugin_pane.as_ref() else {
            return;
        };
        match pane.loader.provide_dataset() {
            Ok(ds) => {
                let cols: Vec<crate::plugin::datasets::DatasetColumn> = ds
                    .columns
                    .into_iter()
                    .map(|(name, type_hint)| crate::plugin::datasets::DatasetColumn {
                        name,
                        type_hint,
                    })
                    .collect();
                let col_count = cols.len();
                let handle = crate::plugin::datasets::publish(
                    &pane.plugin_id,
                    pane.loader.instance_id(),
                    ds.name.clone(),
                    ds.kind,
                    Vec::new(),
                    cols,
                    ds.rows,
                );
                self.chart.set_dataset(handle, ds.name, col_count);
            }
            Err(e) => {
                crate::notification::NotificationManager::notify_error(
                    crate::notification::Notification::new("Dataset unavailable", &e.to_string()),
                );
            }
        }
    }

    /// Render the built-in chart window and service its picker actions.
    fn render_chart_window(&mut self, ctx: &egui::Context) {
        use crate::components::chart_window::ChartAction;
        if !self.chart.open {
            return;
        }
        let mut open = self.chart.open;
        let mut action = None;
        egui::Window::new("Chart")
            .open(&mut open)
            .resizable(true)
            .default_size([640.0, 460.0])
            .show(ctx, |ui| {
                action = self.chart.render(ui);
            });
        self.chart.open = open;
        match action {
            Some(ChartAction::Pick(id)) => self.chart_fetch_from(id),
            Some(ChartAction::ChangeSource) => {
                let producers = self.gather_producers();
                self.chart.open_picker(producers);
            }
            None => {}
        }
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
                        components::error_modal::ErrorModalProps {
                            error: &error,
                            open: true,
                        },
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

    fn render_update_consent_modal(&mut self, ui: &mut egui::Ui) {
        use super::update_handler::ConsentAction;
        match UpdateHandler::render_consent_modal(ui, &self.update_state, self.show_update_consent)
        {
            Some(ConsentAction::RemindLater) => self.show_update_consent = false,
            Some(ConsentAction::UpdateNow) => {
                self.show_update_consent = false;
                if !self.settings_dialog.open {
                    self.settings_dialog.open_updates(&self.settings);
                }
            }
            None => {}
        }
    }
}
