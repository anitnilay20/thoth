use eframe::egui;

#[derive(Default)]
pub struct CentralPanel {
    json_viewer: crate::components::json_viewer::JsonViewer,
}

impl CentralPanel {
    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        filtered_lines: &[serde_json::Value],
        error: &mut Option<String>,
    ) {
        egui::CentralPanel::default().show(ctx, |ui| {
            if let Some(err) = error {
                ui.colored_label(egui::Color32::RED, err);
                return;
            }

            if filtered_lines.is_empty() {
                ui.label("No file loaded or empty content.");
                return;
            }

            egui::ScrollArea::both().show(ui, |ui| {
                ui.set_width(ui.available_width());
                self.json_viewer
                    .load(serde_json::Value::Array(filtered_lines.to_vec()));
                self.json_viewer.ui(ui);
            });
        });
    }
}
