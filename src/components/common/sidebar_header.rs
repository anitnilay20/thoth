//! A uniform sidebar section header (title + optional trailing text / action
//! buttons) followed by a divider. Every sidebar section renders through this so
//! the headers line up at the same height regardless of whether they carry
//! right-side controls. Follows [`StatelessComponent`].

use eframe::egui;

use crate::components::common::separator::Separator;
use crate::components::common::typography::Typography;
use crate::components::icon_button::{IconButton, IconButtonProps};
use crate::components::traits::StatelessComponent;
use crate::theme::ThemeColors;

/// Fixed height of the header content row. Sized to comfortably fit a frameless
/// icon button so action-bearing headers don't grow taller than text-only ones.
const HEADER_H: f32 = 32.0;
/// Horizontal inset matching the list rows' left padding.
const PAD_X: f32 = 8.0;

/// A right-aligned, hover-tooltipped icon button in the header.
pub struct SidebarHeaderAction<'a> {
    pub icon: &'a str,
    pub tooltip: &'a str,
}

pub struct SidebarHeaderProps<'a> {
    /// Section title (rendered as a panel header, typically upper-case).
    pub title: &'a str,
    /// Optional muted text shown on the right (e.g. a count like "3 of 12").
    pub trailing_text: Option<&'a str>,
    /// Optional right-aligned icon buttons. The clicked index is reported in the output.
    pub actions: &'a [SidebarHeaderAction<'a>],
}

impl<'a> SidebarHeaderProps<'a> {
    /// A plain title-only header.
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            trailing_text: None,
            actions: &[],
        }
    }
}

pub struct SidebarHeaderOutput {
    /// Index into `props.actions` of the button that was clicked, if any.
    pub action_clicked: Option<usize>,
}

pub struct SidebarHeader;

impl StatelessComponent for SidebarHeader {
    type Props<'a> = SidebarHeaderProps<'a>;
    type Output = SidebarHeaderOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| crate::theme::Theme::default().colors())
        });

        let mut action_clicked = None;

        ui.allocate_ui(egui::vec2(ui.available_width(), HEADER_H), |ui| {
            ui.horizontal(|ui| {
                ui.set_min_height(HEADER_H);
                ui.add_space(PAD_X);
                Typography::panel_header(ui, props.title);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(PAD_X);
                    // right-to-left: iterate reversed so action[0] is leftmost.
                    for (idx, action) in props.actions.iter().enumerate().rev() {
                        let out = IconButton::render(
                            ui,
                            IconButtonProps {
                                icon: action.icon,
                                frame: false,
                                tooltip: Some(action.tooltip),
                                ..Default::default()
                            },
                        );
                        if out.clicked {
                            action_clicked = Some(idx);
                        }
                    }
                    if let Some(text) = props.trailing_text {
                        ui.label(egui::RichText::new(text).color(colors.fg_muted).size(10.0));
                    }
                });
            });
        });

        Separator::plain(ui);

        SidebarHeaderOutput { action_clicked }
    }
}
