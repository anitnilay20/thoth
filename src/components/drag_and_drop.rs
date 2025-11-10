use crate::{
    app,
    file::{detect_file_type::sniff_file_type, lazy_loader::FileType},
};
use eframe::egui::{self};

impl app::ThothApp {
    pub fn handle_file_drop(&mut self, ctx: &egui::Context) {
        // -------- Drag & Drop (hover preview + accept drop) --------
        // Show overlay when hovering files
        let hovering_files = ctx.input(|i| i.raw.hovered_files.clone());
        if !hovering_files.is_empty() {
            let mut text = String::from("Drop file to open:\n");
            for file in &hovering_files {
                if let Some(path) = &file.path {
                    use std::fmt::Write as _;
                    if let Err(e) = write!(text, "\n{}", path.display()) {
                        self.window_state.error = Some(format!("Failed to format file path: {e}"));
                    }
                } else if !file.mime.is_empty() {
                    use std::fmt::Write as _;
                    if let Err(e) = write!(text, "\n{}", file.mime) {
                        self.window_state.error = Some(format!("Failed to format MIME type: {e}"));
                    }
                }
            }

            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("file_drop_overlay"),
            ));
            let screen_rect = ctx.screen_rect();
            painter.rect_filled(screen_rect, 0.0, egui::Color32::from_black_alpha(180));
            painter.text(
                screen_rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::TextStyle::Heading.resolve(&ctx.style()),
                egui::Color32::WHITE,
            );
        }

        // Handle dropped files (take first valid JSON/NDJSON)
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            for file in dropped_files {
                if let Some(path) = file.path {
                    match sniff_file_type(&path) {
                        Ok(detected) => {
                            let ft: FileType = detected.into();
                            self.window_state.file_type = ft;
                            self.window_state.file_path = Some(path);
                            self.window_state.error = None;
                            self.window_state.toolbar.previous_file_type = ft;
                        }
                        Err(e) => {
                            self.window_state.error = Some(format!(
                                "Failed to detect file type (expect JSON / NDJSON): {e}"
                            ));
                        }
                    }
                    break; // only process first dropped file
                }
            }
        }
    }
}
