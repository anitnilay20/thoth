use crate::components::settings_dialog::helpers::{group_rows, section_header, setting_row};
use crate::components::traits::StatelessComponent;
use crate::settings::PerformanceSettings;
use crate::theme::ThemeColors;
use eframe::egui;

pub struct PerformanceTab;

pub struct PerformanceTabProps<'a> {
    pub performance_settings: &'a PerformanceSettings,
    pub theme_colors: &'a ThemeColors,
}

#[derive(Debug, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum PerformanceTabEvent {
    CacheSizeChanged(usize),
    MaxRecentFilesChanged(usize),
    NavigationHistorySizeChanged(usize),
}

pub struct PerformanceTabOutput {
    pub events: Vec<PerformanceTabEvent>,
}

impl StatelessComponent for PerformanceTab {
    type Props<'a> = PerformanceTabProps<'a>;
    type Output = PerformanceTabOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();
        let s = props.performance_settings;
        let def = PerformanceSettings::default();
        let colors = props.theme_colors;

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                section_header(
                    ui,
                    egui_phosphor::regular::GAUGE,
                    "Performance",
                    "Cache, history and recent files.",
                    colors,
                );

                group_rows(ui, "CACHE", "perf-cache", colors, |ui| {
                    setting_row(
                        ui,
                        "Cache size",
                        Some("LRU cache for parsed JSON nodes. Range: 1–10 000."),
                        s.cache_size != def.cache_size,
                        None,
                        colors,
                        |ui| {
                            let mut val = s.cache_size as i32;
                            if ui
                                .add(
                                    egui::Slider::new(&mut val, 1..=10000)
                                        .step_by(50.0)
                                        .suffix(" nodes"),
                                )
                                .changed()
                            {
                                events.push(PerformanceTabEvent::CacheSizeChanged(val as usize));
                            }
                        },
                    );
                });

                group_rows(ui, "FILES & HISTORY", "perf-files", colors, |ui| {
                    setting_row(
                        ui,
                        "Recent files",
                        Some("Maximum number of recent files to remember. Range: 1–100."),
                        s.max_recent_files != def.max_recent_files,
                        None,
                        colors,
                        |ui| {
                            let mut val = s.max_recent_files as i32;
                            if ui
                                .add(egui::DragValue::new(&mut val).range(1..=100))
                                .changed()
                            {
                                events
                                    .push(PerformanceTabEvent::MaxRecentFilesChanged(val as usize));
                            }
                        },
                    );

                    setting_row(
                        ui,
                        "Navigation history",
                        Some("Back/forward history depth. Range: 1–1000 steps."),
                        s.navigation_history_size != def.navigation_history_size,
                        None,
                        colors,
                        |ui| {
                            let mut val = s.navigation_history_size as i32;
                            if ui
                                .add(
                                    egui::DragValue::new(&mut val)
                                        .range(1..=1000)
                                        .suffix(" steps"),
                                )
                                .changed()
                            {
                                events.push(PerformanceTabEvent::NavigationHistorySizeChanged(
                                    val as usize,
                                ));
                            }
                        },
                    );
                });

                ui.add_space(24.0);
            });

        PerformanceTabOutput { events }
    }
}
