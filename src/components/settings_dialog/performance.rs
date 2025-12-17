use crate::settings::Settings;
use crate::theme::ThemeColors;
use eframe::egui;

/// Performance settings tab component
pub struct PerformanceTab;

impl PerformanceTab {
    pub fn render(ui: &mut egui::Ui, _settings: &mut Settings, _theme_colors: &ThemeColors) {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.heading("Performance");
                ui.add_space(16.0);
                ui.label("Performance settings will go here");
            });
    }
}
