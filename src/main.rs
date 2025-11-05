#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

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
            },
        );

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
    }
}

impl ThothApp {
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
