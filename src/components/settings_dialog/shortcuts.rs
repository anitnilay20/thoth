use crate::settings::Settings;
use crate::theme::ThemeColors;
use eframe::egui;

/// Shortcuts settings tab component
pub struct ShortcutsTab;

impl ShortcutsTab {
    pub fn render(ui: &mut egui::Ui, _settings: &mut Settings, _theme_colors: &ThemeColors) {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.heading("Shortcuts");
                ui.add_space(16.0);
                ui.label("Keyboard shortcuts will go here");
            });
    }
}
