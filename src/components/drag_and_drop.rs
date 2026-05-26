use crate::{app, file::detect_file_type::sniff_file_type};
use eframe::egui;

impl app::ThothApp {
    pub fn handle_file_drop(&mut self, ctx: &egui::Context) {
        let hovering_files = ctx.input(|i| i.raw.hovered_files.clone());
        if !hovering_files.is_empty() {
            let mut text = String::from("Drop file to open:\n");
            for file in &hovering_files {
                if let Some(path) = &file.path {
                    use std::fmt::Write as _;
                    if let Err(e) = write!(text, "\n{}", path.display())
                        && let Some(tab) = self.window_state.tab_manager.active_tab_mut()
                    {
                        tab.error = Some(crate::error::ThothError::UIRenderError {
                            component: "DragAndDrop".to_string(),
                            reason: format!("Failed to format file path: {e}"),
                        });
                    }
                } else if !file.mime.is_empty() {
                    use std::fmt::Write as _;
                    if let Err(e) = write!(text, "\n{}", file.mime)
                        && let Some(tab) = self.window_state.tab_manager.active_tab_mut()
                    {
                        tab.error = Some(crate::error::ThothError::UIRenderError {
                            component: "DragAndDrop".to_string(),
                            reason: format!("Failed to format MIME type: {e}"),
                        });
                    }
                }
            }

            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("file_drop_overlay"),
            ));
            let screen_rect = ctx.content_rect();
            painter.rect_filled(screen_rect, 0.0, egui::Color32::from_black_alpha(180));
            painter.text(
                screen_rect.center(),
                egui::Align2::CENTER_CENTER,
                text,
                egui::TextStyle::Heading.resolve(&ctx.global_style()),
                egui::Color32::WHITE,
            );
        }

        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());
        if !dropped_files.is_empty() {
            let nav_capacity = self.settings.performance.navigation_history_size;
            for file in dropped_files {
                if let Some(path) = file.path {
                    match sniff_file_type(&path) {
                        Ok(detected) => {
                            use crate::file::lazy_loader::FileKind;
                            let ft: FileKind = detected.into();
                            let id = self.window_state.tab_manager.open_file(path, nav_capacity);
                            if let Some(tab) = self.window_state.tab_manager.tabs.get_mut(&id) {
                                tab.file_type = ft;
                                tab.error = None;
                                self.window_state.toolbar.previous_file_type = ft;
                            }
                        }
                        Err(_) => {
                            if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                                tab.error = Some(crate::error::ThothError::InvalidFileType {
                                    path: path.clone(),
                                    expected: "JSON or NDJSON".to_string(),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}
