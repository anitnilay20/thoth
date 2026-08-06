use egui::{InnerResponse, Response, RichText, Stroke, Widget};

use crate::components::{IconButton, Typography, TypographyVariant};
use crate::theme::{ThemeColors, with_alpha};

use super::SidebarHeader;

/// Fixed height of the header content row — design `.shrow{height:32px}`. Sized
/// to fit a ghost icon button, so action-bearing headers match text-only ones.
const HEADER_H: f32 = 32.0;
/// Horizontal inset matching the list rows' left padding — design `.sh{padding:0 8px}`.
const PAD_X: f32 = 8.0;
/// Gap between the title and what follows it — design `.shrow{gap:8px}`.
const GAP: f32 = 8.0;
/// Trailing count text — design `.shtrail{font-size:10px}`.
const FONT_TRAILING: f32 = 10.0;
/// Gap between trailing icon buttons — design `.shacts{gap:1px}`.
const ACTION_GAP: f32 = 1.0;
/// Ghost `.ib` metrics: a 24px square with a 14px glyph.
const ACTION_SIZE: f32 = 24.0;
/// …glyph size inside it.
const ACTION_GLYPH: f32 = 14.0;
/// Separator beneath the row — design
/// `.shsep{height:1px;background:surface1@30%;margin-top:2px}`.
const SEP_ALPHA: u8 = 77; // 30% of 255
/// …and its offset below the row.
const SEP_GAP: f32 = 2.0;

impl SidebarHeader {
    /// Render the header and report which action (if any) was clicked this
    /// frame via [`InnerResponse::inner`] (the index into `actions`).
    pub fn show(&self, ui: &mut egui::Ui) -> InnerResponse<Option<usize>> {
        let colors = ThemeColors::from_ctx(ui.ctx());
        let mut action_clicked = None;

        let inner = ui.allocate_ui(egui::vec2(ui.available_width(), HEADER_H), |ui| {
            ui.horizontal(|ui| {
                ui.set_min_height(HEADER_H);
                // Every gap in the row is explicit — design `.shrow{gap:8px}`.
                ui.spacing_mut().item_spacing.x = 0.0;
                ui.add_space(PAD_X);
                // `.sht` — 11px bold in the section-header colour (`--overlay2`).
                ui.add(
                    Typography::builder()
                        .text(self.title.as_str())
                        .variant(TypographyVariant::PanelHeader)
                        .build(),
                );
                ui.add_space(GAP);

                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add_space(PAD_X);
                    // Right-to-left: iterate reversed so actions[0] is leftmost.
                    for (idx, action) in self.actions.iter().enumerate().rev() {
                        if idx + 1 < self.actions.len() {
                            ui.add_space(ACTION_GAP);
                        }
                        let clicked = ui
                            .add(
                                IconButton::builder()
                                    .icon(action.icon.as_str())
                                    .tooltip(action.tooltip.as_str())
                                    .size_px(ACTION_SIZE)
                                    .icon_size(ACTION_GLYPH)
                                    .build(),
                            )
                            .clicked();
                        if clicked {
                            action_clicked = Some(idx);
                        }
                    }
                    if let Some(text) = self.trailing_text.as_deref() {
                        if !self.actions.is_empty() {
                            ui.add_space(GAP);
                        }
                        ui.label(
                            RichText::new(text)
                                .color(colors.fg_muted)
                                .size(FONT_TRAILING),
                        );
                    }
                });
            });
        });

        // `.shsep` — a 1px tinted rule 2px under the row, full panel width.
        ui.add_space(SEP_GAP);
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 1.0), egui::Sense::hover());
        ui.painter().hline(
            rect.x_range(),
            rect.center().y,
            Stroke::new(1.0, with_alpha(colors.surface_raised, SEP_ALPHA)),
        );

        InnerResponse::new(action_clicked, inner.response)
    }
}

impl Widget for SidebarHeader {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        self.show(ui).response
    }
}
