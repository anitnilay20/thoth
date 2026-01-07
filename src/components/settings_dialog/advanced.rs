use crate::components::traits::StatelessComponent;
use crate::settings::DeveloperSettings;
use crate::theme::ThemeColors;
use eframe::egui;

/// Advanced settings tab component
pub struct AdvancedTab;

/// Props for the Advanced tab
pub struct AdvancedTabProps<'a> {
    #[allow(dead_code)] // Used when profiling feature is enabled
    pub dev_settings: &'a DeveloperSettings,
    pub theme_colors: &'a ThemeColors,
    pub is_in_path: bool,
}

/// Events emitted by the Advanced tab
#[derive(Debug, Clone)]
pub enum AdvancedTabEvent {
    #[allow(dead_code)] // Used when profiling feature is enabled
    ShowProfilerChanged(bool),
    RegisterInPath,
    UnregisterFromPath,
}

/// Output from the Advanced tab
pub struct AdvancedTabOutput {
    pub events: Vec<AdvancedTabEvent>,
}

impl StatelessComponent for AdvancedTab {
    type Props<'a> = AdvancedTabProps<'a>;
    type Output = AdvancedTabOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        #[allow(unused_mut)] // mut needed when profiling feature is enabled
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

                        ui.heading("Advanced");
                        ui.add_space(16.0);

                        // Developer Settings Section
                        ui.label(egui::RichText::new("Developer").size(16.0));
                        ui.add_space(8.0);

                        // Show profiler toggle (only visible when profiling feature is enabled)
                        #[cfg(feature = "profiling")]
                        {
                            ui.horizontal(|ui| {
                                let mut show_profiler = props.dev_settings.show_profiler;
                                let checkbox = ui.checkbox(&mut show_profiler, "");

                                if checkbox.changed() {
                                    events.push(AdvancedTabEvent::ShowProfilerChanged(
                                        show_profiler,
                                    ));
                                }

                                if checkbox.hovered() {
                                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                }
                                ui.label("Show profiler");
                            });

                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new("Display performance profiling information (requires profiling feature)")
                                    .size(12.0)
                                    .color(props.theme_colors.overlay1),
                            );
                        }

                        #[cfg(not(feature = "profiling"))]
                        {
                            ui.label(
                                egui::RichText::new("No developer settings available")
                                    .color(props.theme_colors.overlay1),
                            );
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(
                                    "Build with --features profiling to enable developer options",
                                )
                                .size(12.0)
                                .color(props.theme_colors.overlay1),
                            );
                        }

                        ui.add_space(24.0);

                        // System Integration Section
                        ui.label(egui::RichText::new("System Integration").size(16.0));
                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            ui.label("Command-line access:");
                            if props.is_in_path {
                                ui.label(
                                    egui::RichText::new("✓ Available")
                                        .color(props.theme_colors.success),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new("✗ Not available")
                                        .color(props.theme_colors.overlay1),
                                );
                            }
                        });

                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(
                                "Add Thoth to your system PATH to use the 'thoth' command from any terminal",
                            )
                            .size(12.0)
                            .color(props.theme_colors.overlay1),
                        );

                        ui.add_space(8.0);

                        ui.horizontal(|ui| {
                            if props.is_in_path {
                                let button = egui::Button::new("Remove from PATH")
                                    .fill(props.theme_colors.surface0);
                                if ui.add(button).clicked() {
                                    events.push(AdvancedTabEvent::UnregisterFromPath);
                                }
                            } else {
                                let button = egui::Button::new("Add to PATH")
                                    .fill(props.theme_colors.info);
                                if ui.add(button).clicked() {
                                    events.push(AdvancedTabEvent::RegisterInPath);
                                }
                            }
                        });

                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(
                                if cfg!(target_os = "windows") {
                                    "Note: You may need to restart your terminal for changes to take effect"
                                } else {
                                    "Note: You'll need to restart your terminal or run 'source ~/.zshrc' (or ~/.bashrc)"
                                }
                            )
                            .size(11.0)
                            .italics()
                            .color(props.theme_colors.overlay1),
                        );

                        ui.add_space(16.0);
                    });
                });
            });

        AdvancedTabOutput { events }
    }
}
