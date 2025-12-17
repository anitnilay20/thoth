use crate::settings::Settings;
use crate::theme::ThemeColors;
use eframe::egui;

/// Updates settings tab component
pub struct UpdatesTab;

impl UpdatesTab {
    pub fn render(ui: &mut egui::Ui, _settings: &mut Settings, _theme_colors: &ThemeColors) {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.heading("Updates");
                ui.add_space(16.0);
                ui.label("Update settings will go here");
            });
    }
}
