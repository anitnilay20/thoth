use crate::settings::Settings;
use crate::theme::ThemeColors;
use eframe::egui;

/// Advanced settings tab component
pub struct AdvancedTab;

impl AdvancedTab {
    pub fn render(ui: &mut egui::Ui, _settings: &mut Settings, _theme_colors: &ThemeColors) {
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
                        ui.label("Advanced settings will go here");
                    });
                });
            });
    }
}
