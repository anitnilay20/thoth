use crate::components::traits::StatefulComponent;
use eframe::egui;

/// Props passed to the RecentFiles component
pub struct RecentFilesProps<'a> {
    pub recent_files: &'a [String],
}

/// Events emitted by the RecentFiles component
#[derive(Debug, Clone)]
pub enum RecentFilesEvent {
    OpenFile(String),
    RemoveFile(String),
    OpenFilePicker,
}

pub struct RecentFilesOutput {
    pub events: Vec<RecentFilesEvent>,
}

/// Recent files list component
#[derive(Default)]
pub struct RecentFiles;

impl StatefulComponent for RecentFiles {
    type Props<'a> = RecentFilesProps<'a>;
    type Output = RecentFilesOutput;

    fn render(&mut self, ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        let mut events = Vec::new();

        // Guard against rendering during animation when width is too small
        if ui.available_width() < 50.0 {
            return RecentFilesOutput { events };
        }

        // Get theme colors from context
        let (hover_bg, text_color, header_color) = ui.ctx().memory(|mem| {
            if let Some(colors) = mem
                .data
                .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
            {
                (colors.sidebar_hover, colors.text, colors.sidebar_header)
            } else {
                // Fallback colors
                (
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 13),
                    egui::Color32::from_rgb(204, 204, 204),
                    egui::Color32::from_rgb(153, 153, 153),
                )
            }
        });

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
                        Self::render_file_item(ui, file_path, &mut events, hover_bg, text_color);
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
                    events.push(RecentFilesEvent::OpenFilePicker);
                }
            });

        RecentFilesOutput { events }
    }
}

impl RecentFiles {
    fn render_file_item(
        ui: &mut egui::Ui,
        file_path: &str,
        events: &mut Vec<RecentFilesEvent>,
        hover_bg: egui::Color32,
        text_color: egui::Color32,
    ) {
        // Extract just the filename from the path
        let filename = std::path::Path::new(file_path)
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(file_path);

        let available_width = ui.available_width() - 8.0; // Account for margins

        // Guard against negative width during animation
        if available_width <= 0.0 {
            return;
        }

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
            events.push(RecentFilesEvent::OpenFile(file_path.to_string()));
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
                events.push(RecentFilesEvent::RemoveFile(file_path.to_string()));
            }
        }

        response.on_hover_text(file_path);
    }
}
