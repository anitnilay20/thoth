use chrono::{DateTime, Utc};
use eframe::egui;

use crate::components::settings_dialog::helpers::{
    group_rows, section_header, setting_row, slider_control,
};
use crate::components::traits::StatelessComponent;
use crate::settings::UpdateSettings;
use crate::theme::ThemeColors;
use crate::update::UpdateState;
use thoth_plugin_sdk::components::{Button, ButtonColor, ButtonType, ToggleSwitch, Typography};

pub struct UpdatesTab;

pub struct UpdatesTabProps<'a> {
    pub update_settings: &'a UpdateSettings,
    pub update_state: Option<&'a UpdateState>,
    pub last_check: Option<DateTime<Utc>>,
    pub current_version: &'a str,
    pub theme_colors: &'a ThemeColors,
}

#[derive(Debug, Clone)]
pub enum UpdatesTabEvent {
    AutoCheckChanged(bool),
    CheckIntervalChanged(u64),
    CheckForUpdates,
    DownloadUpdate,
    InstallUpdate,
}

pub struct UpdatesTabOutput {
    pub events: Vec<UpdatesTabEvent>,
}

impl StatelessComponent for UpdatesTab {
    type Props<'a> = UpdatesTabProps<'a>;
    type Output = UpdatesTabOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();
        let s = props.update_settings;
        let def = UpdateSettings::default();
        let colors = props.theme_colors;

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                section_header(
                    ui,
                    egui_phosphor::regular::ARROWS_CLOCKWISE,
                    "Updates",
                    "Auto-update and version info.",
                    colors,
                );

                // ── Auto-update ───────────────────────��───────────────────────────
                group_rows(ui, "AUTO-UPDATE", |ui| {
                    setting_row(
                        ui,
                        "Automatically check for updates",
                        Some("Check for new versions periodically in the background."),
                        s.auto_check != def.auto_check,
                        None,
                        colors,
                        |ui| {
                            let on = s.auto_check;
                            if ui
                                .add(ToggleSwitch::builder().enabled(on).build())
                                .clicked()
                            {
                                events.push(UpdatesTabEvent::AutoCheckChanged(!on));
                            }
                        },
                    );

                    setting_row(
                        ui,
                        "Check interval",
                        Some("How often to check for new versions (1–168 hours)."),
                        s.check_interval_hours != def.check_interval_hours,
                        None,
                        colors,
                        |ui| {
                            if let Some(val) = slider_control(
                                ui,
                                s.check_interval_hours as f64,
                                1.0,
                                168.0,
                                "hours",
                            ) {
                                events.push(UpdatesTabEvent::CheckIntervalChanged(
                                    val.round() as u64
                                ));
                            }
                        },
                    );
                });

                // ── Status ────────────────────────────────────────────────────────
                group_rows(ui, "STATUS", |ui| {
                    // Current version
                    setting_row(ui, "Current version", None, false, None, colors, |ui| {
                        Typography::subtitle(ui, props.current_version);
                    });

                    // Last checked
                    let last_check_str = props
                        .last_check
                        .map(|t| {
                            let local: chrono::DateTime<chrono::Local> = t.into();
                            local.format("%b %d, %Y %H:%M").to_string()
                        })
                        .unwrap_or_else(|| "Never".to_string());
                    setting_row(ui, "Last checked", None, false, None, colors, |ui| {
                        Typography::subtitle(ui, &last_check_str);
                    });

                    // Next check (only meaningful when auto-check is enabled and we have a last_check)
                    if s.auto_check {
                        let next_check_str = props
                            .last_check
                            .map(|t| {
                                let next =
                                    t + chrono::Duration::hours(s.check_interval_hours as i64);
                                let local: chrono::DateTime<chrono::Local> = next.into();
                                local.format("%b %d, %Y %H:%M").to_string()
                            })
                            .unwrap_or_else(|| "Soon".to_string());
                        setting_row(ui, "Next check", None, false, None, colors, |ui| {
                            Typography::subtitle(ui, &next_check_str);
                        });
                    }

                    // Update state row
                    match props.update_state {
                        Some(UpdateState::UpdateAvailable { latest_version, .. }) => {
                            setting_row(
                                ui,
                                "New version available",
                                Some(latest_version.as_str()),
                                false,
                                None,
                                colors,
                                |ui| {
                                    if ui
                                        .add(
                                            Button::builder()
                                                .label("Download")
                                                .button_type(ButtonType::Elevated)
                                                .color(ButtonColor::Success)
                                                .build(),
                                        )
                                        .clicked()
                                    {
                                        events.push(UpdatesTabEvent::DownloadUpdate);
                                    }
                                },
                            );
                        }

                        Some(UpdateState::Downloading { version, progress }) => {
                            setting_row(
                                ui,
                                &format!("Downloading {}…", version),
                                None,
                                false,
                                None,
                                colors,
                                |ui| {
                                    ui.add(egui::ProgressBar::new(*progress).desired_width(120.0));
                                },
                            );
                        }

                        Some(UpdateState::ReadyToInstall { version, .. }) => {
                            setting_row(
                                ui,
                                &format!("Version {} ready to install", version),
                                Some("Thoth will restart after installing."),
                                false,
                                None,
                                colors,
                                |ui| {
                                    if ui
                                        .add(
                                            Button::builder()
                                                .label("Install & Restart")
                                                .button_type(ButtonType::Elevated)
                                                .color(ButtonColor::Primary)
                                                .build(),
                                        )
                                        .clicked()
                                    {
                                        events.push(UpdatesTabEvent::InstallUpdate);
                                    }
                                },
                            );
                        }

                        Some(UpdateState::Error(err)) => {
                            let err_str = err.to_string();
                            setting_row(
                                ui,
                                "Last check failed",
                                Some(&err_str),
                                false,
                                None,
                                colors,
                                |ui| {
                                    if ui
                                        .add(
                                            Button::builder()
                                                .label("Retry")
                                                .button_type(ButtonType::Elevated)
                                                .color(ButtonColor::Default)
                                                .build(),
                                        )
                                        .clicked()
                                    {
                                        events.push(UpdatesTabEvent::CheckForUpdates);
                                    }
                                },
                            );
                        }

                        state => {
                            let checking = matches!(
                                state,
                                Some(UpdateState::Checking) | Some(UpdateState::Installing)
                            );
                            let hint = if matches!(state, Some(UpdateState::Installing)) {
                                Some("Installing…")
                            } else if checking {
                                Some("Checking…")
                            } else {
                                None
                            };
                            setting_row(ui, "Last check", hint, false, None, colors, |ui| {
                                if checking {
                                    ui.add(egui::Spinner::new().size(14.0).color(colors.info));
                                } else {
                                    if ui
                                        .add(
                                            Button::builder()
                                                .label("Check now")
                                                .button_type(ButtonType::Elevated)
                                                .color(ButtonColor::Default)
                                                .build(),
                                        )
                                        .clicked()
                                    {
                                        events.push(UpdatesTabEvent::CheckForUpdates);
                                    }
                                }
                            });
                        }
                    }
                });
            });

        UpdatesTabOutput { events }
    }
}
