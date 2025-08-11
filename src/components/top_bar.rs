use std::path::PathBuf;

use eframe::egui;
use rfd::FileDialog;

use crate::{FileType, load_file::load_file};

#[derive(Default)]
pub struct TopBar {
    search_query: String,
    previous_file_type: FileType,
}

impl TopBar {
    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        file_path: &mut Option<PathBuf>,
        file_type: &mut FileType,
        json_lines: &mut Vec<serde_json::Value>,
        error: &mut Option<String>,
    ) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                if ui.button("Open File").clicked() {
                    if let Some(path) = FileDialog::new()
                        .add_filter("JSON", &["json", "ndjson"])
                        .pick_file()
                    {
                        match load_file(&path, file_type) {
                            Ok(lines) => {
                                *file_path = Some(path);
                                *json_lines = lines;
                                *error = None;
                            }
                            Err(e) => {
                                *error = Some(format!("Failed to load file: {e}"));
                            }
                        }
                    }
                }

                if ui.button("Clear").clicked() {
                    *file_path = None;
                    *json_lines = Vec::new();
                    *error = None;
                }

                ui.label("Search:");
                let changed = ui.text_edit_singleline(&mut self.search_query).changed();

                // ui.label(format!(
                //     "File: {}",
                //     file_path
                //         .as_ref()
                //         .map_or("None", |p| p.to_str().unwrap_or("None"))
                // ));

                egui::ComboBox::from_label("Mention File Type")
                    .selected_text(format!("{:?}", file_type))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(file_type, FileType::Json, "JSON");
                        ui.selectable_value(file_type, FileType::Ndjson, "NDJSON");
                    });

                if self.previous_file_type != *file_type {
                    if let Some(path) = file_path.as_ref() {
                        match load_file(path, file_type) {
                            Ok(lines) => {
                                *json_lines = lines;
                                *error = None;
                            }
                            Err(e) => {
                                *error = Some(format!("Failed to load file: {e}"));
                            }
                        }
                    } else {
                        *error = Some("No file selected".to_string());
                    }
                    self.previous_file_type = file_type.clone();
                }
                // if changed {
                //     self.filtered_lines = if self.search_query.is_empty() {
                //         self.json_lines.clone()
                //     } else {
                //         self.json_lines
                //             .iter()
                //             .filter(|line| line.contains(&self.search_query))
                //             .cloned()
                //             .collect()
                //     };
                //     self.scroll_offset = 0;
                // }
            });
        });
    }
}
