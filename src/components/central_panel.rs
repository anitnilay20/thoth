use crate::components::file_viewer::FileViewer;
use crate::file::lazy_loader::FileType;
use crate::search;
use eframe::egui;
use std::path::PathBuf;

#[derive(Default)]
pub struct CentralPanel {
    file_viewer: FileViewer,
    loaded_path: Option<PathBuf>,
    loaded_type: Option<FileType>,
    last_open_err: Option<String>,
    searching: bool,
}

impl CentralPanel {
    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        path: &Option<std::path::PathBuf>,
        file_type: &mut FileType,
        error: &mut Option<String>,
        search_message: Option<search::SearchMessage>,
    ) {
        // Open / close viewer once on change
        match (path, self.loaded_path.as_ref(), self.loaded_type) {
            (Some(new_path), Some(curr_path), Some(curr_ty))
                if curr_path == new_path && curr_ty == *file_type =>
            {
                // no change
            }
            (Some(new_path), _, _) => {
                self.last_open_err = None;
                match self.file_viewer.open(new_path, file_type) {
                    Ok(()) => {
                        self.loaded_path = Some(new_path.clone());
                        self.loaded_type = Some(*file_type);
                        *error = None;
                        // clear any prior search filter on new file
                        self.file_viewer.set_root_filter(None);
                    }
                    Err(e) => {
                        let msg = format!("Failed to open file: {e}");
                        self.last_open_err = Some(msg.clone());
                        *error = Some(msg);
                        self.loaded_path = None;
                        self.loaded_type = None;
                    }
                }
            }
            (None, Some(_), _) => {
                self.file_viewer = FileViewer::new();
                self.loaded_path = None;
                self.loaded_type = None;
                self.last_open_err = None;
            }
            (None, None, _) => { /* nothing selected */ }
        }

        // React to search messages
        if let Some(msg) = search_message {
            self.searching = msg.is_searching();

            match msg {
                search::SearchMessage::StartSearch(s) => {
                    // Apply the filter to the viewer using returned indices.
                    // (Ignore `scanning` flag here; you can send multiple StartSearch as results accumulate.)
                    if s.results.is_empty() {
                        // show "no matches" by filtering to empty set
                        self.file_viewer.set_root_filter(Some(Vec::new()));
                    } else {
                        self.file_viewer.set_root_filter(Some(s.results.clone()));
                    }
                }
                search::SearchMessage::StopSearch => {
                    // Clear filter; show all rows
                    self.file_viewer.set_root_filter(None);
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // Show any error (either from TopBar or open attempt)
            if let Some(err) = error.as_ref().or(self.last_open_err.as_ref()) {
                ui.colored_label(egui::Color32::RED, err);
                ui.separator();
            }

            if self.searching {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(16.0));
                    ui.label("Searching…");
                });
                ui.add_space(6.0);
                return;
            }

            if self.loaded_path.is_none() {
                ui.label("Open a JSON/NDJSON file from the top bar to begin.");
                return;
            }

            if let Some(count) = self.file_viewer.current_filter_len() {
                ui.horizontal(|ui| {
                    ui.label(format!("Filtered to {} record(s)", count));
                    if ui.button("Clear filter").clicked() {
                        self.file_viewer.set_root_filter(None);
                    }
                });
                ui.separator();
            }

            // Render the viewer
            self.file_viewer.ui(ui);
        });
    }
}
