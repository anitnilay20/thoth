use crate::{error::ThothError, settings, state, update};
use eframe::egui;

pub enum ConsentAction {
    UpdateNow,
    RemindLater,
}

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

    /// Process incoming update messages.
    /// Returns `true` once when a new update is detected (first time only per session).
    pub fn handle_update_messages(
        update_state: &mut state::ApplicationUpdateState,
        ctx: &egui::Context,
    ) -> bool {
        let mut update_detected = false;
        while let Ok(msg) = update_state.update_manager.receiver().try_recv() {
            match msg {
                update::manager::UpdateMessage::UpdateCheckComplete(result) => {
                    if Self::handle_check_complete(result, update_state) {
                        update_detected = true;
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
        update_detected
    }

    // Private helper methods
    fn handle_check_complete(
        result: Result<Vec<update::ReleaseInfo>, ThothError>,
        update_state: &mut state::ApplicationUpdateState,
    ) -> bool {
        let mut update_detected = false;

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

                        if !update_state.update_notification_shown {
                            update_detected = true;
                            update_state.update_notification_shown = true;
                        }
                    }
                } else {
                    update_state.update_status.state = update::UpdateState::Idle;
                    crate::notification::NotificationManager::remove_notification(
                        "thoth_update_available",
                    );
                }
            }
            Err(e) => {
                update_state.update_status.state = update::UpdateState::Error(e);
            }
        }

        update_detected
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
        result: Result<std::path::PathBuf, ThothError>,
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
        result: Result<(), ThothError>,
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

    /// Render the update consent modal. Returns the action the user chose, or `None` if the
    /// modal is not visible (no update available or already dismissed for this session).
    pub fn render_consent_modal(
        ui: &mut egui::Ui,
        update_state: &state::ApplicationUpdateState,
        show_consent: bool,
    ) -> Option<ConsentAction> {
        if !show_consent {
            return None;
        }

        let (current_version, latest_version) = if let update::UpdateState::UpdateAvailable {
            ref current_version,
            ref latest_version,
            ..
        } = update_state.update_status.state
        {
            (current_version.clone(), latest_version.clone())
        } else {
            return None;
        };

        use crate::components::{
            traits::StatelessComponent,
            update_consent_modal::{UpdateConsentModal, UpdateConsentModalProps},
        };

        let out = UpdateConsentModal::render(
            ui,
            UpdateConsentModalProps {
                current_version: &current_version,
                latest_version: &latest_version,
            },
        );

        if out.update_now {
            Some(ConsentAction::UpdateNow)
        } else if out.remind_later {
            Some(ConsentAction::RemindLater)
        } else {
            None
        }
    }

    /// Post (or refresh) the pinned update-available notification with an "Update Now" action.
    pub fn post_update_notification(update_state: &state::ApplicationUpdateState) {
        let (current_version, latest_version) = if let update::UpdateState::UpdateAvailable {
            ref current_version,
            ref latest_version,
            ..
        } = update_state.update_status.state
        {
            (current_version.clone(), latest_version.clone())
        } else {
            return;
        };

        crate::notification::NotificationManager::notify(
            crate::notification::Notification::new(
                "Update Available",
                &format!("v{current_version} → v{latest_version}"),
            )
            .with_id("thoth_update_available")
            .with_kind(crate::notification::NotificationKind::Update)
            .with_toast(false)
            .with_action(
                "Update Now",
                std::sync::Arc::new(|| {
                    crate::OPEN_UPDATES_REQUESTED.store(true, std::sync::atomic::Ordering::Relaxed);
                }),
            )
            .pinned(),
        );
    }
}
