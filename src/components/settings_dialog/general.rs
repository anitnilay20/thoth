use crate::components::traits::StatelessComponent;
use crate::settings::{UiSettings, WindowSettings};
use eframe::egui;

/// General settings tab component
pub struct GeneralTab;

/// Props for the General tab
pub struct GeneralTabProps<'a> {
    pub window_settings: &'a WindowSettings,
    pub ui_settings: &'a UiSettings,
}

/// Events emitted by the General tab
#[derive(Debug, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum GeneralTabEvent {
    WindowWidthChanged(f32),
    WindowHeightChanged(f32),
    RememberSidebarStateChanged(bool),
    ShowToolbarChanged(bool),
    ShowStatusBarChanged(bool),
    EnableAnimationsChanged(bool),
    SidebarWidthChanged(f32),
}

/// Output from the General tab
pub struct GeneralTabOutput {
    pub events: Vec<GeneralTabEvent>,
}

impl StatelessComponent for GeneralTab {
    type Props<'a> = GeneralTabProps<'a>;
    type Output = GeneralTabOutput;

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

                        ui.heading("General");
                        ui.add_space(16.0);

                        // Window Settings Section
                        Self::render_window_settings(ui, props.window_settings, &mut events);

                        ui.add_space(16.0);
                        ui.separator();
                        ui.add_space(16.0);

                        // UI Settings Section
                        Self::render_ui_settings(ui, props.ui_settings, &mut events);

                        ui.add_space(16.0);
                    });
                });
            });

        GeneralTabOutput { events }
    }
}

impl GeneralTab {
    fn render_window_settings(
        ui: &mut egui::Ui,
        window_settings: &WindowSettings,
        events: &mut Vec<GeneralTabEvent>,
    ) {
        ui.label(egui::RichText::new("Window").size(16.0));
        ui.add_space(8.0);

        // Default window width
        ui.horizontal(|ui| {
            ui.label("Default window width:");
            ui.add_space(8.0);

            let mut width = window_settings.default_width;
            let slider = egui::Slider::new(&mut width, 800.0..=2560.0)
                .suffix("px")
                .min_decimals(0)
                .max_decimals(0);

            let response = ui.add_sized([ui.available_width().min(300.0), 20.0], slider);

            if response.changed() {
                events.push(GeneralTabEvent::WindowWidthChanged(width));
            }

            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        });

        ui.add_space(8.0);

        // Default window height
        ui.horizontal(|ui| {
            ui.label("Default window height:");
            ui.add_space(8.0);

            let mut height = window_settings.default_height;
            let slider = egui::Slider::new(&mut height, 600.0..=1440.0)
                .suffix("px")
                .min_decimals(0)
                .max_decimals(0);

            let response = ui.add_sized([ui.available_width().min(300.0), 20.0], slider);

            if response.changed() {
                events.push(GeneralTabEvent::WindowHeightChanged(height));
            }

            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        });
    }

    fn render_ui_settings(
        ui: &mut egui::Ui,
        ui_settings: &UiSettings,
        events: &mut Vec<GeneralTabEvent>,
    ) {
        ui.label(egui::RichText::new("User Interface").size(16.0));
        ui.add_space(8.0);

        // Remember sidebar state
        ui.horizontal(|ui| {
            let mut remember_sidebar = ui_settings.remember_sidebar_state;
            let checkbox = ui.checkbox(&mut remember_sidebar, "");

            if checkbox.changed() {
                events.push(GeneralTabEvent::RememberSidebarStateChanged(
                    remember_sidebar,
                ));
            }

            if checkbox.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            ui.label("Remember sidebar state across sessions");
        });

        ui.add_space(8.0);

        // Show toolbar
        ui.horizontal(|ui| {
            let mut show_toolbar = ui_settings.show_toolbar;
            let checkbox = ui.checkbox(&mut show_toolbar, "");

            if checkbox.changed() {
                events.push(GeneralTabEvent::ShowToolbarChanged(show_toolbar));
            }

            if checkbox.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            ui.label("Show toolbar");
        });

        ui.add_space(8.0);

        // Show status bar
        ui.horizontal(|ui| {
            let mut show_status_bar = ui_settings.show_status_bar;
            let checkbox = ui.checkbox(&mut show_status_bar, "");

            if checkbox.changed() {
                events.push(GeneralTabEvent::ShowStatusBarChanged(show_status_bar));
            }

            if checkbox.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
            ui.label("Show status bar");
        });

        ui.add_space(8.0);

        // Enable animations
        ui.horizontal(|ui| {
            let mut enable_animations = ui_settings.enable_animations;
            let checkbox = ui.checkbox(&mut enable_animations, "");

            if checkbox.changed() {
                events.push(GeneralTabEvent::EnableAnimationsChanged(enable_animations));
            }

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

            let mut sidebar_width = ui_settings.sidebar_width;
            let slider = egui::Slider::new(&mut sidebar_width, 200.0..=600.0)
                .suffix("px")
                .min_decimals(0)
                .max_decimals(0);

            let response = ui.add_sized([ui.available_width().min(300.0), 20.0], slider);

            if response.changed() {
                events.push(GeneralTabEvent::SidebarWidthChanged(sidebar_width));
            }

            if response.hovered() {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        });
    }
}
