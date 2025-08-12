use crate::components::json_viewer::JsonViewer;
use crate::file::lazy_loader::FileType;
use eframe::egui;
use std::path::PathBuf;

#[derive(Default)]
pub struct CentralPanel {
    json_viewer: JsonViewer,
    loaded_path: Option<PathBuf>,
    loaded_type: Option<FileType>,
    last_open_err: Option<String>,
}

impl CentralPanel {
    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        path: &Option<std::path::PathBuf>,
        file_type: &mut FileType,
        error: &mut Option<String>,
    ) {
        // If the selected path or file type changed, open (or clear) once here.
        match (path, self.loaded_path.as_ref(), self.loaded_type) {
            (Some(new_path), Some(curr_path), Some(curr_ty))
                if curr_path == new_path && curr_ty == *file_type =>
            {
                // No change — do nothing
            }
            (Some(new_path), _, _) => {
                // New file or type — try open once
                self.last_open_err = None;
                match self.json_viewer.open(new_path, file_type) {
                    Ok(()) => {
                        self.loaded_path = Some(new_path.clone());
                        self.loaded_type = Some(*file_type);
                        *error = None;
                    }
                    Err(e) => {
                        let msg = format!("Failed to open file: {e}");
                        self.last_open_err = Some(msg.clone());
                        *error = Some(msg);
                        // Clear loaded markers so we can retry next time user changes input
                        self.loaded_path = None;
                        self.loaded_type = None;
                    }
                }
            }
            (None, Some(_), _) => {
                // Cleared file selection — reset viewer
                self.json_viewer = JsonViewer::new();
                self.loaded_path = None;
                self.loaded_type = None;
                self.last_open_err = None;
            }
            (None, None, _) => { /* nothing selected */ }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // Show any error (either from TopBar or open attempt)
            if let Some(err) = error.as_ref().or(self.last_open_err.as_ref()) {
                ui.colored_label(egui::Color32::RED, err);
                ui.separator();
            }

            if self.loaded_path.is_none() {
                ui.label("Open a JSON/NDJSON file from the top bar to begin.");
                return;
            }

            // Let the viewer render itself (it already uses a ScrollArea internally)
            self.json_viewer.ui(ui);
        });
    }
}
