use crate::settings::Settings;
use crate::theme::ThemeColors;
use eframe::egui;

/// Appearance settings tab component
pub struct AppearanceTab;

impl AppearanceTab {
    pub fn render(ui: &mut egui::Ui, settings: &mut Settings, theme_colors: &ThemeColors) {
        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                // Add padding to the content
                ui.add_space(24.0);
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    ui.vertical(|ui| {
                        ui.set_max_width(ui.available_width() - 24.0);

                        ui.heading("Theme");
                        ui.add_space(20.0);

                        // Dark Mode toggle
                        ui.horizontal(|ui| {
                            let mut dark_mode = settings.dark_mode;

                            // Checkbox first
                            let response = ui.checkbox(&mut dark_mode, "");
                            if response.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }

                            // Label
                            ui.label(egui::RichText::new("Dark Mode").size(14.0));

                            settings.dark_mode = dark_mode;
                        });

                        ui.add_space(16.0);

                        // Horizontal divider
                        ui.painter().hline(
                            ui.available_rect_before_wrap().x_range(),
                            ui.cursor().min.y,
                            egui::Stroke::new(1.0, theme_colors.surface0),
                        );
                        ui.add_space(16.0);

                        // Font Section
                        ui.heading("Font");
                        ui.add_space(20.0);

                        // Font Size slider
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Font Size").size(14.0));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(egui::RichText::new("pt").size(13.0));
                                    ui.add_space(8.0);
                                    ui.add_sized(
                                        [60.0, 24.0],
                                        egui::Label::new(
                                            egui::RichText::new(format!(
                                                "{:.0}",
                                                settings.font_size
                                            ))
                                            .size(13.0),
                                        ),
                                    );
                                    ui.add_space(8.0);
                                    let slider_width = (ui.available_width() - 16.0).max(100.0);
                                    ui.add_sized(
                                        [slider_width, 20.0],
                                        egui::Slider::new(&mut settings.font_size, 8.0..=32.0)
                                            .show_value(false),
                                    );
                                },
                            );
                        });

                        ui.add_space(16.0);

                        // Horizontal divider
                        ui.painter().hline(
                            ui.available_rect_before_wrap().x_range(),
                            ui.cursor().min.y,
                            egui::Stroke::new(1.0, theme_colors.surface0),
                        );
                        ui.add_space(16.0);

                        // UI Elements Section
                        ui.heading("UI Elements");
                        ui.add_space(20.0);

                        // First row: Show Toolbar and Show Status Bar
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(ui.available_width() / 2.0);
                                ui.horizontal(|ui| {
                                    let response = ui.checkbox(&mut settings.ui.show_toolbar, "");
                                    if response.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    ui.label(egui::RichText::new("Show Toolbar").size(14.0));
                                });
                            });

                            ui.vertical(|ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    let response =
                                        ui.checkbox(&mut settings.ui.show_status_bar, "");
                                    if response.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    ui.label(egui::RichText::new("Show Status Bar").size(14.0));
                                });
                            });
                        });

                        ui.add_space(16.0);

                        // Second row: Enable Animations and Remember Sidebar State
                        ui.horizontal(|ui| {
                            ui.vertical(|ui| {
                                ui.set_width(ui.available_width() / 2.0);
                                ui.horizontal(|ui| {
                                    let response =
                                        ui.checkbox(&mut settings.ui.enable_animations, "");
                                    if response.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    ui.label(egui::RichText::new("Enable Animations").size(14.0));
                                });
                            });

                            ui.vertical(|ui| {
                                ui.set_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    let response =
                                        ui.checkbox(&mut settings.ui.remember_sidebar_state, "");
                                    if response.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                    ui.label(
                                        egui::RichText::new("Remember Sidebar State").size(14.0),
                                    );
                                });
                            });
                        });

                        ui.add_space(20.0);

                        // Sidebar Width slider
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Sidebar Width").size(14.0));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(egui::RichText::new("px").size(13.0));
                                    ui.add_space(8.0);
                                    ui.add_sized(
                                        [60.0, 24.0],
                                        egui::Label::new(
                                            egui::RichText::new(format!(
                                                "{:.0}",
                                                settings.ui.sidebar_width
                                            ))
                                            .size(13.0),
                                        ),
                                    );
                                    ui.add_space(8.0);
                                    let slider_width = (ui.available_width() - 16.0).max(100.0);
                                    ui.add_sized(
                                        [slider_width, 20.0],
                                        egui::Slider::new(
                                            &mut settings.ui.sidebar_width,
                                            200.0..=600.0,
                                        )
                                        .show_value(false),
                                    );
                                },
                            );
                        });

                        ui.add_space(16.0);

                        // Horizontal divider
                        ui.painter().hline(
                            ui.available_rect_before_wrap().x_range(),
                            ui.cursor().min.y,
                            egui::Stroke::new(1.0, theme_colors.surface0),
                        );
                        ui.add_space(16.0);

                        // Font Family (placeholder)
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Font Family").size(14.0));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let dropdown_width =
                                        (ui.available_width() - 16.0).clamp(200.0, 300.0);
                                    let btn = ui.add_sized(
                                        [dropdown_width, 32.0],
                                        egui::Button::new(
                                            egui::RichText::new(format!(
                                                "Fira Code  {}",
                                                egui_phosphor::regular::CARET_DOWN
                                            ))
                                            .size(13.0),
                                        )
                                        .frame(true),
                                    );
                                    if btn.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                },
                            );
                        });

                        ui.add_space(16.0);

                        // Icon Theme (placeholder)
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new("Icon Theme").size(14.0));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    let dropdown_width =
                                        (ui.available_width() - 16.0).clamp(200.0, 300.0);
                                    let btn = ui.add_sized(
                                        [dropdown_width, 32.0],
                                        egui::Button::new(
                                            egui::RichText::new(format!(
                                                "Material Icons  {}",
                                                egui_phosphor::regular::CARET_DOWN
                                            ))
                                            .size(13.0),
                                        )
                                        .frame(true),
                                    );
                                    if btn.hovered() {
                                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                                    }
                                },
                            );
                        });
                    });
                });
            });
    }
}
