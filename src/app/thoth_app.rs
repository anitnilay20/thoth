use eframe::{App, Frame, egui};

use crate::{components, settings, state, window};

use super::{search_handler::SearchHandler, update_handler::UpdateHandler};

pub struct ThothApp {
    // Shared state across all windows
    pub shared_state: state::SharedState,

    // Main window state
    pub window_state: state::WindowState,

    // Window manager for additional windows
    pub window_manager: window::WindowManager,

    // Update state (global for now, could be per-window later)
    pub update_state: state::ApplicationUpdateState,

    // Settings panel (global UI)
    pub settings_panel: components::settings_panel::SettingsPanel,
}

impl ThothApp {
    /// Create a new ThothApp with loaded settings
    pub fn new(settings: settings::Settings) -> Self {
        let shared_state = state::SharedState::new(settings);
        Self {
            window_manager: window::WindowManager::new(shared_state.clone()),
            shared_state,
            window_state: state::WindowState::default(),
            update_state: state::ApplicationUpdateState::default(),
            settings_panel: components::settings_panel::SettingsPanel::default(),
        }
    }

    /// Create a new window
    pub fn create_new_window(&mut self) {
        self.window_manager.request_new_window();
    }
}

impl App for ThothApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        // Show all windows every frame (required for deferred viewports)
        // This also handles cleanup of closed windows internally
        self.window_manager.show_windows(ctx);

        // Get settings (lock for minimal time)
        let settings = {
            let mut settings = self.shared_state.settings.lock().unwrap();
            // Sync dark mode from context (in case it was changed externally)
            settings.dark_mode = ctx.style().visuals.dark_mode;
            settings.clone()
        };

        // Check for updates based on settings
        if UpdateHandler::should_check_updates(&self.update_state, &settings) {
            UpdateHandler::check_for_updates(&mut self.update_state);
        }

        // Handle update messages
        UpdateHandler::handle_update_messages(
            &mut self.update_state,
            &mut self.settings_panel,
            ctx,
        );

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
        crate::theme::apply_theme(ctx, &settings);

        // Save settings when dark mode changes
        self.save_settings_if_changed(ctx, &settings);

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

        let mut dark_mode = self.shared_state.settings.lock().unwrap().dark_mode;
        let mut new_window_requested = false;

        let result = self.window_state.toolbar.ui(
            ctx,
            &mut components::toolbar::ToolbarState {
                file_path: &mut self.window_state.file_path,
                file_type: &mut self.window_state.file_type,
                error: &mut self.window_state.error,
                dark_mode: &mut dark_mode,
                show_settings: &mut self.settings_panel.show,
                update_available,
                new_window_requested: &mut new_window_requested,
            },
        );

        // Update settings if dark mode changed
        self.shared_state.settings.lock().unwrap().dark_mode = dark_mode;

        // Handle new window request
        if new_window_requested {
            self.create_new_window();
        }

        result
    }

    /// Save settings if they have changed
    fn save_settings_if_changed(&mut self, ctx: &egui::Context, settings: &settings::Settings) {
        if ctx.style().visuals.dark_mode != settings.dark_mode {
            if let Err(e) = settings.save() {
                eprintln!("Failed to save settings: {}", e);
            }
        }
    }

    /// Render settings panel and handle actions
    fn render_settings_panel(&mut self, ctx: &egui::Context) {
        if let Some(action) = self.settings_panel.render(
            ctx,
            &self.update_state.update_status,
            crate::update::UpdateManager::get_current_version(),
        ) {
            UpdateHandler::handle_settings_action(action, &mut self.update_state, ctx);
        }
    }
}
