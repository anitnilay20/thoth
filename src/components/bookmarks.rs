use crate::app::persistent_state::Bookmark;
use crate::components::traits::StatefulComponent;
use eframe::egui;

/// Props passed to the Bookmarks component
pub struct BookmarksProps<'a> {
    pub bookmarks: &'a [Bookmark],
    pub current_file_path: Option<&'a str>,
}

/// Events emitted by the Bookmarks component
#[derive(Debug, Clone)]
pub enum BookmarksEvent {
    /// User clicked on a bookmark to navigate to it
    NavigateToBookmark { file_path: String, path: String },
    /// User wants to remove a bookmark
    RemoveBookmark(usize),
}

pub struct BookmarksOutput {
    pub events: Vec<BookmarksEvent>,
}

/// Bookmarks list component
#[derive(Default)]
pub struct Bookmarks;

impl StatefulComponent for Bookmarks {
    type Props<'a> = BookmarksProps<'a>;
    type Output = BookmarksOutput;

    fn render(&mut self, ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        let mut events = Vec::new();

        // Guard against rendering during animation when width is too small
        if ui.available_width() < 50.0 {
            return BookmarksOutput { events };
        }

        // Get theme colors from context
        let (hover_bg, text_color, header_color, muted_color) = ui.ctx().memory(|mem| {
            if let Some(colors) = mem
                .data
                .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
            {
                (
                    colors.sidebar_hover,
                    colors.text,
                    colors.sidebar_header,
                    colors.overlay1,
                )
            } else {
                // Fallback colors
                (
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 13),
                    egui::Color32::from_rgb(204, 204, 204),
                    egui::Color32::from_rgb(153, 153, 153),
                    egui::Color32::from_rgb(128, 128, 128),
                )
            }
        });

        // Header
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("BOOKMARKS")
                .size(11.0)
                .color(header_color)
                .strong(),
        );

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        // Bookmarks list
        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                if props.bookmarks.is_empty() {
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("No bookmarks")
                                .size(13.0)
                                .color(muted_color),
                        );
                    });
                    ui.add_space(4.0);
                    ui.horizontal(|ui| {
                        ui.add_space(8.0);
                        ui.label(
                            egui::RichText::new("Press Cmd+D to bookmark")
                                .size(11.0)
                                .color(muted_color),
                        );
                    });
                } else {
                    for (index, bookmark) in props.bookmarks.iter().enumerate() {
                        Self::render_bookmark_item(
                            ui,
                            bookmark,
                            index,
                            props.current_file_path,
                            &mut events,
                            hover_bg,
                            text_color,
                            muted_color,
                        );
                    }
                }

                ui.add_space(8.0);
            });

        BookmarksOutput { events }
    }
}

impl Bookmarks {
    fn render_bookmark_item(
        ui: &mut egui::Ui,
        bookmark: &Bookmark,
        index: usize,
        current_file_path: Option<&str>,
        events: &mut Vec<BookmarksEvent>,
        hover_bg: egui::Color32,
        text_color: egui::Color32,
        muted_color: egui::Color32,
    ) {
        let available_width = ui.available_width() - 8.0;

        // Guard against negative width during animation
        if available_width <= 0.0 {
            return;
        }

        // Calculate height based on whether we show file name
        let show_file_name = current_file_path != Some(&bookmark.file_path);
        let full_height = if show_file_name { 42.0 } else { 28.0 };

        let (rect, response) = ui.allocate_exact_size(
            egui::vec2(available_width, full_height),
            egui::Sense::click(),
        );

        // Background on hover
        if response.hovered() {
            ui.painter().rect_filled(rect, 2.0, hover_bg);
        }

        // Reserve 24px on the right for the remove button
        let _text_width = available_width - 32.0;

        // Display label if exists, otherwise path
        let display_text = bookmark
            .label
            .as_ref()
            .unwrap_or(&bookmark.path)
            .to_string();

        // Main text (path or label)
        let main_text_y = if show_file_name {
            rect.min.y + 6.0
        } else {
            rect.center().y - 6.0
        };

        ui.painter().text(
            egui::pos2(rect.min.x + 8.0, main_text_y),
            egui::Align2::LEFT_TOP,
            &display_text,
            egui::FontId::proportional(13.0),
            text_color,
        );

        // Show file name on second line if different from current file
        if show_file_name {
            let filename = std::path::Path::new(&bookmark.file_path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or(&bookmark.file_path);

            ui.painter().text(
                egui::pos2(rect.min.x + 8.0, main_text_y + 16.0),
                egui::Align2::LEFT_TOP,
                filename,
                egui::FontId::proportional(11.0),
                muted_color,
            );
        }

        if response.clicked() {
            eprintln!(
                "DEBUG: Bookmark clicked - file: {}, path: {}",
                bookmark.file_path, bookmark.path
            );
            events.push(BookmarksEvent::NavigateToBookmark {
                file_path: bookmark.file_path.clone(),
                path: bookmark.path.clone(),
            });
        }

        // Show remove button on hover
        if response.hovered() {
            let remove_button_rect = egui::Rect::from_center_size(
                rect.right_center() - egui::vec2(16.0, 0.0),
                egui::vec2(20.0, 20.0),
            );

            let remove_response = ui.interact(
                remove_button_rect,
                ui.id().with(index),
                egui::Sense::click(),
            );

            if remove_response.hovered() {
                ui.painter().rect_filled(
                    remove_button_rect,
                    2.0,
                    egui::Color32::from_rgba_unmultiplied(255, 255, 255, 25),
                );
            }

            ui.painter().text(
                remove_button_rect.center(),
                egui::Align2::CENTER_CENTER,
                egui_phosphor::regular::X,
                egui::FontId::proportional(12.0),
                text_color,
            );

            if remove_response.clicked() {
                events.push(BookmarksEvent::RemoveBookmark(index));
            }
        }

        // Tooltip showing full info
        let tooltip_text = if let Some(label) = &bookmark.label {
            format!(
                "{}\nPath: {}\nFile: {}",
                label, bookmark.path, bookmark.file_path
            )
        } else {
            format!("Path: {}\nFile: {}", bookmark.path, bookmark.file_path)
        };
        response.on_hover_text(tooltip_text);
    }
}
