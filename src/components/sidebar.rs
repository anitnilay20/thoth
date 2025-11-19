use crate::components::traits::ContextComponent;
use eframe::egui;

/// Which sidebar section is currently selected
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SidebarSection {
    RecentFiles,
    Search,
    Settings,
}

/// Props passed to the Sidebar (immutable, one-way binding)
pub struct SidebarProps<'a> {
    pub recent_files: &'a [String],
}

/// Events emitted by the Sidebar
#[derive(Debug, Clone)]
pub enum SidebarEvent {
    OpenFile(String),
    RemoveRecentFile(String),
    OpenFilePicker,
    SectionSelected(SidebarSection),
}

pub struct SidebarOutput {
    pub events: Vec<SidebarEvent>,
}

/// Stateful sidebar component
pub struct Sidebar {
    expanded: bool,
    selected_section: Option<SidebarSection>,
}

impl Default for Sidebar {
    fn default() -> Self {
        Self {
            expanded: false,
            selected_section: Some(SidebarSection::RecentFiles),
        }
    }
}

impl Sidebar {
    /// Render the icon buttons (always visible)
    fn render_icon_buttons(
        &mut self,
        ui: &mut egui::Ui,
        events: &mut Vec<SidebarEvent>,
        hover_bg: egui::Color32,
        text_color: egui::Color32,
    ) {
        let icon_size = 20.0;
        let button_size = egui::vec2(48.0, 48.0);
        let selection_color = ui.visuals().selection.bg_fill;

        // Recent Files button
        let recent_files_selected = self.selected_section == Some(SidebarSection::RecentFiles);
        if self.render_icon_button(
            ui,
            egui_phosphor::regular::FOLDER,
            "Recent Files",
            recent_files_selected,
            (button_size, icon_size),
            (hover_bg, selection_color, text_color),
        ) {
            if self.expanded && recent_files_selected {
                // Clicking the same button collapses
                self.expanded = false;
            } else {
                self.selected_section = Some(SidebarSection::RecentFiles);
                self.expanded = true;
                events.push(SidebarEvent::SectionSelected(SidebarSection::RecentFiles));
            }
        }

        // Search button
        let search_selected = self.selected_section == Some(SidebarSection::Search);
        if self.render_icon_button(
            ui,
            egui_phosphor::regular::MAGNIFYING_GLASS,
            "Search",
            search_selected,
            (button_size, icon_size),
            (hover_bg, selection_color, text_color),
        ) {
            if self.expanded && search_selected {
                // Clicking the same button collapses
                self.expanded = false;
            } else {
                self.selected_section = Some(SidebarSection::Search);
                self.expanded = true;
                events.push(SidebarEvent::SectionSelected(SidebarSection::Search));
            }
        }

        // Settings button
        let settings_selected = self.selected_section == Some(SidebarSection::Settings);
        if self.render_icon_button(
            ui,
            egui_phosphor::regular::GEAR,
            "Settings",
            settings_selected,
            (button_size, icon_size),
            (hover_bg, selection_color, text_color),
        ) {
            if self.expanded && settings_selected {
                // Clicking the same button collapses
                self.expanded = false;
            } else {
                self.selected_section = Some(SidebarSection::Settings);
                self.expanded = true;
                events.push(SidebarEvent::SectionSelected(SidebarSection::Settings));
            }
        }
    }

    /// Render the content area (when expanded)
    fn render_content(
        &mut self,
        ui: &mut egui::Ui,
        props: SidebarProps<'_>,
        events: &mut Vec<SidebarEvent>,
        hover_bg: egui::Color32,
        text_color: egui::Color32,
        header_color: egui::Color32,
    ) {
        // Render content based on selected section
        match self.selected_section {
            Some(SidebarSection::RecentFiles) => {
                self.render_recent_files(ui, props, events, hover_bg, text_color, header_color);
            }
            Some(SidebarSection::Search) => {
                self.render_search_section(ui, header_color, text_color);
            }
            Some(SidebarSection::Settings) => {
                self.render_settings_section(ui, header_color, text_color);
            }
            None => {}
        }
    }
}

