use eframe::egui;
use std::path::Path;

use crate::components::breadcrumbs::{Breadcrumbs, BreadcrumbsEvent, BreadcrumbsProps};
use crate::components::traits::{ContextComponent, StatelessComponent};
use crate::file::loaders::FileType;

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

    /// Currently selected path in the JSON (for breadcrumbs)
    pub selected_path: Option<&'a str>,
}

/// Status indicator for the status bar
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusBarStatus {
    Ready,
    #[allow(dead_code)]
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

    /// Get the color for this status from theme
    pub fn color(&self, ctx: &egui::Context) -> egui::Color32 {
        ctx.memory(|mem| {
            let theme_colors = mem
                .data
                .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| {
                    // Fallback: create default theme based on dark mode from visuals
                    let dark_mode = ctx.style().visuals.dark_mode;
                    crate::theme::Theme::for_dark_mode(dark_mode).colors()
                });

            match self {
                StatusBarStatus::Ready => theme_colors.success,
                StatusBarStatus::Loading => theme_colors.warning,
                StatusBarStatus::Error => theme_colors.error,
                StatusBarStatus::Searching | StatusBarStatus::Filtered => theme_colors.info,
            }
        })
    }
}

/// Events emitted by the status bar
#[derive(Debug, Clone)]
pub enum StatusBarEvent {
    /// User clicked on a breadcrumb to navigate
    NavigateToPath(String),
}

/// Output from status bar component
pub struct StatusBarOutput {
    pub events: Vec<StatusBarEvent>,
}

impl ContextComponent for StatusBar {
    type Props<'a> = StatusBarProps<'a>;
    type Output = StatusBarOutput;

    fn render(&mut self, ctx: &egui::Context, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();

        // Use theme colors from context
        let bg_color = ctx.memory(|mem| {
            mem.data
                .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| {
                    // Fallback: create default theme based on dark mode from visuals
                    let dark_mode = ctx.style().visuals.dark_mode;
                    crate::theme::Theme::for_dark_mode(dark_mode).colors()
                })
                .crust
        });

        egui::TopBottomPanel::bottom("status_bar")
            .exact_height(24.0)
            .frame(egui::Frame::NONE.fill(bg_color).inner_margin(egui::Margin {
                left: 12,
                right: 12,
                top: 4,
                bottom: 4,
            }))
            .show(ctx, |ui| {
                // Use theme text color from context
                let text_color = ctx.memory(|mem| {
                    mem.data
                        .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
                        .unwrap_or_else(|| {
                            // Fallback: create default theme based on dark mode from visuals
                            let dark_mode = ctx.style().visuals.dark_mode;
                            crate::theme::Theme::for_dark_mode(dark_mode).colors()
                        })
                        .text
                });
                ui.style_mut().visuals.override_text_color = Some(text_color);

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);

                    // Filename with icon
                    if let Some(path) = props.file_path {
                        let filename = path
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or("Untitled");
                        ui.label(format!(
                            "{} {}",
                            egui_phosphor::regular::FILE_TEXT,
                            filename
                        ));
                        ui.separator();
                    }

                    // Item count with icon
                    if let Some(filtered) = props.filtered_count {
                        ui.label(format!(
                            "{} {} of {} items",
                            egui_phosphor::regular::FUNNEL,
                            filtered,
                            props.item_count
                        ));
                    } else if props.item_count > 0 {
                        ui.label(format!(
                            "{} {} items",
                            egui_phosphor::regular::LIST_BULLETS,
                            props.item_count
                        ));
                    } else {
                        ui.label(format!("{} No items", egui_phosphor::regular::LIST_BULLETS));
                    }

                    ui.separator();

                    // File type with icon
                    let file_type_icon = match props.file_type {
                        crate::file::lazy_loader::FileType::Json => {
                            egui_phosphor::regular::BRACKETS_CURLY
                        }
                        crate::file::lazy_loader::FileType::Ndjson => {
                            egui_phosphor::regular::LIST_DASHES
                        }
                    };
                    ui.label(format!("{} {:?}", file_type_icon, props.file_type));

                    // Breadcrumbs navigation
                    if props.selected_path.is_some() {
                        ui.separator();
                        let breadcrumbs_output = Breadcrumbs::render(
                            ui,
                            BreadcrumbsProps {
                                current_path: props.selected_path,
                            },
                        );

                        // Convert breadcrumb events to status bar events
                        for event in breadcrumbs_output.events {
                            match event {
                                BreadcrumbsEvent::NavigateToPath(path) => {
                                    events.push(StatusBarEvent::NavigateToPath(path));
                                }
                            }
                        }
                    }

                    // Push status to the right
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Status indicator
                        let (icon, text) = props.status.icon_and_text();
                        let status_color = props.status.color(ctx);
                        ui.colored_label(status_color, format!("{} {}", icon, text));
                    });
                });
            });

        StatusBarOutput { events }
    }
}
