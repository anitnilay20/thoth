use crate::components::settings_dialog::helpers::{group_rows, section_header, setting_row};
use crate::components::traits::StatelessComponent;
use crate::settings::ViewerSettings;
use crate::theme::ThemeColors;
use eframe::egui;
use thoth_plugin_sdk::components::ToggleSwitch;

pub struct ViewerTab;

pub struct ViewerTabProps<'a> {
    pub viewer_settings: &'a ViewerSettings,
    pub theme_colors: &'a ThemeColors,
}

#[derive(Debug, Clone)]
pub enum ViewerTabEvent {
    SyntaxHighlightingChanged(bool),
}

pub struct ViewerTabOutput {
    pub events: Vec<ViewerTabEvent>,
}

impl StatelessComponent for ViewerTab {
    type Props<'a> = ViewerTabProps<'a>;
    type Output = ViewerTabOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();
        let s = props.viewer_settings;
        let def = ViewerSettings::default();
        let colors = props.theme_colors;

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                section_header(
                    ui,
                    egui_phosphor::regular::EYE,
                    "Viewer",
                    "Syntax highlighting and display.",
                    colors,
                );

                group_rows(ui, "DISPLAY", "viewer-display", colors, |ui| {
                    setting_row(
                        ui,
                        "Syntax highlighting",
                        Some("Colorize JSON keys, strings, numbers and booleans."),
                        s.syntax_highlighting != def.syntax_highlighting,
                        None,
                        colors,
                        |ui| {
                            let on = s.syntax_highlighting;
                            if ui
                                .add(ToggleSwitch::builder().enabled(on).build())
                                .clicked()
                            {
                                events.push(ViewerTabEvent::SyntaxHighlightingChanged(!on));
                            }
                        },
                    );
                });

                ui.add_space(24.0);
            });

        ViewerTabOutput { events }
    }
}