impl ContextComponent for Sidebar {
    type Props<'a> = SidebarProps<'a>;
    type Output = SidebarOutput;

    fn render(&mut self, ctx: &egui::Context, props: Self::Props<'_>) -> Self::Output {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        let mut events = Vec::new();

        // Get theme colors
        let theme_colors = ctx.memory(|mem| {
            mem.data
                .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
        });

        let (sidebar_bg, border_color, hover_bg, text_color, header_color) =
            if let Some(colors) = theme_colors {
                (
                    colors.mantle,
                    colors.surface0,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 13), // rgba(255,255,255,0.05)
                    colors.text,
                    egui::Color32::from_rgb(153, 153, 153), // #999999
                )
            } else {
                // Fallback colors
                (
                    egui::Color32::from_rgb(37, 37, 38),
                    egui::Color32::from_rgb(62, 62, 66),
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 13),
                    egui::Color32::from_rgb(204, 204, 204),
                    egui::Color32::from_rgb(153, 153, 153),
                )
            };

        let sidebar_width = if self.expanded { 240.0 } else { 48.0 };

        egui::SidePanel::left("sidebar")
            .resizable(false)
            .exact_width(sidebar_width)
            .frame(
                egui::Frame::NONE
                    .fill(sidebar_bg)
                    .stroke(egui::Stroke::new(1.0, border_color)),
            )
            .show(ctx, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

                if self.expanded {
                    // Horizontal layout: icon buttons on left, content on right
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

                        // Left side: 48px icon buttons
                        ui.vertical(|ui| {
                            ui.set_width(48.0);
                            self.render_icon_buttons(ui, &mut events, hover_bg, text_color);
                        });

                        // Right side: expanded content with padding
                        ui.vertical(|ui| {
                            ui.set_width(192.0); // 240 - 48 = 192

                            // Add frame with inner padding
                            egui::Frame::NONE
                                .inner_margin(egui::Margin::same(8))
                                .show(ui, |ui| {
                                    self.render_content(
                                        ui,
                                        props,
                                        &mut events,
                                        hover_bg,
                                        text_color,
                                        header_color,
                                    );
                                });
                        });
                    });
                } else {
                    // Just show icon buttons
                    self.render_icon_buttons(ui, &mut events, hover_bg, text_color);
                }
            });

        SidebarOutput { events }
    }
}

impl Sidebar {
    fn render_icon_button(
        &self,
        ui: &mut egui::Ui,
        icon: &str,
        tooltip: &str,
        selected: bool,
        (size, icon_size): (egui::Vec2, f32),
        (hover_bg, selection_bg, text_color): (egui::Color32, egui::Color32, egui::Color32),
    ) -> bool {
        let (rect, response) = ui.allocate_exact_size(size, egui::Sense::click());

        if ui.is_rect_visible(rect) {
            // Background
            if selected || response.hovered() {
                let bg_color = if selected {
                    selection_bg // Use theme selection color
                } else {
                    hover_bg
                };
                ui.painter().rect_filled(rect, 0.0, bg_color);
            }

            // Icon (always use text_color)
            let icon_color = text_color;

            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                icon,
                egui::FontId::proportional(icon_size),
                icon_color,
            );
        }

