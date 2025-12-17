use crate::settings::Settings;
use crate::theme::ThemeColors;
use eframe::egui;

/// General settings tab component
pub struct GeneralTab;

impl GeneralTab {
    pub fn render(ui: &mut egui::Ui, settings: &mut Settings, theme_colors: &ThemeColors) {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                ui.heading("General");
                ui.add_space(16.0);

                // Window Settings Section
                Self::render_window_settings(ui, &mut settings.window, theme_colors);

                ui.add_space(16.0);
                ui.separator();
                ui.add_space(16.0);

                // UI Settings Section
                Self::render_ui_settings(ui, &mut settings.ui, theme_colors);

                ui.add_space(16.0);
            });
    }

    fn render_window_settings(
        ui: &mut egui::Ui,
        window_settings: &mut crate::settings::WindowSettings,
        theme_colors: &ThemeColors,
    ) {
        ui.label(
            egui::RichText::new("Window")
                .size(16.0)
                .color(theme_colors.text),
        );
        ui.add_space(8.0);

        // Default window width
        ui.horizontal(|ui| {
            ui.label("Default window width:");
            ui.add_space(8.0);
            let slider = egui::Slider::new(&mut window_settings.default_width, 800.0..=2560.0)
                .suffix("px")
                .min_decimals(0)
                .max_decimals(0);

            let response = ui.add_sized([ui.available_width().min(300.0), 20.0], slider);

            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        });

        ui.add_space(8.0);

        // Default window height
        ui.horizontal(|ui| {
            ui.label("Default window height:");
            ui.add_space(8.0);
            let slider = egui::Slider::new(&mut window_settings.default_height, 600.0..=1440.0)
                .suffix("px")
                .min_decimals(0)
                .max_decimals(0);

            let response = ui.add_sized([ui.available_width().min(300.0), 20.0], slider);

            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        });
    }

    fn render_ui_settings(
        ui: &mut egui::Ui,
        ui_settings: &mut crate::settings::UiSettings,
        theme_colors: &ThemeColors,
    ) {
        ui.label(
            egui::RichText::new("User Interface")
                .size(16.0)
                .color(theme_colors.text),
        );
        ui.add_space(8.0);

        // Remember sidebar state
        ui.horizontal(|ui| {
            let checkbox = ui.checkbox(&mut ui_settings.remember_sidebar_state, "");
            if checkbox.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            ui.label("Remember sidebar state across sessions");
        });

        ui.add_space(8.0);

        // Show toolbar
        ui.horizontal(|ui| {
            let checkbox = ui.checkbox(&mut ui_settings.show_toolbar, "");
            if checkbox.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            ui.label("Show toolbar");
        });

        ui.add_space(8.0);

        // Show status bar
        ui.horizontal(|ui| {
            let checkbox = ui.checkbox(&mut ui_settings.show_status_bar, "");
            if checkbox.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            ui.label("Show status bar");
        });

        ui.add_space(8.0);

        // Enable animations
        ui.horizontal(|ui| {
            let checkbox = ui.checkbox(&mut ui_settings.enable_animations, "");
            if checkbox.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            ui.label("Enable animations");
        });

        ui.add_space(8.0);

        // Default sidebar width
        ui.horizontal(|ui| {
            ui.label("Default sidebar width:");
            ui.add_space(8.0);
            let slider = egui::Slider::new(&mut ui_settings.sidebar_width, 200.0..=600.0)
                .suffix("px")
                .min_decimals(0)
                .max_decimals(0);

            let response = ui.add_sized([ui.available_width().min(300.0), 20.0], slider);

            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        });
    }
}
