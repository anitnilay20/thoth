use crate::components::traits::StatelessComponent;
use crate::settings::ViewerSettings;
use crate::theme::ThemeColors;
use eframe::egui;

/// Viewer settings tab component
pub struct ViewerTab;

/// Props for the Viewer tab
pub struct ViewerTabProps<'a> {
    pub viewer_settings: &'a ViewerSettings,
    pub theme_colors: &'a ThemeColors,
}

/// Events emitted by the Viewer tab
#[derive(Debug, Clone)]
pub enum ViewerTabEvent {
    SyntaxHighlightingChanged(bool),
}

/// Output from the Viewer tab
pub struct ViewerTabOutput {
    pub events: Vec<ViewerTabEvent>,
}

impl StatelessComponent for ViewerTab {
    type Props<'a> = ViewerTabProps<'a>;
    type Output = ViewerTabOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                // Add padding to the content
                ui.add_space(24.0);
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    ui.vertical(|ui| {
                        ui.set_max_width(ui.available_width() - 24.0);

                        ui.heading("Viewer");
                        ui.add_space(16.0);

                        // Syntax Highlighting Section
                        ui.label(egui::RichText::new("Display").size(16.0));
                        ui.add_space(8.0);

                        // Syntax highlighting toggle
                        ui.horizontal(|ui| {
                            let mut syntax_highlighting = props.viewer_settings.syntax_highlighting;
                            let checkbox = ui.checkbox(&mut syntax_highlighting, "");

                            if checkbox.changed() {
                                events.push(ViewerTabEvent::SyntaxHighlightingChanged(
                                    syntax_highlighting,
                                ));
                            }

                            if checkbox.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                            ui.label("Enable syntax highlighting");
                        });

                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(
                                "Colorize JSON keys, strings, numbers, and booleans",
                            )
                            .size(12.0)
                            .color(props.theme_colors.overlay1),
                        );

                        ui.add_space(16.0);
                    });
                });
            });

        ViewerTabOutput { events }
    }
}
