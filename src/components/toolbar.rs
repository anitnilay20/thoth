use std::path::{Path, PathBuf};

use eframe::egui;
use rfd::FileDialog;

use crate::{file::lazy_loader::FileType, search::SearchMessage};

#[derive(Default)]
pub struct Toolbar {
    pub previous_file_type: FileType,
    search_query: String,
    match_case: bool,
}

impl Toolbar {
    pub fn ui(
        &mut self,
        ctx: &egui::Context,
        file_path: &mut Option<PathBuf>,
        file_type: &mut FileType,
        error: &mut Option<String>,
        dark_mode: &mut bool,
        show_settings: &mut bool,
        update_available: bool,
    ) -> Option<SearchMessage> {
        let mut search_message = None;

        // Top bar with essential actions
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // File actions
                if ui.button("📂 Open").clicked() {
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

                if ui.button("✖ Clear").clicked() {
                    *file_path = None;
                    *error = None;
                }

                ui.separator();

                // File type selector
                egui::ComboBox::from_label("Type")
                    .selected_text(format!("{:?}", file_type))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(file_type, FileType::Json, "JSON");
                        ui.selectable_value(file_type, FileType::Ndjson, "NDJSON");
                    });

                if self.previous_file_type != *file_type {
                    self.previous_file_type = *file_type;
                }

                // Spacer to push right-side items to the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Settings button (rightmost) with update notification badge
                    let settings_response = ui.add(egui::Button::new("⚙"));

                    // Draw notification badge if update available
                    if update_available {
                        let button_rect = settings_response.rect;
                        let badge_center =
                            egui::pos2(button_rect.right() - 6.0, button_rect.top() + 6.0);
                        let badge_radius = 2.0;

                        ui.painter().circle_filled(
                            badge_center,
                            badge_radius,
                            egui::Color32::from_rgb(255, 80, 80),
                        );

                        // Optional: Add white border around badge
                        ui.painter().circle_stroke(
                            badge_center,
                            badge_radius,
                            egui::Stroke::new(1.5, egui::Color32::WHITE),
                        );
                    }

                    if settings_response.clicked() {
                        *show_settings = !*show_settings;
                    }

                    ui.separator();

                    // Dark mode toggle
                    ui.checkbox(dark_mode, "🌙");
                });
            });
        });

        // Bottom bar with search
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label("🔍 Search:");

                let text_box_response = ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .desired_width(300.0)
                        .hint_text("Enter search term..."),
                );

                ui.checkbox(&mut self.match_case, "Match case");

                if ui.button("Search").clicked()
                    || (text_box_response.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    search_message =
                        SearchMessage::create_search(self.search_query.clone(), self.match_case);
                }

                if ui.button("Stop").clicked() {
                    search_message = Some(SearchMessage::StopSearch);
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
