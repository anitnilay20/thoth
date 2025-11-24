use crate::{components, settings, state, update};
use eframe::egui;

/// Handles all update-related logic
pub struct UpdateHandler;

impl UpdateHandler {
    /// Check if updates should be checked on startup
    pub fn should_check_updates(
        update_state: &state::ApplicationUpdateState,
        settings: &settings::Settings,
    ) -> bool {
        update_state.update_status.should_check(
            settings.updates.check_interval_hours,
            settings.updates.auto_check,
        )
    }

    /// Initiate update check
    pub fn check_for_updates(update_state: &mut state::ApplicationUpdateState) {
        update_state.update_manager.check_for_updates();
        update_state.update_status.state = update::UpdateState::Checking;
        update_state.update_status.last_check = Some(chrono::Utc::now());
    }

    /// Process incoming update messages
    /// Returns true if settings panel should be shown
    pub fn handle_update_messages(
        update_state: &mut state::ApplicationUpdateState,
        ctx: &egui::Context,
    ) -> bool {
        let mut should_show_settings = false;
        while let Ok(msg) = update_state.update_manager.receiver().try_recv() {
            match msg {
                update::manager::UpdateMessage::UpdateCheckComplete(result) => {
                    if Self::handle_check_complete(result, update_state) {
                        should_show_settings = true;
                    }
                }
                update::manager::UpdateMessage::DownloadProgress(progress) => {
                    Self::handle_download_progress(progress, update_state, ctx);
                }
                update::manager::UpdateMessage::DownloadComplete(result) => {
                    Self::handle_download_complete(result, update_state);
                }
                update::manager::UpdateMessage::InstallComplete(result) => {
                    Self::handle_install_complete(result, update_state, ctx);
                }
            }
            ctx.request_repaint();
        }
        should_show_settings
    }

    /// Handle settings panel events related to updates
    pub fn handle_settings_action(
        event: components::settings_panel::SettingsPanelEvent,
        update_state: &mut state::ApplicationUpdateState,
        ctx: &egui::Context,
    ) {
        match event {
            components::settings_panel::SettingsPanelEvent::CheckForUpdates => {
                Self::check_for_updates(update_state);
            }
            components::settings_panel::SettingsPanelEvent::DownloadUpdate => {
                if let update::UpdateState::UpdateAvailable { releases, .. } =
                    &update_state.update_status.state
                {
                    if let Some(latest) = releases.first() {
                        Self::start_download(latest.clone(), update_state, ctx);
                    }
                }
            }
            components::settings_panel::SettingsPanelEvent::InstallUpdate => {
                if let Some(path) = update_state.pending_install_path.take() {
                    update_state.update_status.state = update::UpdateState::Installing;
                    update_state.update_manager.install_update(path);
                }
            }
            components::settings_panel::SettingsPanelEvent::RetryUpdate => {
                Self::check_for_updates(update_state);
            }
        }
    }

    // Private helper methods
    fn handle_check_complete(
        result: Result<Vec<update::ReleaseInfo>, String>,
        update_state: &mut state::ApplicationUpdateState,
    ) -> bool {
        let mut should_show_settings = false;

        match result {
            Ok(releases) => {
                if update::UpdateManager::has_newer_version(&releases) {
                    let newer_releases = update::UpdateManager::get_newer_releases(&releases);
                    if let Some(latest) = newer_releases.first() {
                        update_state.update_status.state = update::UpdateState::UpdateAvailable {
                            latest_version: latest.tag_name.clone(),
                            current_version: update::UpdateManager::get_current_version()
                                .to_string(),
                            releases: newer_releases,
                        };

                        // Auto-open settings panel on first update notification
                        if !update_state.update_notification_shown {
                            should_show_settings = true;
                            update_state.update_notification_shown = true;
                        }
                    }
                } else {
                    update_state.update_status.state = update::UpdateState::Idle;
                }
            }
            Err(e) => {
                update_state.update_status.state = update::UpdateState::Error(e);
            }
        }

        should_show_settings
    }

    fn handle_download_progress(
        progress: f32,
        update_state: &mut state::ApplicationUpdateState,
        ctx: &egui::Context,
    ) {
        if let update::UpdateState::Downloading { version, .. } = &update_state.update_status.state
        {
            update_state.update_status.state = update::UpdateState::Downloading {
                progress,
                version: version.clone(),
            };
            ctx.request_repaint();
        }
    }

    fn handle_download_complete(
        result: Result<std::path::PathBuf, String>,
        update_state: &mut state::ApplicationUpdateState,
    ) {
        match result {
            Ok(path) => {
                if let Some(release) = &update_state.pending_download_release {
                    update_state.update_status.state = update::UpdateState::ReadyToInstall {
                        version: release.tag_name.clone(),
                        path: path.clone(),
                    };
                    update_state.pending_install_path = Some(path);
                }
            }
            Err(e) => {
                update_state.update_status.state = update::UpdateState::Error(e);
            }
        }
    }

    fn handle_install_complete(
        result: Result<(), String>,
        update_state: &mut state::ApplicationUpdateState,
        ctx: &egui::Context,
    ) {
        match result {
            Ok(_) => {
                // Installation successful, restart application
                ctx.send_viewport_cmd(egui::ViewportCommand::Close);
            }
            Err(e) => {
                update_state.update_status.state = update::UpdateState::Error(e);
            }
        }
    }

    fn start_download(
        release: update::ReleaseInfo,
        update_state: &mut state::ApplicationUpdateState,
        ctx: &egui::Context,
    ) {
        update_state.pending_download_release = Some(release.clone());
        update_state.update_status.state = update::UpdateState::Downloading {
            progress: 0.0,
            version: release.tag_name.clone(),
        };
        update_state.update_manager.download_update(&release);
        ctx.request_repaint();
    }
}
