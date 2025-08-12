use std::path::{Path, PathBuf};

use eframe::egui;
use rfd::FileDialog;

use crate::file::lazy_loader::FileType;

#[derive(Default)]
pub struct TopBar {
    pub search_query: String,
    pub previous_file_type: FileType,
}

impl TopBar {
    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        file_path: &mut Option<PathBuf>,
        file_type: &mut FileType,
        error: &mut Option<String>,
    ) {
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // Pick file, but don't load it here
                if ui.button("Open File").clicked() {
                    if let Some(path) = FileDialog::new()
                        .add_filter("JSON", &["json", "ndjson"])
                        .pick_file()
                    {
                        *file_type = infer_file_type(&path).unwrap_or(*file_type);
                        *file_path = Some(path);
                        *error = None;
                        self.previous_file_type = *file_type;
                    }
                }

                if ui.button("Clear").clicked() {
                    *file_path = None;
                    *error = None;
                }

                ui.label("Search:");
                ui.text_edit_singleline(&mut self.search_query);

                egui::ComboBox::from_label("Mention File Type")
                    .selected_text(format!("{:?}", file_type))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(file_type, FileType::Json, "JSON");
                        ui.selectable_value(file_type, FileType::Ndjson, "NDJSON");
                    });

                if self.previous_file_type != *file_type {
                    // No actual load here — just remember for later
                    self.previous_file_type = *file_type;
                }

                if let Some(p) = file_path {
                    ui.label(format!(
                        "File: {}",
                        p.file_name()
                            .and_then(|s| s.to_str())
                            .unwrap_or("<unknown>")
                    ));
                } else {
                    ui.label("File: <none>");
                }
            });
        });
    }
}

fn infer_file_type(path: &Path) -> Option<FileType> {
    match path.extension()?.to_str()?.to_lowercase().as_str() {
        "ndjson" => Some(FileType::Ndjson),
        "json" => Some(FileType::Json),
        _ => None,
    }
}
