use eframe::egui::{self, RichText};

use crate::components::button::{Button, ButtonColor, ButtonProps, ButtonType};
#[cfg(feature = "profiling")]
use crate::components::common::toggle_switch::{
    ToggleSwitch, ToggleSwitchEvent, ToggleSwitchProps,
};
use crate::components::settings_dialog::helpers::{group_rows, section_header, setting_row};
use crate::components::traits::StatelessComponent;
use crate::settings::DeveloperSettings;
use crate::theme::ThemeColors;

pub struct AdvancedTab;

pub struct AdvancedTabProps<'a> {
    pub dev_settings: &'a DeveloperSettings,
    pub theme_colors: &'a ThemeColors,
    pub is_in_path: bool,
}

#[derive(Debug, Clone)]
pub enum AdvancedTabEvent {
    ShowProfilerChanged(bool),
    RegisterInPath,
    UnregisterFromPath,
}

pub struct AdvancedTabOutput {
    pub events: Vec<AdvancedTabEvent>,
}

impl StatelessComponent for AdvancedTab {
    type Props<'a> = AdvancedTabProps<'a>;
    type Output = AdvancedTabOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();
        let colors = props.theme_colors;

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                section_header(
                    ui,
                    egui_phosphor::regular::WRENCH,
                    "Developer",
                    "Profiler and configuration file.",
                    colors,
                );

                // ── Profiler ─────────────────────────────────────────────────────
                group_rows(ui, "PROFILER", "dev-profiler", colors, |ui| {
                    #[cfg(feature = "profiling")]
                    setting_row(
                        ui,
                        "Show profiler",
                        Some("Display puffin performance profiling overlay."),
                        false,
                        None,
                        colors,
                        |ui| {
                            let out = ToggleSwitch::render(
                                ui,
                                ToggleSwitchProps {
                                    enabled: props.dev_settings.show_profiler,
                                    hover_text: None,
                                },
                            );
                            for evt in out.events {
                                let ToggleSwitchEvent::Toggled(v) = evt;
                                events.push(AdvancedTabEvent::ShowProfilerChanged(v));
                            }
                        },
                    );

                    #[cfg(not(feature = "profiling"))]
                    setting_row(
                        ui,
                        "Profiling",
                        Some("Build with --features profiling to enable developer options."),
                        false,
                        None,
                        colors,
                        |ui| {
                            ui.label(RichText::new("Disabled").size(12.0).color(colors.fg_muted));
                        },
                    );
                });

                // ── Config file ──────────────────────────────────────────────────
                group_rows(ui, "CONFIGURATION FILE", "dev-config", colors, |ui| {
                    let path_str = crate::settings::Settings::settings_file_path()
                        .map(|p| p.to_string_lossy().to_string())
                        .unwrap_or_else(|_| "—".to_string());

                    setting_row(
                        ui,
                        "Settings file",
                        Some(&path_str),
                        false,
                        None,
                        colors,
                        |ui| {
                            if Button::render(
                                ui,
                                ButtonProps {
                                    label: "Open".to_string(),
                                    button_type: ButtonType::Elevated,
                                    color: ButtonColor::Default,
                                    size: Some(12.0),
                                    ..Default::default()
                                },
                            )
                            .clicked
                                && let Ok(path) = crate::settings::Settings::settings_file_path()
                            {
                                let _ = open::that(path);
                            }
                        },
                    );
                });

                // ── System integration ───────────────────────────────────────────
                group_rows(ui, "SYSTEM INTEGRATION", "dev-path", colors, |ui| {
                    let (status_text, status_color) = if props.is_in_path {
                        (
                            format!("{} Available in PATH", egui_phosphor::regular::CHECK_CIRCLE),
                            colors.success,
                        )
                    } else {
                        (
                            format!("{} Not in PATH", egui_phosphor::regular::X_CIRCLE),
                            colors.fg_muted,
                        )
                    };

                    setting_row(
                        ui,
                        "Command-line access",
                        Some("Use the `thoth` command from any terminal."),
                        false,
                        None,
                        colors,
                        |ui| {
                            if props.is_in_path {
                                if Button::render(
                                    ui,
                                    ButtonProps {
                                        label: "Remove from PATH".to_string(),
                                        button_type: ButtonType::Elevated,
                                        color: ButtonColor::Danger,
                                        size: Some(12.0),
                                        ..Default::default()
                                    },
                                )
                                .clicked
                                {
                                    events.push(AdvancedTabEvent::UnregisterFromPath);
                                }
                            } else {
                                if Button::render(
                                    ui,
                                    ButtonProps {
                                        label: "Add to PATH".to_string(),
                                        button_type: ButtonType::Elevated,
                                        color: ButtonColor::Success,
                                        size: Some(12.0),
                                        ..Default::default()
                                    },
                                )
                                .clicked
                                {
                                    events.push(AdvancedTabEvent::RegisterInPath);
                                }
                            }
                            ui.add_space(8.0);
                            ui.label(RichText::new(&status_text).size(12.0).color(status_color));
                        },
                    );
                });

                ui.add_space(24.0);
            });

        AdvancedTabOutput { events }
    }
}
