use crate::components::traits::StatelessComponent;
use crate::settings::PerformanceSettings;
use crate::theme::ThemeColors;
use eframe::egui;

/// Performance settings tab component
pub struct PerformanceTab;

/// Props for the Performance tab
pub struct PerformanceTabProps<'a> {
    pub performance_settings: &'a PerformanceSettings,
    pub theme_colors: &'a ThemeColors,
}

/// Events emitted by the Performance tab
#[derive(Debug, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum PerformanceTabEvent {
    CacheSizeChanged(usize),
    MaxRecentFilesChanged(usize),
    NavigationHistorySizeChanged(usize),
}

/// Output from the Performance tab
pub struct PerformanceTabOutput {
    pub events: Vec<PerformanceTabEvent>,
}

impl StatelessComponent for PerformanceTab {
    type Props<'a> = PerformanceTabProps<'a>;
    type Output = PerformanceTabOutput;

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

                        ui.heading("Performance");
                        ui.add_space(16.0);

                        // Cache Settings Section
                        ui.label(egui::RichText::new("Cache").size(16.0));
                        ui.add_space(8.0);

                        // Cache size
                        ui.horizontal(|ui| {
                            ui.label("LRU cache size:");
                            ui.add_space(8.0);

                            let mut cache_size = props.performance_settings.cache_size;
                            let slider = egui::Slider::new(&mut cache_size, 1..=10000)
                                .logarithmic(true)
                                .min_decimals(0)
                                .max_decimals(0);

                            let response =
                                ui.add_sized([ui.available_width().min(300.0), 20.0], slider);

                            if response.changed() {
                                events.push(PerformanceTabEvent::CacheSizeChanged(cache_size));
                            }

                            if response.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                        });

                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(
                                "Higher values use more memory but improve performance",
                            )
                            .size(12.0)
                            .color(props.theme_colors.overlay1),
                        );

                        ui.add_space(16.0);
                        ui.separator();
                        ui.add_space(16.0);

                        // Recent Files Section
                        ui.label(egui::RichText::new("Recent Files").size(16.0));
                        ui.add_space(8.0);

                        // Max recent files
                        ui.horizontal(|ui| {
                            ui.label("Maximum recent files:");
                            ui.add_space(8.0);

                            let mut max_recent = props.performance_settings.max_recent_files;
                            let slider = egui::Slider::new(&mut max_recent, 1..=100)
                                .min_decimals(0)
                                .max_decimals(0);

                            let response =
                                ui.add_sized([ui.available_width().min(300.0), 20.0], slider);

                            if response.changed() {
                                events.push(PerformanceTabEvent::MaxRecentFilesChanged(max_recent));
                            }

                            if response.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                        });

                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new("Number of recent files to remember")
                                .size(12.0)
                                .color(props.theme_colors.overlay1),
                        );

                        ui.add_space(16.0);
                        ui.separator();
                        ui.add_space(16.0);

                        // Navigation History Section
                        ui.label(egui::RichText::new("Navigation").size(16.0));
                        ui.add_space(8.0);

                        // Navigation history size
                        ui.horizontal(|ui| {
                            ui.label("Navigation history size:");
                            ui.add_space(8.0);

                            let mut history_size =
                                props.performance_settings.navigation_history_size;
                            let slider = egui::Slider::new(&mut history_size, 10..=1000)
                                .min_decimals(0)
                                .max_decimals(0);

                            let response =
                                ui.add_sized([ui.available_width().min(300.0), 20.0], slider);

                            if response.changed() {
                                events.push(PerformanceTabEvent::NavigationHistorySizeChanged(
                                    history_size,
                                ));
                            }

                            if response.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }
                        });

                        ui.add_space(4.0);
                        ui.label(
                            egui::RichText::new(
                                "Number of navigation steps to remember for back/forward",
                            )
                            .size(12.0)
                            .color(props.theme_colors.overlay1),
                        );

                        ui.add_space(16.0);
                    });
                });
            });

        PerformanceTabOutput { events }
    }
}
