use std::path::{Path, PathBuf};

use eframe::egui;
use rfd::FileDialog;

use crate::{
    components::{
        icon_button::{IconButton, IconButtonProps},
        traits::{ContextComponent, StatelessComponent},
    },
    file::lazy_loader::FileType,
    shortcuts::KeyboardShortcuts,
};

#[derive(Default)]
pub struct Toolbar {
    pub previous_file_type: FileType,
}

/// Props passed down to the Toolbar (immutable, one-way binding)
pub struct ToolbarProps<'a> {
    pub file_type: &'a FileType,
    pub dark_mode: bool,
    pub shortcuts: &'a KeyboardShortcuts,
    pub file_path: Option<&'a Path>,
    pub is_fullscreen: bool,
}

/// Events emitted by the toolbar (bottom-to-top communication)
pub enum ToolbarEvent {
    FileOpen { path: PathBuf, file_type: FileType },
    FileClear,
    NewWindow,
    FileTypeChange(FileType),
    ToggleTheme,
}

pub struct ToolbarOutput {
    pub events: Vec<ToolbarEvent>,
}

impl ContextComponent for Toolbar {
    type Props<'a> = ToolbarProps<'a>;
    type Output = ToolbarOutput;

    fn render(&mut self, ctx: &egui::Context, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();
        self.render_ui(ctx, props, &mut events);

        ToolbarOutput { events }
    }
}

impl Toolbar {
    fn render_ui(
        &mut self,
        ctx: &egui::Context,
        props: ToolbarProps<'_>,
        events: &mut Vec<ToolbarEvent>,
    ) {
        // Use theme colors from context
        let bg_color = ctx.style().visuals.extreme_bg_color; // Catppuccin Mantle

        // Row 1: Title bar (32px height - integrated with window controls, with title)
        // Hide completely in fullscreen mode
        if !props.is_fullscreen {
            egui::TopBottomPanel::top("title_bar_row")
                .exact_height(32.0)
                .frame(egui::Frame::NONE.fill(bg_color).inner_margin(egui::Margin {
                    left: 8,
                    right: 8,
                    top: 0,
                    bottom: 0,
                }))
                .show(ctx, |ui| {
                    ui.vertical_centered(|ui| {
                        ui.horizontal_centered(|ui| {
                            // Space for macOS traffic light buttons
                            #[cfg(target_os = "macos")]
                            let traffic_light_space = 70.0;
                            #[cfg(not(target_os = "macos"))]
                            let traffic_light_space = 0.0;

                            ui.add_space(traffic_light_space);

                            // Display "Thoth - filename" or just "Thoth" centered
                            let title = if let Some(path) = props.file_path {
                                let filename = path
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("Untitled");
                                format!("Thoth - {}", filename)
                            } else {
                                "Thoth".to_string()
                            };

                            // Calculate centering: total width - traffic light space, then center within that
                            let available_width = ui.available_width();
                            let text_width = ui.fonts(|f| {
                                f.layout_no_wrap(
                                    title.clone(),
                                    egui::FontId::proportional(13.0),
                                    ui.visuals().text_color(),
                                )
                                .rect
                                .width()
                            });

                            // Center the text, accounting for the traffic light offset
                            let center_offset =
                                (available_width - text_width) / 2.0 - traffic_light_space / 2.0;
                            if center_offset > 0.0 {
                                ui.add_space(center_offset);
                            }

                            ui.label(egui::RichText::new(title).size(13.0));
                        });
                    });
                });
        }

        // Row 2: Button toolbar (32px height)
        egui::TopBottomPanel::top("button_toolbar")
            .exact_height(32.0)
            .frame(egui::Frame::NONE.fill(bg_color).inner_margin(egui::Margin {
                left: 8,
                right: 8,
                top: 0,
                bottom: 0,
            }))
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);

                    // Button size to match ComboBox height
                    let button_size = egui::vec2(28.0, 28.0);

                    // File actions (icon-only buttons)
                    if IconButton::render(
                        ui,
                        IconButtonProps {
                            icon: egui_phosphor::regular::FOLDER_OPEN,
                            frame: false,
                            tooltip: Some(&format!(
                                "Open file ({})",
                                props.shortcuts.open_file.format()
                            )),
                            badge_color: None,
                            size: Some(button_size),
                        },
                    )
                    .clicked
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

                    if IconButton::render(
                        ui,
                        IconButtonProps {
                            icon: egui_phosphor::regular::X,
                            frame: false,
                            tooltip: Some(&format!(
                                "Clear file ({})",
                                props.shortcuts.clear_file.format()
                            )),
                            badge_color: None,
                            size: Some(button_size),
                        },
                    )
                    .clicked
                    {
                        events.push(ToolbarEvent::FileClear);
                    }

                    if IconButton::render(
                        ui,
                        IconButtonProps {
                            icon: egui_phosphor::regular::SQUARES_FOUR,
                            frame: false,
                            tooltip: Some(&format!(
                                "New window ({})",
                                props.shortcuts.new_window.format()
                            )),
                            badge_color: None,
                            size: Some(button_size),
                        },
                    )
                    .clicked
                    {
                        events.push(ToolbarEvent::NewWindow);
                    }

                    ui.add_space(4.0);
                    ui.separator();
                    ui.add_space(4.0);

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

                    // Spacer to push right-side items to the right
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(4.0, 0.0);

                        // Theme toggle (icon-only)
                        let theme_icon = if props.dark_mode {
                            egui_phosphor::regular::MOON
                        } else {
                            egui_phosphor::regular::SUN
                        };
                        if IconButton::render(
                            ui,
                            IconButtonProps {
                                icon: theme_icon,
                                frame: false,
                                tooltip: Some(&format!(
                                    "Toggle theme ({})",
                                    props.shortcuts.toggle_theme.format()
                                )),
                                badge_color: None,
                                size: Some(button_size),
                            },
                        )
                        .clicked
                        {
                            events.push(ToolbarEvent::ToggleTheme);
                        }
                    });
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
