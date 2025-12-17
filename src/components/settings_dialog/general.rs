use crate::settings::Settings;
use crate::theme::ThemeColors;
use eframe::egui;

/// General settings tab component
pub struct GeneralTab;

impl GeneralTab {
    pub fn render(ui: &mut egui::Ui, _settings: &mut Settings, _theme_colors: &ThemeColors) {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.heading("General");
                ui.add_space(16.0);
                ui.label("General settings will go here");
            });
    }
}
