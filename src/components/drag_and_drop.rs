// File drop overlay — design `screens.html` §6 “File drop overlay”: a black-alpha
// scrim over the whole window carrying the prompt and the incoming path.

use crate::{app, file::detect_file_type::sniff_file_type};
use eframe::egui::{
    self,
    text::{LayoutJob, TextFormat},
};
use thoth_plugin_sdk::theme::with_alpha;

/// `.drop-scrim{background:rgba(0,0,0,.7)}` — 70% of 255. Not a palette colour:
/// the scrim is a neutral dim over whatever the window was showing, so it stays
/// black in every theme.
const SCRIM_ALPHA: u8 = 179;
/// `.drop-scrim .msg{font-size:20px;font-weight:600}`.
const MSG_FONT: f32 = 20.0;
/// `.drop-scrim .msg{line-height:1.9}` — 1.9 × 20px.
const MSG_LINE_H: f32 = 38.0;
/// `.drop-scrim .path{font-family:var(--mono);font-size:14px;font-weight:400}`.
const PATH_FONT: f32 = 14.0;
/// `.path{color:#e9e9f4}` — near-white over the scrim, expressed as an alpha of
/// white so it holds in any theme.
const PATH_ALPHA: u8 = 235;

impl app::ThothApp {
    pub fn handle_file_drop(&mut self, ctx: &egui::Context) {
        let hovering_files = ctx.input(|i| i.raw.hovered_files.clone());
        if !hovering_files.is_empty() {
            // One `.path` line per incoming file: its path, or its MIME type when
            // the platform hands us no path.
            let mut paths: Vec<String> = Vec::new();
            for file in &hovering_files {
                let mut line = String::new();
                use std::fmt::Write as _;
                let written = if let Some(path) = &file.path {
                    Some(("file path", write!(line, "{}", path.display())))
                } else if !file.mime.is_empty() {
                    Some(("MIME type", write!(line, "{}", file.mime)))
                } else {
                    None
                };
                match written {
                    Some((what, Err(e))) => {
                        if let Some(tab) = self.window_state.tab_manager.active_tab_mut() {
                            tab.error = Some(crate::error::ThothError::UIRenderError {
                                component: "DragAndDrop".to_string(),
                                reason: format!("Failed to format {what}: {e}"),
                            });
                        }
                    }
                    Some((_, Ok(()))) => paths.push(line),
                    None => {}
                }
            }

            let painter = ctx.layer_painter(egui::LayerId::new(
                egui::Order::Foreground,
                egui::Id::new("file_drop_overlay"),
            ));
            let screen_rect = ctx.content_rect();
            painter.rect_filled(
                screen_rect,
                0.0,
                egui::Color32::from_black_alpha(SCRIM_ALPHA),
            );

            // `.msg` over `.path`: one centred block, the prompt at 20px semibold
            // white and each path under it in mono.
            let mut job = LayoutJob {
                halign: egui::Align::Center,
                ..Default::default()
            };
            job.append(
                "Drop file to open:",
                0.0,
                TextFormat {
                    font_id: egui::FontId::proportional(MSG_FONT),
                    color: egui::Color32::WHITE,
                    line_height: Some(MSG_LINE_H),
                    ..Default::default()
                },
            );
            for path in &paths {
                job.append(
                    &format!("\n{path}"),
                    0.0,
                    TextFormat {
                        font_id: egui::FontId::monospace(PATH_FONT),
                        color: with_alpha(egui::Color32::WHITE, PATH_ALPHA),
                        line_height: Some(MSG_LINE_H),
                        ..Default::default()
                    },
                );
            }
            let galley = painter.layout_job(job);
            painter.galley(
                screen_rect.center() - egui::vec2(0.0, galley.size().y / 2.0),
                galley,
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
