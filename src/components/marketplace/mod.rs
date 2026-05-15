mod detail;
mod list;
mod state;

use eframe::egui;

use crate::theme::ThemeColors;
use crate::{components::traits::StatelessComponent, settings};

use state::{DetailAction, InstallState, MarketplaceUiState};

const STATE_ID: &str = "marketplace_ui_state";

// ── Sidebar list ──────────────────────────────────────────────────────────────

pub struct Marketplace;
pub struct MarketplaceProps;
pub struct MarketplaceOutput;

impl StatelessComponent for Marketplace {
    type Props<'a> = MarketplaceProps;
    type Output = MarketplaceOutput;

    fn render(ui: &mut egui::Ui, _props: Self::Props<'_>) -> Self::Output {
        let state_id = egui::Id::new(STATE_ID);
        let mut state: MarketplaceUiState =
            ui.ctx().data(|d| d.get_temp(state_id).unwrap_or_default());

        state.load_if_needed(ui.ctx(), false);
        let setting = settings::Settings::read(ui.ctx());
        state.poll_pending(&setting.plugins.disabled_plugin_ids);

        let colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| crate::theme::Theme::default().colors())
        });

        list::render(ui, &mut state, &colors);

        ui.ctx().data_mut(|d| d.insert_temp(state_id, state));
        MarketplaceOutput
    }
}

// ── Central detail ────────────────────────────────────────────────────────────

pub struct MarketplaceDetail;
pub struct MarketplaceDetailProps;
pub struct MarketplaceDetailOutput;

impl StatelessComponent for MarketplaceDetail {
    type Props<'a> = MarketplaceDetailProps;
    type Output = MarketplaceDetailOutput;

    fn render(ui: &mut egui::Ui, _props: Self::Props<'_>) -> Self::Output {
        let state_id = egui::Id::new(STATE_ID);
        let mut state: MarketplaceUiState =
            ui.ctx().data(|d| d.get_temp(state_id).unwrap_or_default());

        let pending_resolved = state
            .pending
            .as_ref()
            .is_some_and(|slot| slot.lock().ok().is_some_and(|g| g.is_some()));

        let setting = settings::Settings::read(ui.ctx());
        state.poll_pending(&setting.plugins.disabled_plugin_ids);

        let colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| crate::theme::Theme::default().colors())
        });

        let selected_plugin = state
            .selected_id
            .as_ref()
            .and_then(|id| state.plugins.iter().find(|p| &p.id == id))
            .cloned();

        // Poll first so install_state reflects the latest progress from the background thread.
        let completed = state.poll_installs();
        let has_active_installs = !state.install_handles.is_empty();
        let mut state_changed = pending_resolved || !completed.is_empty() || has_active_installs;

        // Read install_state AFTER poll so the banner gets the current percentage.
        let install_state = selected_plugin
            .as_ref()
            .and_then(|p| state.install_states.get(&p.id))
            .cloned()
            .unwrap_or_default();

        for (id, result) in completed {
            if result.is_ok() {
                let name = state
                    .plugins
                    .iter()
                    .find(|p| p.id == id)
                    .map(|p| p.name.clone())
                    .unwrap_or_else(|| id.clone());
                crate::notification::NotificationManager::notify(
                    crate::notification::Notification::new(
                        "Plugin installed",
                        &format!("{name} was installed. Restart Thoth to activate it."),
                    )
                    .with_id("plugin_install_complete")
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

        if let Some(plugin) = &selected_plugin {
            if let Some(action) = detail::render(ui, plugin, &install_state, &colors) {
                match action {
                    DetailAction::Install => {
                        let slot = plugin.download_and_install(ui.ctx().clone());
                        state.install_handles.insert(plugin.id.clone(), slot);
                        state
                            .install_states
                            .insert(plugin.id.clone(), InstallState::Installing(0));
                    }
                    DetailAction::Uninstall => {
                        let plugin_id = plugin.id.clone();
                        let plugin_name = plugin.name.clone();

                        let result = crate::app::persistent_state::PersistentState::plugin_install_dir_by_id(&plugin_id)
                            .and_then(|dir| {
                                if dir.exists() {
                                    std::fs::remove_dir_all(&dir).map_err(|e| e.to_string())?;
                                }
                                Ok(())
                            });

                        match result {
                            Ok(()) => {
                                state.install_states.remove(&plugin_id);
                                // Also remove from disabled list so a reinstall isn't blocked.
                                crate::settings::Settings::update(ui.ctx(), |s| {
                                    s.plugins.disabled_plugin_ids.retain(|id| *id != plugin_id);
                                });
                                crate::notification::NotificationManager::notify(
                                    crate::notification::Notification::new(
                                        "Plugin uninstalled",
                                        &format!(
                                            "{plugin_name} was removed. Restart Thoth to apply."
                                        ),
                                    )
                                    .with_id("plugin_uninstall_complete")
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
                            Err(e) => {
                                crate::notification::NotificationManager::notify(
                                    crate::notification::Notification::new(
                                        "Uninstall failed",
                                        &format!("Could not remove {plugin_name}: {e}"),
                                    )
                                    .with_toast(true)
                                    .with_status(crate::notification::NotificationStatus::Error),
                                );
                            }
                        }
                    }
                    DetailAction::Enable => {
                        state
                            .install_states
                            .insert(plugin.id.clone(), InstallState::Installed);

                        settings::Settings::update(ui.ctx(), |s| {
                            s.plugins
                                .disabled_plugin_ids
                                .retain(|f| *f != plugin.id.clone());
                        });
                    }
                    DetailAction::Disable => {
                        state
                            .install_states
                            .insert(plugin.id.clone(), InstallState::Disabled);

                        settings::Settings::update(ui.ctx(), |s| {
                            s.plugins.disabled_plugin_ids.push(plugin.id.clone())
                        });
                    }
                    DetailAction::Retry => {
                        state.install_handles.remove(&plugin.id);
                        state.install_states.remove(&plugin.id);
                    }
                }
                state_changed = true;
            }
        } else {
            detail::render_empty(ui, &colors);
        }

        if state_changed {
            ui.ctx().data_mut(|d| d.insert_temp(state_id, state));
        }

        MarketplaceDetailOutput
    }
}
