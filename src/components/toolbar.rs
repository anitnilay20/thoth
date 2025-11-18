use std::path::{Path, PathBuf};

use eframe::egui;
use rfd::FileDialog;

use crate::{
    components::traits::ContextComponent, file::lazy_loader::FileType, search::SearchMessage,
    shortcuts::KeyboardShortcuts,
};

#[derive(Default)]
pub struct Toolbar {
    pub previous_file_type: FileType,
    search_query: String,
    match_case: bool,
    pub request_search_focus: bool,
}

/// Props passed down to the Toolbar (immutable, one-way binding)
pub struct ToolbarProps<'a> {
    pub file_type: &'a FileType,
    pub dark_mode: bool,
    pub update_available: bool,
    pub shortcuts: &'a KeyboardShortcuts,
}

/// Events emitted by the toolbar (bottom-to-top communication)
pub enum ToolbarEvent {
    FileOpen { path: PathBuf, file_type: FileType },
    FileClear,
    NewWindow,
    FileTypeChange(FileType),
    ToggleSettings,
    ToggleTheme,
}

pub struct ToolbarOutput {
    pub search_message: Option<SearchMessage>,
    pub events: Vec<ToolbarEvent>,
}

impl ContextComponent for Toolbar {
    type Props<'a> = ToolbarProps<'a>;
    type Output = ToolbarOutput;

    fn render(&mut self, ctx: &egui::Context, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();
        let search_message = self.render_ui(ctx, props, &mut events);

        ToolbarOutput {
            search_message,
            events,
        }
    }
}

impl Toolbar {
    fn render_ui(
        &mut self,
        ctx: &egui::Context,
        props: ToolbarProps<'_>,
        events: &mut Vec<ToolbarEvent>,
    ) -> Option<SearchMessage> {
        let mut search_message = None;

        // Top bar with essential actions
        egui::TopBottomPanel::top("top_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                // File actions
                if ui
                    .button(format!("{} Open", egui_phosphor::regular::FOLDER_OPEN))
                    .on_hover_text(format!(
                        "Open file ({})",
                        props.shortcuts.open_file.format()
                    ))
                    .clicked()
                {
                    if let Some(path) = FileDialog::new()
                        .add_filter("JSON", &["json", "ndjson"])
                        .pick_file()
                    {
                        let file_type = infer_file_type(&path).unwrap_or(*props.file_type);
                        events.push(ToolbarEvent::FileOpen { path, file_type });
                        self.previous_file_type = file_type;
                    }
                }

                if ui
                    .button(format!("{} Clear", egui_phosphor::regular::X))
                    .on_hover_text(format!(
                        "Clear file ({})",
                        props.shortcuts.clear_file.format()
                    ))
                    .clicked()
                {
                    events.push(ToolbarEvent::FileClear);
                }

                if ui
                    .button(format!(
                        "{} New Window",
                        egui_phosphor::regular::SQUARES_FOUR
                    ))
                    .on_hover_text(format!(
                        "New window ({})",
                        props.shortcuts.new_window.format()
                    ))
                    .clicked()
                {
                    events.push(ToolbarEvent::NewWindow);
                }

                ui.separator();

                // File type selector
                let mut current_file_type = *props.file_type;
                egui::ComboBox::from_label("Type")
                    .selected_text(format!("{:?}", current_file_type))
                    .show_ui(ui, |ui| {
                        ui.selectable_value(&mut current_file_type, FileType::Json, "JSON");
                        ui.selectable_value(&mut current_file_type, FileType::Ndjson, "NDJSON");
                    });

                if self.previous_file_type != current_file_type
                    && current_file_type != *props.file_type
                {
                    events.push(ToolbarEvent::FileTypeChange(current_file_type));
                    self.previous_file_type = current_file_type;
                }

                // Spacer to push right-side items to the right
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    // Settings button (rightmost) with update notification badge
                    let settings_response = ui
                        .add(egui::Button::new(egui_phosphor::regular::GEAR))
                        .on_hover_text(format!("Settings ({})", props.shortcuts.settings.format()));

                    // Draw notification badge if update available
                    if props.update_available {
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
                        events.push(ToolbarEvent::ToggleSettings);
                    }

                    ui.separator();

                    // Dark mode toggle
                    let mut dark_mode = props.dark_mode;
                    let theme_icon = if dark_mode {
                        egui_phosphor::regular::MOON
                    } else {
                        egui_phosphor::regular::SUN
                    };
                    let theme_response = ui.checkbox(&mut dark_mode, theme_icon);
                    if dark_mode != props.dark_mode {
                        events.push(ToolbarEvent::ToggleTheme);
                    }
                    theme_response.on_hover_text(format!(
                        "Toggle theme ({})",
                        props.shortcuts.toggle_theme.format()
                    ));
                });
            });
        });

        // Bottom bar with search
        egui::TopBottomPanel::bottom("bottom_panel").show(ctx, |ui| {
            ui.horizontal(|ui| {
                ui.label(format!(
                    "{} Search:",
                    egui_phosphor::regular::MAGNIFYING_GLASS
                ));

                let text_box_response = ui.add(
                    egui::TextEdit::singleline(&mut self.search_query)
                        .desired_width(300.0)
                        .hint_text("Enter search term..."),
                );

                // Request focus if needed
                if self.request_search_focus {
                    text_box_response.request_focus();
                    self.request_search_focus = false;
                }

                ui.checkbox(&mut self.match_case, "Match Case");

                if ui
                    .button("Search")
                    .on_hover_text("Search for text in the file (Enter)")
                    .clicked()
                    || (text_box_response.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter)))
                {
                    search_message =
                        SearchMessage::create_search(self.search_query.clone(), self.match_case);
                }

                if ui
                    .button("Stop")
                    .on_hover_text("Stop the current search operation")
                    .clicked()
                {
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
