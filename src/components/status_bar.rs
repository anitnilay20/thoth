use eframe::egui;
use std::path::Path;

use crate::components::traits::ContextComponent;
use crate::file::lazy_loader::FileType;

/// Status bar component displaying file info and application status
#[derive(Default)]
pub struct StatusBar;

/// Props for the status bar component (immutable, one-way binding)
pub struct StatusBarProps<'a> {
    /// Current file path (if any)
    pub file_path: Option<&'a Path>,

    /// File type
    pub file_type: &'a FileType,

    /// Total item count
    pub item_count: usize,

    /// Filtered item count (if search is active)
    pub filtered_count: Option<usize>,

    /// Current status
    pub status: StatusBarStatus,

    /// Dark mode
    pub dark_mode: bool,
}

/// Status indicator for the status bar
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusBarStatus {
    Ready,
    Loading,
    Error,
    Searching,
    Filtered,
}

impl StatusBarStatus {
    /// Get the icon and text for this status
    pub fn icon_and_text(&self) -> (&'static str, &'static str) {
        match self {
            StatusBarStatus::Ready => ("⚡", "Ready"),
            StatusBarStatus::Loading => ("⏳", "Loading..."),
            StatusBarStatus::Error => ("⚠", "Error"),
            StatusBarStatus::Searching => ("🔍", "Searching..."),
            StatusBarStatus::Filtered => ("🔍", "Filtered"),
        }
    }

    /// Get the color for this status
    pub fn color(&self) -> egui::Color32 {
        match self {
            StatusBarStatus::Ready => egui::Color32::WHITE,
            StatusBarStatus::Loading => egui::Color32::from_rgb(0xcc, 0xa7, 0x00), // Yellow
            StatusBarStatus::Error => egui::Color32::from_rgb(0xf1, 0x4c, 0x4c),   // Red
            StatusBarStatus::Searching | StatusBarStatus::Filtered => {
                egui::Color32::from_rgb(0x4e, 0xc9, 0xb0) // Cyan/teal
            }
        }
    }
}

/// Output from status bar component (currently no events)
pub struct StatusBarOutput;

impl ContextComponent for StatusBar {
    type Props<'a> = StatusBarProps<'a>;
    type Output = StatusBarOutput;

    fn render(&mut self, ctx: &egui::Context, props: Self::Props<'_>) -> Self::Output {
        // VS Code blue background for status bar
        let bg_color = egui::Color32::from_rgb(0x00, 0x7a, 0xcc);

        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(24.0)
            .frame(egui::Frame::NONE.fill(bg_color).inner_margin(egui::Margin {
                left: 12,
                right: 12,
                top: 4,
                bottom: 4,
            }))
            .show(ctx, |ui| {
                // Override text color to white
                ui.style_mut().visuals.override_text_color = Some(egui::Color32::WHITE);

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);

                    // Filename with icon
                    if let Some(path) = props.file_path {
                        let filename = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Untitled");
                        ui.label(format!("📄 {}", filename));
                        ui.label("│");
                    }

                    // Item count
                    if let Some(filtered) = props.filtered_count {
                        ui.label(format!("{} of {} items", filtered, props.item_count));
                    } else if props.item_count > 0 {
                        ui.label(format!("{} items", props.item_count));
                    } else {
                        ui.label("No items");
                    }

                    ui.label("│");

                    // File type
                    ui.label(format!("{:?}", props.file_type));

                    ui.label("│");

                    // Status indicator
                    let (icon, text) = props.status.icon_and_text();
                    let status_color = props.status.color();
                    ui.colored_label(status_color, format!("{} {}", icon, text));
                });
            });

        StatusBarOutput
    }
}
