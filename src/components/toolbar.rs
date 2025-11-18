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

        // Compact single-row toolbar (40px height)
        // Use theme colors from context
        let bg_color = ctx.style().visuals.extreme_bg_color; // Catppuccin Mantle
        let border_color = ctx.style().visuals.widgets.noninteractive.bg_stroke.color;

        egui::TopBottomPanel::top("top_panel")
            .exact_height(40.0)
            .frame(
                egui::Frame::NONE
                    .fill(bg_color)
                    .inner_margin(egui::Margin {
                        left: 8,
                        right: 8,
                        top: 0,
                        bottom: 0,
                    })
                    .stroke(egui::Stroke::new(1.0, border_color)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    // On macOS with fullsize_content_view, add padding for traffic light buttons
                    #[cfg(target_os = "macos")]
                    ui.add_space(70.0);

                    ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);

                    // File actions (icon-only buttons)
                    if ui
                        .add(egui::Button::new(egui_phosphor::regular::FOLDER_OPEN).frame(false))
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
                        .add(egui::Button::new(egui_phosphor::regular::X).frame(false))
                        .on_hover_text(format!(
                            "Clear file ({})",
                            props.shortcuts.clear_file.format()
                        ))
                        .clicked()
                    {
                        events.push(ToolbarEvent::FileClear);
                    }

                    if ui
                        .add(egui::Button::new(egui_phosphor::regular::SQUARES_FOUR).frame(false))
                        .on_hover_text(format!(
                            "New window ({})",
                            props.shortcuts.new_window.format()
                        ))
                        .clicked()
                    {
                        events.push(ToolbarEvent::NewWindow);
                    }

                    ui.add_space(8.0); // Separator spacing
                    ui.separator();
                    ui.add_space(8.0);

                    // File type selector (compact, no label)
                    let mut current_file_type = *props.file_type;
                    egui::ComboBox::from_id_salt("file_type")
                        .selected_text(format!("{:?}", current_file_type))
                        .width(80.0)
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

                    ui.add_space(8.0);
                    ui.separator();
                    ui.add_space(8.0);

                    // Integrated search box
                    let text_box_response = ui.add(
                        egui::TextEdit::singleline(&mut self.search_query)
                            .desired_width(200.0)
                            .hint_text("🔍 Search..."),
                    );

                    // Handle search on Enter
                    if text_box_response.lost_focus()
                        && ui.input(|i| i.key_pressed(egui::Key::Enter))
                    {
                        search_message = SearchMessage::create_search(
                            self.search_query.clone(),
                            self.match_case,
                        );
                    }

                    // Request focus if needed
                    if self.request_search_focus {
                        text_box_response.request_focus();
                        self.request_search_focus = false;
                    }

                    // Match case toggle button (Aa icon)
                    let match_case_button = ui.add(
                        egui::Button::new("Aa")
                            .frame(self.match_case)
                            .selected(self.match_case),
                    );
                    if match_case_button.clicked() {
                        self.match_case = !self.match_case;
                    }
                    match_case_button.on_hover_text("Match case");

                    // Spacer to push right-side items to the right
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);

                        // Theme toggle (icon-only)
                        let theme_icon = if props.dark_mode {
                            egui_phosphor::regular::MOON
                        } else {
                            egui_phosphor::regular::SUN
                        };
                        if ui
                            .add(egui::Button::new(theme_icon).frame(false))
                            .on_hover_text(format!(
                                "Toggle theme ({})",
                                props.shortcuts.toggle_theme.format()
                            ))
                            .clicked()
                        {
                            events.push(ToolbarEvent::ToggleTheme);
                        }

                        ui.add_space(4.0);

                        // Settings button with optional update notification badge
                        let settings_response = ui
                            .add(egui::Button::new(egui_phosphor::regular::GEAR).frame(false))
                            .on_hover_text(format!(
                                "Settings ({})",
                                props.shortcuts.settings.format()
                            ));

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

                            ui.painter().circle_stroke(
                                badge_center,
                                badge_radius,
                                egui::Stroke::new(1.5, egui::Color32::WHITE),
                            );
                        }

                        if settings_response.clicked() {
                            events.push(ToolbarEvent::ToggleSettings);
                        }
                    });
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
