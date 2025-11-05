#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use eframe::{
    App, Frame, NativeOptions,
    egui::{self},
};

use crate::{components::theme, helpers::load_icon};

mod components;
mod file;
mod helpers;
mod search;
mod update;

// Per-window state that can be used by child windows
struct ChildWindowState {
    toolbar: components::toolbar::Toolbar,
    central_panel: components::central_panel::CentralPanel,
    error: Option<String>,
    file_path: Option<PathBuf>,
    file_type: file::lazy_loader::FileType,
    search: search::Search,
    search_rx: Option<std::sync::mpsc::Receiver<search::Search>>,
}

impl Default for ChildWindowState {
    fn default() -> Self {
        Self {
            toolbar: Default::default(),
            central_panel: Default::default(),
            error: None,
            file_path: None,
            file_type: Default::default(),
            search: Default::default(),
            search_rx: None,
        }
    }
}

struct ThothApp {
    toolbar: components::toolbar::Toolbar,
    central_panel: components::central_panel::CentralPanel,
    settings_panel: components::settings_panel::SettingsPanel,
    error: Option<String>,
    file_path: Option<PathBuf>,
    file_type: file::lazy_loader::FileType,

    // search engine state
    search: search::Search,
    search_rx: Option<std::sync::mpsc::Receiver<search::Search>>,

    // update state
    update_manager: update::UpdateManager,
    update_status: update::UpdateStatus,
    pending_download_release: Option<update::ReleaseInfo>,
    pending_install_path: Option<PathBuf>,
    update_notification_shown: bool,

    // UI
    dark_mode: bool,

    // Multi-window support
    child_windows: HashMap<usize, ChildWindowState>,
    next_window_id: usize,
}

impl Default for ThothApp {
    fn default() -> Self {
        let update_manager = update::UpdateManager::new();
        Self {
            toolbar: Default::default(),
            central_panel: Default::default(),
            settings_panel: Default::default(),
            error: None,
            file_path: None,
            file_type: Default::default(),
            search: Default::default(),
            search_rx: None,
            update_manager,
            update_status: Default::default(),
            pending_download_release: None,
            pending_install_path: None,
            update_notification_shown: false,
            dark_mode: false,
            child_windows: HashMap::new(),
            next_window_id: 1,
        }
    }
}

impl App for ThothApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        self.dark_mode = ctx.style().visuals.dark_mode;

        // Check for updates on startup or every 24 hours
        if self.update_status.should_check() {
            self.update_manager.check_for_updates();
            self.update_status.state = update::UpdateState::Checking;
            self.update_status.last_check = Some(chrono::Utc::now());
        }

        // Handle update messages
        self.handle_update_messages(ctx);

        self.handle_file_drop(ctx);

        // Check for Ctrl/Cmd+N to open new window
        let mut new_window_requested = false;
        ctx.input(|i| {
            if i.modifiers.command && i.key_pressed(egui::Key::N) {
                new_window_requested = true;
            }
        });

        if let Some(path) = &self.file_path {
            let file_name = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown file");
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                "Thoth — {}",
                file_name
            )));
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(
                "Thoth — JSON & NDJSON Viewer".to_owned(),
            ));
        }

        // Check if update is available
        let update_available = matches!(
            self.update_status.state,
            update::UpdateState::UpdateAvailable { .. }
        );

        // Get user's action from Toolbar (open file / change type / search / stop)
        let incoming_msg = self.toolbar.ui(
            ctx,
            &mut components::toolbar::ToolbarState {
                file_path: &mut self.file_path,
                file_type: &mut self.file_type,
                error: &mut self.error,
                dark_mode: &mut self.dark_mode,
                show_settings: &mut self.settings_panel.show,
                update_available,
                new_window_requested: &mut new_window_requested,
            },
        );

        // Create new window if requested
        if new_window_requested {
            let new_id = self.next_window_id;
            self.next_window_id += 1;
            self.child_windows
                .insert(new_id, ChildWindowState::default());
        }

        // We will forward a processed message (with results) to the CentralPanel
        let mut msg_to_central: Option<search::SearchMessage> = None;

        if let Some(rx) = &self.search_rx {
            if let Ok(done) = rx.try_recv() {
                self.search = done.clone(); // finished: scanning=false, results filled
                msg_to_central = Some(search::SearchMessage::StartSearch(done));
                self.search_rx = None; // finished
            }
        }

        if let Some(msg) = incoming_msg {
            match msg {
                search::SearchMessage::StartSearch(s) => {
                    // kick off background
                    self.search = s.clone();
                    self.search.scanning = true;

                    // tell CentralPanel to show loader NOW
                    msg_to_central = Some(search::SearchMessage::StartSearch(self.search.clone()));

                    // spawn and keep receiver
                    self.search_rx =
                        Some(self.search.start_scanning(&self.file_path, &self.file_type));

                    // keep UI repainting while scanning
                    ctx.request_repaint();
                }
                search::SearchMessage::StopSearch => {
                    self.search_rx = None; // optional: drop pending result
                    msg_to_central = Some(search::SearchMessage::StopSearch);
                }
            }
        }

        theme::apply_theme(ctx, self.dark_mode); // Always dark mode

        // Render the settings panel and handle actions
        if let Some(action) = self.settings_panel.render(
            ctx,
            &self.update_status.state,
            update::UpdateManager::get_current_version(),
        ) {
            self.handle_settings_action(action, ctx);
        }

        // Render the central panel, passing the processed search message (if any)
        self.central_panel.ui(
            ctx,
            &self.file_path,
            &mut self.file_type,
            &mut self.error,
            msg_to_central,
        );

        // Render child windows
        self.render_child_windows(ctx);
    }
}

