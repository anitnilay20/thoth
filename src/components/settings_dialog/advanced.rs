use eframe::egui;

use crate::components::settings_dialog::helpers::{group_rows, section_header, setting_row};
use crate::components::traits::StatelessComponent;
use crate::settings::DeveloperSettings;
use crate::theme::ThemeColors;
#[cfg(feature = "profiling")]
use thoth_plugin_sdk::components::ToggleSwitch;
use thoth_plugin_sdk::components::{
    Button, ButtonColor, ButtonType, Typography, TypographyVariant,
};

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
                group_rows(ui, "PROFILER", |ui| {
                    #[cfg(feature = "profiling")]
                    setting_row(
                        ui,
                        "Show profiler",
                        Some("Display puffin performance profiling overlay."),
                        false,
                        None,
                        colors,
                        |ui| {
                            let on = props.dev_settings.show_profiler;
                            if ui
                                .add(ToggleSwitch::builder().enabled(on).build())
                                .clicked()
                            {
                                events.push(AdvancedTabEvent::ShowProfilerChanged(!on));
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
                            Typography::body_muted(ui, "Disabled");
                        },
                    );
                });

                // ── Config file ──────────────────────────────────────────────────
                group_rows(ui, "CONFIGURATION FILE", |ui| {
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
                            if ui
                                .add(
                                    Button::builder()
                                        .label("Open")
                                        .button_type(ButtonType::Elevated)
                                        .color(ButtonColor::Default)
                                        .build(),
                                )
                                .clicked()
                                && let Ok(path) = crate::settings::Settings::settings_file_path()
                            {
                                let _ = open::that(path);
                            }
                        },
                    );
                });

                // ── System integration ───────────────────────────────────────────
                group_rows(ui, "SYSTEM INTEGRATION", |ui| {
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
                                if ui
                                    .add(
                                        Button::builder()
                                            .label("Remove from PATH")
                                            .button_type(ButtonType::Elevated)
                                            .color(ButtonColor::Danger)
                                            .build(),
                                    )
                                    .clicked()
                                {
                                    events.push(AdvancedTabEvent::UnregisterFromPath);
                                }
                            } else {
                                if ui
                                    .add(
                                        Button::builder()
                                            .label("Add to PATH")
                                            .button_type(ButtonType::Elevated)
                                            .color(ButtonColor::Success)
                                            .build(),
                                    )
                                    .clicked()
                                {
                                    events.push(AdvancedTabEvent::RegisterInPath);
                                }
                            }
                            ui.add_space(8.0);
                            ui.add(
                                Typography::builder()
                                    .text(&status_text)
                                    .variant(TypographyVariant::Body)
                                    .color(thoth_plugin_sdk::theme::color_to_hex(status_color))
                                    .build(),
                            );
                        },
                    );
                });
            });

        AdvancedTabOutput { events }
    }
}
