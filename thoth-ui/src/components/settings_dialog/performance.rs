use crate::components::settings_dialog::helpers::{
    group_rows, section_header, setting_row, slider_control,
};
use crate::components::traits::StatelessComponent;
use crate::settings::PerformanceSettings;
use crate::theme::ThemeColors;
use eframe::egui;
use thoth_plugin_sdk::components::NumberInput;

pub struct PerformanceTab;

pub struct PerformanceTabProps<'a> {
    pub performance_settings: &'a PerformanceSettings,
    pub theme_colors: &'a ThemeColors,
}

#[derive(Debug, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum PerformanceTabEvent {
    IndexCacheBudgetChanged(usize),
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

                group_rows(ui, "CACHE", |ui| {
                    setting_row(
                        ui,
                        "Index cache",
                        Some(
                            "Disk kept for file indexes, so a large file reopens instantly. \
                             Least recently used are dropped first. Range: 128 MB – 16 GB.",
                        ),
                        s.index_cache_mb != def.index_cache_mb,
                        None,
                        colors,
                        |ui| {
                            if let Some(val) =
                                slider_control(ui, s.index_cache_mb as f64, 128.0, 16384.0, "MB")
                            {
                                let snapped = ((val / 128.0).round() * 128.0).max(128.0);
                                events.push(PerformanceTabEvent::IndexCacheBudgetChanged(
                                    snapped as usize,
                                ));
                            }
                        },
                    );
                });

                group_rows(ui, "FILES & HISTORY", |ui| {
                    setting_row(
                        ui,
                        "Recent files",
                        Some("Maximum number of recent files to remember. Range: 1–100."),
                        s.max_recent_files != def.max_recent_files,
                        None,
                        colors,
                        |ui| {
                            let mut num = NumberInput::builder()
                                .id("perf_max_recent_files")
                                .value(s.max_recent_files as f64)
                                .min(1.0)
                                .max(100.0)
                                .unit("files")
                                .build();
                            if num.show(ui).changed() {
                                events.push(PerformanceTabEvent::MaxRecentFilesChanged(
                                    num.value as usize,
                                ));
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
                            let mut num = NumberInput::builder()
                                .id("perf_navigation_history_size")
                                .value(s.navigation_history_size as f64)
                                .min(1.0)
                                .max(1000.0)
                                .unit("steps")
                                .build();
                            if num.show(ui).changed() {
                                events.push(PerformanceTabEvent::NavigationHistorySizeChanged(
                                    num.value as usize,
                                ));
                            }
                        },
                    );
                });
            });

        PerformanceTabOutput { events }
    }
}