impl ThothApp {
    fn render_child_windows(&mut self, ctx: &egui::Context) {
        // Keep track of which windows should be closed
        let mut windows_to_remove = Vec::new();

        // Get window IDs to iterate over (to avoid borrow issues)
        let window_ids: Vec<usize> = self.child_windows.keys().copied().collect();

        for window_id in window_ids {
            let mut should_close = false;

            // Get the window state (we need to work around borrow checker here)
            if let Some(state) = self.child_windows.get_mut(&window_id) {
                ctx.show_viewport_immediate(
                    egui::ViewportId::from_hash_of(("child_window", window_id)),
                    egui::ViewportBuilder::default()
                        .with_title("Thoth — JSON & NDJSON Viewer")
                        .with_inner_size([1200.0, 800.0]),
                    |ctx, _class| {
                        // Apply theme
                        theme::apply_theme(ctx, ctx.style().visuals.dark_mode);

                        // Check if window should close
                        if ctx.input(|i| i.viewport().close_requested()) {
                            should_close = true;
                            return;
                        }

                        // Render the full Thoth UI for this window
                        Self::render_window_ui(ctx, state);
                    },
                );
            }

            if should_close {
                windows_to_remove.push(window_id);
            }
        }

        // Remove closed windows
        for id in windows_to_remove {
            self.child_windows.remove(&id);
        }
    }

