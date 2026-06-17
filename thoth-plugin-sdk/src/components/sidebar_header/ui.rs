use egui::{InnerResponse, Response, RichText, Widget};

use crate::components::{IconButton, Separator, Typography, TypographyVariant};
use crate::theme::ThemeColors;

use super::SidebarHeader;

/// Fixed height of the header content row — sized to fit a frameless icon
/// button so action-bearing headers match text-only ones.
const HEADER_H: f32 = 32.0;
/// Horizontal inset matching the list rows' left padding.
const PAD_X: f32 = 8.0;

impl SidebarHeader {
    /// Render the header and report which action (if any) was clicked this
    /// frame via [`InnerResponse::inner`] (the index into `actions`).
    pub fn show(&self, ui: &mut egui::Ui) -> InnerResponse<Option<usize>> {
        let colors = ThemeColors::from_ctx(ui.ctx());
        let mut action_clicked = None;

        let inner = ui.allocate_ui(egui::vec2(ui.available_width(), HEADER_H), |ui| {
            ui.horizontal(|ui| {
                ui.set_min_height(HEADER_H);
                ui.add_space(PAD_X);
                ui.add(
                    Typography::builder()
                        .text(self.title.as_str())
                        .variant(TypographyVariant::PanelHeader)
                        .build(),
                );

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(PAD_X);
                    // Right-to-left: iterate reversed so actions[0] is leftmost.
                    for (idx, action) in self.actions.iter().enumerate().rev() {
                        let clicked = ui
                            .add(
                                IconButton::builder()
                                    .icon(action.icon.as_str())
                                    .tooltip(action.tooltip.as_str())
                                    .build(),
                            )
                            .clicked();
                        if clicked {
                            action_clicked = Some(idx);
                        }
                    }
                    if let Some(text) = self.trailing_text.as_deref() {
                        ui.label(RichText::new(text).color(colors.fg_muted).size(10.0));
                    }
                });
            });
        });

        ui.add(Separator::plain());

        InnerResponse::new(action_clicked, inner.response)
    }
}

impl Widget for SidebarHeader {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        self.show(ui).response
    }
}
