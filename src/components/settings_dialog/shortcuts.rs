use crate::components::traits::StatelessComponent;
use crate::shortcuts::KeyboardShortcuts;
use crate::theme::ThemeColors;
use eframe::egui;

/// Shortcuts settings tab component
pub struct ShortcutsTab;

/// Props for the Shortcuts tab
pub struct ShortcutsTabProps<'a> {
    pub shortcuts: &'a KeyboardShortcuts,
    pub theme_colors: &'a ThemeColors,
}

/// Events emitted by the Shortcuts tab
#[derive(Debug, Clone)]
pub enum ShortcutsTabEvent {
    // No events yet - shortcuts are read-only for now
    // Future: Add shortcut customization events
}

/// Output from the Shortcuts tab
pub struct ShortcutsTabOutput {
    #[allow(dead_code)] // Reserved for future shortcut customization
    pub events: Vec<ShortcutsTabEvent>,
}

impl StatelessComponent for ShortcutsTab {
    type Props<'a> = ShortcutsTabProps<'a>;
    type Output = ShortcutsTabOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let events = Vec::new();

        egui::ScrollArea::vertical()
            .auto_shrink([false; 2])
            .show(ui, |ui| {
                // Add padding to the content
                ui.add_space(24.0);
                ui.horizontal(|ui| {
                    ui.add_space(24.0);
                    ui.vertical(|ui| {
                        ui.set_max_width(ui.available_width() - 24.0);

                        ui.heading("Keyboard Shortcuts");
                        ui.add_space(16.0);

                        // File Operations Section
                        Self::render_section(
                            ui,
                            "File Operations",
                            props.theme_colors,
                            &[
                                ("Open file", &props.shortcuts.open_file),
                                ("Close file", &props.shortcuts.clear_file),
                                ("New window", &props.shortcuts.new_window),
                            ],
                        );

                        ui.add_space(16.0);
                        ui.separator();
                        ui.add_space(16.0);

                        // Navigation Section
                        Self::render_section(
                            ui,
                            "Navigation",
                            props.theme_colors,
                            &[
                                ("Focus search", &props.shortcuts.focus_search),
                                ("Next match", &props.shortcuts.next_match),
                                ("Previous match", &props.shortcuts.prev_match),
                                ("Escape", &props.shortcuts.escape),
                            ],
                        );

                        ui.add_space(16.0);
                        ui.separator();
                        ui.add_space(16.0);

                        // Tree Operations Section
                        Self::render_section(
                            ui,
                            "Tree Operations",
                            props.theme_colors,
                            &[
                                ("Expand node", &props.shortcuts.expand_node),
                                ("Collapse node", &props.shortcuts.collapse_node),
                                ("Expand all", &props.shortcuts.expand_all),
                                ("Collapse all", &props.shortcuts.collapse_all),
                            ],
                        );

                        ui.add_space(16.0);
                        ui.separator();
                        ui.add_space(16.0);

                        // Clipboard Section
                        Self::render_section(
                            ui,
                            "Clipboard",
                            props.theme_colors,
                            &[
                                ("Copy key", &props.shortcuts.copy_key),
                                ("Copy value", &props.shortcuts.copy_value),
                                ("Copy object", &props.shortcuts.copy_object),
                                ("Copy path", &props.shortcuts.copy_path),
                            ],
                        );

                        ui.add_space(16.0);
                        ui.separator();
                        ui.add_space(16.0);

                        // Movement Section
                        Self::render_section(
                            ui,
                            "Movement",
                            props.theme_colors,
                            &[
                                ("Move up", &props.shortcuts.move_up),
                                ("Move down", &props.shortcuts.move_down),
                            ],
                        );

                        ui.add_space(16.0);
                        ui.separator();
                        ui.add_space(16.0);

                        // UI Section
                        Self::render_section(
                            ui,
                            "User Interface",
                            props.theme_colors,
                            &[
                                ("Settings", &props.shortcuts.settings),
                                ("Toggle theme", &props.shortcuts.toggle_theme),
                            ],
                        );

                        ui.add_space(16.0);
                        ui.separator();
                        ui.add_space(16.0);

                        // Developer Section
                        Self::render_section(
                            ui,
                            "Developer",
                            props.theme_colors,
                            &[("Toggle profiler", &props.shortcuts.toggle_profiler)],
                        );

                        ui.add_space(16.0);
                    });
                });
            });

        ShortcutsTabOutput { events }
    }
}

impl ShortcutsTab {
    fn render_section(
        ui: &mut egui::Ui,
        title: &str,
        theme_colors: &ThemeColors,
        shortcuts: &[(&str, &crate::shortcuts::Shortcut)],
    ) {
        ui.label(egui::RichText::new(title).size(16.0));
        ui.add_space(8.0);

        for (label, shortcut) in shortcuts {
            ui.horizontal(|ui| {
                // Label
                ui.label(*label);

                // Spacer
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Shortcut badge - render as regular text so icons work
                    let shortcut_text = shortcut.format();

                    // Create a badge-like background
                    let text = egui::RichText::new(&shortcut_text).size(13.0);

                    let response = ui.add(egui::Label::new(text).sense(egui::Sense::hover()));

                    // Draw background frame
                    let rect = response.rect;
                    let expanded_rect = rect.expand2(egui::vec2(6.0, 3.0));
                    ui.painter()
                        .rect_filled(expanded_rect, 4.0, theme_colors.surface0);

                    // Re-draw the text on top of the background
                    ui.painter().text(
                        rect.center(),
                        egui::Align2::CENTER_CENTER,
                        &shortcut_text,
                        egui::FontId::proportional(13.0),
                        theme_colors.text,
                    );
                });
            });

            ui.add_space(4.0);
        }
    }
}