    fn render_window_ui(ctx: &egui::Context, state: &mut ChildWindowState) {
        // Handle file drop
        let hovering_files = ctx.input(|i| i.raw.hovered_files.clone());
        if !hovering_files.is_empty() {
            let mut text = String::from("Drop file to open:\n");
            for file in &hovering_files {
                if let Some(path) = &file.path {
                    use std::fmt::Write as _;
                    let _ = write!(text, "\n{}", path.display());
                } else if !file.mime.is_empty() {
                    use std::fmt::Write as _;
                    let _ = write!(text, "\n{}", file.mime);
                }
            }

            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("file_drop_overlay"),
            ));
            let screen_rect = ctx.screen_rect();
            painter.rect_filled(screen_rect, 0.0, egui::Color32::from_black_alpha(180));
            painter.text(
                screen_rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::TextStyle::Heading.resolve(&ctx.style()),
                egui::Color32::WHITE,
            );
        }

        // Handle dropped files
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            for file in dropped_files {
                if let Some(path) = file.path {
                    match file::detect_file_type::sniff_file_type(&path) {
                        Ok(detected) => {
                            let ft: file::lazy_loader::FileType = detected.into();
                            state.file_type = ft;
                            state.file_path = Some(path);
                            state.error = None;
                            state.toolbar.previous_file_type = ft;
                        }
                        Err(e) => {
                            state.error = Some(format!(
                                "Failed to detect file type (expect JSON / NDJSON): {e}"
                            ));
                        }
                    }
                    break;
                }
            }
        }

        // Update window title
        if let Some(path) = &state.file_path {
            let file_name = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown file");
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                "Thoth — {}",
                file_name
            )));
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(
                "Thoth — JSON & NDJSON Viewer".to_owned(),
            ));
        }

        // Render toolbar (child windows don't show settings or update notifications)
        let mut child_new_window_requested = false; // Child windows can't spawn new windows
        let incoming_msg = state.toolbar.ui(
            ctx,
            &mut components::toolbar::ToolbarState {
                file_path: &mut state.file_path,
                file_type: &mut state.file_type,
                error: &mut state.error,
                dark_mode: &mut false, // Don't let child windows control dark mode
                show_settings: &mut false, // Don't show settings in child windows
                update_available: false,
                new_window_requested: &mut child_new_window_requested,
            },
        );

        // Handle search messages
        let mut msg_to_central: Option<search::SearchMessage> = None;

        if let Some(rx) = &state.search_rx {
            if let Ok(done) = rx.try_recv() {
                state.search = done.clone();
                msg_to_central = Some(search::SearchMessage::StartSearch(done));
                state.search_rx = None;
            }
        }

        if let Some(msg) = incoming_msg {
            match msg {
                search::SearchMessage::StartSearch(s) => {
                    state.search = s.clone();
                    state.search.scanning = true;
                    msg_to_central = Some(search::SearchMessage::StartSearch(state.search.clone()));

                    let rx = state
                        .search
                        .start_scanning(&state.file_path, &state.file_type);
                    state.search_rx = Some(rx);

                    ctx.request_repaint();
                }
                search::SearchMessage::StopSearch => {
                    state.search_rx = None;
                    msg_to_central = Some(search::SearchMessage::StopSearch);
                }
            }
        }

        // Render the central panel
        state.central_panel.ui(
            ctx,
            &state.file_path,
            &mut state.file_type,
            &mut state.error,
            msg_to_central,
        );
    }

    fn handle_update_messages(&mut self, ctx: &egui::Context) {
        while let Ok(msg) = self.update_manager.receiver().try_recv() {
            match msg {
                update::manager::UpdateMessage::UpdateCheckComplete(result) => match result {
                    Ok(releases) => {
                        if update::UpdateManager::has_newer_version(&releases) {
                            let newer_releases =
                                update::UpdateManager::get_newer_releases(&releases);
                            if let Some(latest) = newer_releases.first() {
                                self.update_status.state = update::UpdateState::UpdateAvailable {
                                    latest_version: latest.tag_name.clone(),
                                    current_version: update::UpdateManager::get_current_version()
                                        .to_string(),
                                    releases: newer_releases,
                                };

                                // Auto-open settings panel on first update notification
                                if !self.update_notification_shown {
                                    self.settings_panel.show = true;
                                    self.update_notification_shown = true;
                                }
                            }
                        } else {
                            self.update_status.state = update::UpdateState::Idle;
                        }
                    }
                    Err(e) => {
                        self.update_status.state = update::UpdateState::Error(e);
                    }
                },
                update::manager::UpdateMessage::DownloadProgress(progress) => {
                    if let update::UpdateState::Downloading { version, .. } =
                        &self.update_status.state
                    {
                        self.update_status.state = update::UpdateState::Downloading {
                            progress,
                            version: version.clone(),
                        };
                        ctx.request_repaint();
                    }
                }
                update::manager::UpdateMessage::DownloadComplete(result) => match result {
                    Ok(path) => {
                        if let Some(release) = &self.pending_download_release {
                            self.update_status.state = update::UpdateState::ReadyToInstall {
                                version: release.tag_name.clone(),
                                path: path.clone(),
                            };
                            self.pending_install_path = Some(path);
                        }
                    }
                    Err(e) => {
                        self.update_status.state = update::UpdateState::Error(e);
                    }
                },
                update::manager::UpdateMessage::InstallComplete(result) => match result {
                    Ok(_) => {
                        // Installation successful, restart application
                        ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                    }
                    Err(e) => {
                        self.update_status.state = update::UpdateState::Error(e);
                    }
                },
            }
        }
    }

    fn handle_settings_action(
        &mut self,
        action: components::settings_panel::SettingsAction,
        ctx: &egui::Context,
    ) {
        match action {
            components::settings_panel::SettingsAction::CheckForUpdates => {
                self.update_manager.check_for_updates();
                self.update_status.state = update::UpdateState::Checking;
                self.update_status.last_check = Some(chrono::Utc::now());
            }
            components::settings_panel::SettingsAction::DownloadUpdate => {
                if let update::UpdateState::UpdateAvailable { releases, .. } =
                    &self.update_status.state
                {
                    if let Some(latest) = releases.first() {
                        self.show_download_confirmation(latest.clone(), ctx);
                    }
                }
            }
            components::settings_panel::SettingsAction::InstallUpdate => {
                if let Some(path) = self.pending_install_path.take() {
                    self.update_status.state = update::UpdateState::Installing;
                    self.update_manager.install_update(path);
                }
            }
            components::settings_panel::SettingsAction::RetryUpdate => {
                self.update_manager.check_for_updates();
                self.update_status.state = update::UpdateState::Checking;
                self.update_status.last_check = Some(chrono::Utc::now());
            }
        }
    }

    fn show_download_confirmation(&mut self, release: update::ReleaseInfo, ctx: &egui::Context) {
        // For now, start download immediately. In future, can add a confirmation dialog
        self.pending_download_release = Some(release.clone());
        self.update_status.state = update::UpdateState::Downloading {
            progress: 0.0,
            version: release.tag_name.clone(),
        };
        self.update_manager.download_update(&release);
        ctx.request_repaint();
    }
}

fn main() -> Result<()> {
    let icon = load_icon(include_bytes!("../assets/thoth_icon_256.png"));
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default().with_icon(icon),
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "Thoth — JSON & NDJSON Viewer",
        options,
        Box::new(|_cc| Ok(Box::new(ThothApp::default()))),
    ) {
        eprintln!("Error running application: {e:?}");
        return Err(anyhow::anyhow!("Failed to run application"));
    }
    Ok(())
}
