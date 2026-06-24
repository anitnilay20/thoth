use eframe::egui;

use crate::components::settings_dialog::helpers::{group_rows, section_header, setting_row};
use crate::components::traits::StatelessComponent;
use crate::settings::UiSettings;
use crate::theme::ThemeColors;
use thoth_plugin_sdk::components::ToggleSwitch;

#[derive(Debug, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum InterfaceTabEvent {
    SidebarWidthChanged(f32),
    RememberSidebarStateChanged(bool),
    ShowToolbarChanged(bool),
    ShowStatusBarChanged(bool),
    EnableAnimationsChanged(bool),
}

pub struct InterfaceTabOutput {
    pub events: Vec<InterfaceTabEvent>,
}

pub struct InterfaceTabProps<'a> {
    pub ui_settings: &'a UiSettings,
    pub baseline: &'a UiSettings,
    pub theme_colors: &'a ThemeColors,
}

pub struct InterfaceTab;

impl StatelessComponent for InterfaceTab {
    type Props<'a> = InterfaceTabProps<'a>;
    type Output = InterfaceTabOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();
        let s = props.ui_settings;
        let b = props.baseline;
        let colors = props.theme_colors;

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                section_header(
                    ui,
                    egui_phosphor::regular::SIDEBAR,
                    "Interface",
                    "Sidebar, toolbar, status bar and animation preferences.",
                    colors,
                );

                // ── Sidebar ──────────────────────────────────────────────────
                group_rows(ui, "SIDEBAR", "interface-sidebar", colors, |ui| {
                    setting_row(
                        ui,
                        "Sidebar width",
                        Some("Default sidebar width in pixels. Range: 200–1000."),
                        s.sidebar_width != b.sidebar_width,
                        None,
                        colors,
                        |ui| {
                            let mut val = s.sidebar_width as i32;
                            if ui
                                .add(
                                    egui::Slider::new(&mut val, 200..=1000)
                                        .suffix(" px")
                                        .clamping(egui::SliderClamping::Always),
                                )
                                .changed()
                            {
                                events.push(InterfaceTabEvent::SidebarWidthChanged(val as f32));
                            }
                        },
                    );

                    setting_row(
                        ui,
                        "Remember sidebar state",
                        Some("Restore sidebar open/closed state between sessions."),
                        s.remember_sidebar_state != b.remember_sidebar_state,
                        None,
                        colors,
                        |ui| {
                            let on = s.remember_sidebar_state;
                            if ui
                                .add(ToggleSwitch::builder().enabled(on).build())
                                .clicked()
                            {
                                events.push(InterfaceTabEvent::RememberSidebarStateChanged(!on));
                            }
                        },
                    );
                });

                // ── Chrome ───────────────────────────────────────────────────
                group_rows(ui, "CHROME", "interface-chrome", colors, |ui| {
                    setting_row(
                        ui,
                        "Show toolbar",
                        Some("Top toolbar with file actions and search."),
                        s.show_toolbar != b.show_toolbar,
                        None,
                        colors,
                        |ui| {
                            let on = s.show_toolbar;
                            if ui
                                .add(ToggleSwitch::builder().enabled(on).build())
                                .clicked()
                            {
                                events.push(InterfaceTabEvent::ShowToolbarChanged(!on));
                            }
                        },
                    );

                    setting_row(
                        ui,
                        "Show status bar",
                        Some("Bottom bar showing row count, search status and theme."),
                        s.show_status_bar != b.show_status_bar,
                        None,
                        colors,
                        |ui| {
                            let on = s.show_status_bar;
                            if ui
                                .add(ToggleSwitch::builder().enabled(on).build())
                                .clicked()
                            {
                                events.push(InterfaceTabEvent::ShowStatusBarChanged(!on));
                            }
                        },
                    );
                });

                // ── Motion ───────────────────────────────────────────────────
                group_rows(ui, "MOTION", "interface-motion", colors, |ui| {
                    setting_row(
                        ui,
                        "Enable animations",
                        Some("Smooth transitions for collapsibles and panels."),
                        s.enable_animations != b.enable_animations,
                        None,
                        colors,
                        |ui| {
                            let on = s.enable_animations;
                            if ui
                                .add(ToggleSwitch::builder().enabled(on).build())
                                .clicked()
                            {
                                events.push(InterfaceTabEvent::EnableAnimationsChanged(!on));
                            }
                        },
                    );
                });

                ui.add_space(24.0);
            });

        InterfaceTabOutput { events }
    }
}
