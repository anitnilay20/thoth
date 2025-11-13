use eframe::{App, Frame, egui};

use crate::{components, components::traits::ContextComponent, settings, state};

use super::{
    ShortcutAction, search_handler::SearchHandler, shortcut_handler::ShortcutHandler,
    update_handler::UpdateHandler,
};
use crate::components::settings_panel::SettingsPanelProps;

pub struct ThothApp {
    // Settings for this window
    pub settings: settings::Settings,

    // Window state
    pub window_state: state::WindowState,

    // Update state
    pub update_state: state::ApplicationUpdateState,

    // Settings panel (UI)
    pub settings_panel: components::settings_panel::SettingsPanel,
    pub show_settings: bool,

    // Clipboard text to copy (set by shortcuts, copied in update loop)
    clipboard_text: Option<String>,
}

impl ThothApp {
    /// Create a new ThothApp with loaded settings
    pub fn new(settings: settings::Settings) -> Self {
        Self {
            settings,
            window_state: state::WindowState::default(),
            update_state: state::ApplicationUpdateState::default(),
            settings_panel: components::settings_panel::SettingsPanel::default(),
            show_settings: false,
            clipboard_text: None,
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
        if UpdateHandler::handle_update_messages(&mut self.update_state, ctx) {
            self.show_settings = true;
        }

        // Handle file drops
        self.handle_file_drop(ctx);

        // Update window title
        self.update_window_title(ctx);

        // Get user's action from Toolbar
        let incoming_msg = self.render_toolbar(ctx);

        // Handle search messages
        let msg_to_central = SearchHandler::handle_search_messages(
            incoming_msg,
            &mut self.window_state.search_engine_state,
            &self.window_state.file_path,
            &self.window_state.file_type,
            ctx,
        );

        // Apply theme and font settings
        crate::theme::apply_theme(ctx, &self.settings);

        // Save settings when dark mode changes
        self.save_settings_if_changed(ctx);

        // Render the settings panel and handle actions
        self.render_settings_panel(ctx);

        // Render the central panel
        self.window_state.central_panel.ui(
            ctx,
            &self.window_state.file_path,
            &mut self.window_state.file_type,
            &mut self.window_state.error,
            msg_to_central,
        );
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
                    self.show_settings = !self.show_settings;
                }
                ShortcutAction::ToggleTheme => {
                    self.settings.dark_mode = !self.settings.dark_mode;
                }
                // Navigation shortcuts - handled by JSON viewer or search
                ShortcutAction::FocusSearch => {
                    // Request focus on search box
                    self.window_state.toolbar.request_search_focus = true;
                }
                ShortcutAction::NextMatch => {
                    // TODO: Implement next match navigation
                }
                ShortcutAction::PrevMatch => {
                    // TODO: Implement previous match navigation
                }
                ShortcutAction::Escape => {
                    // Clear search or close panels
                    if self.show_settings {
                        self.show_settings = false;
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

    /// Render toolbar and return any search messages
    fn render_toolbar(&mut self, ctx: &egui::Context) -> Option<crate::search::SearchMessage> {
        let update_available = UpdateHandler::is_update_available(&self.update_state);

        // Render toolbar using ContextComponent trait with one-way binding
        let output = self.window_state.toolbar.render(
            ctx,
            components::toolbar::ToolbarProps {
                file_type: &self.window_state.file_type,
                dark_mode: self.settings.dark_mode,
                update_available,
                shortcuts: &self.settings.shortcuts,
            },
        );

        // Handle events emitted by the toolbar (bottom-to-top communication)
        for event in output.events {
            match event {
                components::toolbar::ToolbarEvent::FileOpen { path, file_type } => {
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
                components::toolbar::ToolbarEvent::ToggleSettings => {
                    self.show_settings = !self.show_settings;
                }
                components::toolbar::ToolbarEvent::ToggleTheme => {
                    self.settings.dark_mode = !self.settings.dark_mode;
                }
            }
        }

        output.search_message
    }

    /// Save settings if they have changed
    fn save_settings_if_changed(&mut self, ctx: &egui::Context) {
        if ctx.style().visuals.dark_mode != self.settings.dark_mode {
            if let Err(e) = self.settings.save() {
                eprintln!("Failed to save settings: {}", e);
            }
        }
    }

    /// Render settings panel and handle actions
    fn render_settings_panel(&mut self, ctx: &egui::Context) {
        // Render settings panel using ContextComponent trait with one-way binding
        let output = self.settings_panel.render(
            ctx,
            SettingsPanelProps {
                show: self.show_settings,
                update_status: &self.update_state.update_status,
                current_version: crate::update::UpdateManager::get_current_version(),
            },
        );

        // Handle events emitted by the settings panel (bottom-to-top communication)
        for event in output.events {
            match event {
                components::settings_panel::SettingsPanelEvent::Close => {
                    self.show_settings = false;
                }
                components::settings_panel::SettingsPanelEvent::CheckForUpdates => {
                    UpdateHandler::handle_settings_action(
                        components::settings_panel::SettingsPanelEvent::CheckForUpdates,
                        &mut self.update_state,
                        ctx,
                    );
                }
                components::settings_panel::SettingsPanelEvent::DownloadUpdate => {
                    UpdateHandler::handle_settings_action(
                        components::settings_panel::SettingsPanelEvent::DownloadUpdate,
                        &mut self.update_state,
                        ctx,
                    );
                }
                components::settings_panel::SettingsPanelEvent::InstallUpdate => {
                    UpdateHandler::handle_settings_action(
                        components::settings_panel::SettingsPanelEvent::InstallUpdate,
                        &mut self.update_state,
                        ctx,
                    );
                }
                components::settings_panel::SettingsPanelEvent::RetryUpdate => {
                    UpdateHandler::handle_settings_action(
                        components::settings_panel::SettingsPanelEvent::RetryUpdate,
                        &mut self.update_state,
                        ctx,
                    );
                }
            }
        }
    }
}
