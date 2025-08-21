use std::path::{Path, PathBuf};

use eframe::egui;
use rfd::FileDialog;

use crate::{
    file::lazy_loader::FileType,
    search::{Search, SearchMessage},
};

#[derive(Default)]
pub struct TopBar {
    pub previous_file_type: FileType,
    search_query: String,
    match_case: bool,
}

impl TopBar {
    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        file_path: &mut Option<PathBuf>,
        file_type: &mut FileType,
        error: &mut Option<String>,
    ) -> Option<SearchMessage> {
        let mut search_message = None;
        let mut search = Search::default();

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

                ui.separator();
                ui.label("Search:");
                let text_box_response = ui.text_edit_singleline(&mut self.search_query);
                ui.checkbox(&mut self.match_case, "Aa");

                if ui.button("Search").clicked()
                    || (text_box_response.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    search.query = self.search_query.clone();
                    search.match_case = self.match_case;
                    search_message = Some(SearchMessage::StartSearch(search));
                }

                if ui.button("Stop").clicked() {
                    search_message = Some(SearchMessage::StopSearch);
                }

                egui::ComboBox::from_label("Mention File Type")
                    .selected_text(format!("{:?}", file_type))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(file_type, FileType::Json, "JSON");
                        ui.selectable_value(file_type, FileType::Ndjson, "NDJSON");
                    });

                if self.previous_file_type != *file_type {
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

        search_message
    }
}

fn infer_file_type(path: &Path) -> Option<FileType> {
    match path.extension()?.to_str()?.to_lowercase().as_str() {
        "ndjson" => Some(FileType::Ndjson),
        "json" => Some(FileType::Json),
        _ => None,
    }
}