        response.on_hover_text(tooltip).clicked()
    }

    fn render_recent_files(
        &mut self,
        ui: &mut egui::Ui,
        props: SidebarProps<'_>,
        events: &mut Vec<SidebarEvent>,
        hover_bg: egui::Color32,
        text_color: egui::Color32,
        header_color: egui::Color32,
    ) {
        // Header
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("RECENT FILES")
                .size(11.0)
                .color(header_color)
                .strong(),
        );

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        // Recent files list
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if props.recent_files.is_empty() {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("No recent files")
                                .size(13.0)
                                .color(egui::Color32::from_rgb(128, 128, 128)),
                        );
                    });
                } else {
                    for file_path in props.recent_files {
                        self.render_file_item(ui, file_path, events, hover_bg, text_color);
                    }
                }

                ui.add_space(8.0);

                // "Open File..." button
                let button_response = ui.add_sized(
                    egui::vec2(ui.available_width() - 16.0, 28.0),
                    egui::Button::new(
                        egui::RichText::new(format!(
                            "{} Open File...",
                            egui_phosphor::regular::FILE_PLUS
                        ))
                        .size(13.0),
                    ),
                );

                if button_response.clicked() {
                    events.push(SidebarEvent::OpenFilePicker);
                }
            });
    }

    fn render_file_item(
        &self,
        ui: &mut egui::Ui,
        file_path: &str,
        events: &mut Vec<SidebarEvent>,
        hover_bg: egui::Color32,
        text_color: egui::Color32,
    ) {
        // Extract just the filename from the path
        let filename = std::path::Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file_path);

        let available_width = ui.available_width() - 8.0; // Account for margins
        let full_height = 28.0; // Increased height for better spacing

        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(available_width, full_height),
            egui::Sense::click(),
        );

        // Background on hover
        if response.hovered() {
            ui.painter().rect_filled(rect, 2.0, hover_bg);
        }

        // Reserve 24px on the right for the close button
        let text_width = available_width - 32.0; // Leave room for close button + padding

        // File name (truncate if needed)
        let text_rect = egui::Rect::from_min_size(
            rect.min + egui::vec2(8.0, 0.0),
            egui::vec2(text_width, full_height),
        );

        let galley = ui.fonts(|f| {
            f.layout_no_wrap(
                filename.to_string(),
                egui::FontId::proportional(13.0),
                text_color,
            )
        });

        // Truncate text if it overflows
        let text_pos = text_rect.left_center() - egui::vec2(0.0, galley.size().y / 2.0);
        ui.painter().text(
            text_pos,
            egui::Align2::LEFT_TOP,
            filename,
            egui::FontId::proportional(13.0),
            text_color,
        );

        if response.clicked() {
            events.push(SidebarEvent::OpenFile(file_path.to_string()));
        }

        // Show close button on hover
        if response.hovered() {
            let close_button_rect = egui::Rect::from_center_size(
                rect.right_center() - egui::vec2(16.0, 0.0),
                egui::vec2(20.0, 20.0),
            );

            let close_response = ui.interact(
                close_button_rect,
                ui.id().with(file_path),
                egui::Sense::click(),
            );

            if close_response.hovered() {
                ui.painter().rect_filled(
                    close_button_rect,
                    2.0,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 25),
                );
            }

            ui.painter().text(
                close_button_rect.center(),
                egui::Align2::CENTER_CENTER,
                egui_phosphor::regular::X,
                egui::FontId::proportional(12.0),
                text_color,
            );

            if close_response.clicked() {
                events.push(SidebarEvent::RemoveRecentFile(file_path.to_string()));
            }
        }

        response.on_hover_text(file_path);
    }

    fn render_search_section(
        &self,
        ui: &mut egui::Ui,
        header_color: egui::Color32,
        text_color: egui::Color32,
    ) {
        // Header
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("SEARCH")
                .size(11.0)
                .color(header_color)
                .strong(),
        );

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Search functionality is in the toolbar")
                    .size(13.0)
                    .color(text_color),
            );
        });
    }

    fn render_settings_section(
        &self,
        ui: &mut egui::Ui,
        header_color: egui::Color32,
        text_color: egui::Color32,
    ) {
        // Header
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("SETTINGS")
                .size(11.0)
                .color(header_color)
                .strong(),
        );

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(8.0);

        ui.horizontal(|ui| {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new("Settings panel is in the toolbar")
                    .size(13.0)
                    .color(text_color),
            );
        });
    }
}
