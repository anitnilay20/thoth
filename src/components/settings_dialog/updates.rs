use chrono::{DateTime, Utc};
use eframe::egui::{self, RichText};

use crate::components::button::{Button, ButtonColor, ButtonProps, ButtonType};
use crate::components::common::toggle_switch::{
    ToggleSwitch, ToggleSwitchEvent, ToggleSwitchProps,
};
use crate::components::settings_dialog::helpers::{group_rows, section_header, setting_row};
use crate::components::traits::StatelessComponent;
use crate::settings::UpdateSettings;
use crate::theme::ThemeColors;
use crate::update::UpdateState;

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
                group_rows(ui, "AUTO-UPDATE", "updates-auto", colors, |ui| {
                    setting_row(
                        ui,
                        "Automatically check for updates",
                        Some("Check for new versions periodically in the background."),
                        s.auto_check != def.auto_check,
                        None,
                        colors,
                        |ui| {
                            let out = ToggleSwitch::render(
                                ui,
                                ToggleSwitchProps {
                                    enabled: s.auto_check,
                                    hover_text: None,
                                },
                            );
                            for evt in out.events {
                                let ToggleSwitchEvent::Toggled(v) = evt;
                                events.push(UpdatesTabEvent::AutoCheckChanged(v));
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
                            let mut val = s.check_interval_hours;
                            if ui
                                .add(
                                    egui::Slider::new(&mut val, 1..=168)
                                        .suffix(" h")
                                        .clamping(egui::SliderClamping::Always),
                                )
                                .changed()
                            {
                                events.push(UpdatesTabEvent::CheckIntervalChanged(val));
                            }
                        },
                    );
                });

                // ── Status ────────────────────────────────────────────────────────
                group_rows(ui, "STATUS", "updates-status", colors, |ui| {
                    // Current version
                    setting_row(ui, "Current version", None, false, None, colors, |ui| {
                        ui.label(
                            RichText::new(props.current_version)
                                .size(13.0)
                                .color(colors.fg_muted),
                        );
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
                        ui.label(
                            RichText::new(&last_check_str)
                                .size(13.0)
                                .color(colors.fg_muted),
                        );
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
                            ui.label(
                                RichText::new(&next_check_str)
                                    .size(13.0)
                                    .color(colors.fg_muted),
                            );
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
                                    if Button::render(
                                        ui,
                                        ButtonProps {
                                            label: "Download".to_string(),
                                            button_type: ButtonType::Elevated,
                                            color: ButtonColor::Success,
                                            size: Some(13.0),
                                            ..Default::default()
                                        },
                                    )
                                    .clicked
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
                                    if Button::render(
                                        ui,
                                        ButtonProps {
                                            label: "Install & Restart".to_string(),
                                            button_type: ButtonType::Elevated,
                                            color: ButtonColor::Primary,
                                            size: Some(13.0),
                                            ..Default::default()
                                        },
                                    )
                                    .clicked
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
                                    if Button::render(
                                        ui,
                                        ButtonProps {
                                            label: "Retry".to_string(),
                                            button_type: ButtonType::Elevated,
                                            color: ButtonColor::Default,
                                            size: Some(13.0),
                                            ..Default::default()
                                        },
                                    )
                                    .clicked
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
                                    if Button::render(
                                        ui,
                                        ButtonProps {
                                            label: "Check now".to_string(),
                                            button_type: ButtonType::Elevated,
                                            color: ButtonColor::Default,
                                            size: Some(13.0),
                                            ..Default::default()
                                        },
                                    )
                                    .clicked
                                    {
                                        events.push(UpdatesTabEvent::CheckForUpdates);
                                    }
                                }
                            });
                        }
                    }
                });

                ui.add_space(24.0);
            });

        UpdatesTabOutput { events }
    }
}
