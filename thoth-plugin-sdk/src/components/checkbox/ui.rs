use egui::{Align2, CornerRadius, Rect, Sense, StrokeKind, Vec2};

use crate::theme::{
    FONT_CONTROL, RADIUS_CHECK, ThemeColors, edge_stroke, get_contrast_text_color, phosphor_font_id,
};

use super::Checkbox;

/// Box side — design `.cb { width:16px; height:16px }`.
const BOX_SIZE: f32 = 16.0;
/// Gap between the box and its label — design `.opt-lbl { gap:8px }`.
const LABEL_GAP: f32 = 8.0;
/// Glyph size inside the box — design `.cb { font-size:11px }`.
const GLYPH_SIZE: f32 = 11.0;
/// Opacity of a disabled checkbox. The design sheet has no disabled checkbox
/// variant; match the switch (`.switch.dis { opacity:.4 }`).
const DISABLED_OPACITY: f32 = 0.4;

impl Checkbox {
    /// Render the checkbox, toggling [`checked`](Checkbox::checked) in place.
    pub fn show(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let colors = ThemeColors::from_ctx(ui.ctx());
        let interactive = !self.disabled && ui.is_enabled();

        let galley = ui.painter().layout_no_wrap(
            self.label.clone(),
            egui::FontId::proportional(FONT_CONTROL),
            colors.fg,
        );
        let label_w = if self.label.is_empty() {
            0.0
        } else {
            LABEL_GAP + galley.size().x
        };
        let desired = Vec2::new(BOX_SIZE + label_w, galley.size().y.max(BOX_SIZE));

        // The label is part of the hit area — design wraps both in one `.opt-lbl`.
        let (rect, mut response) = ui.allocate_exact_size(
            desired,
            if interactive {
                Sense::click()
            } else {
                Sense::hover()
            },
        );
        // Clicking a mixed box resolves it, as elsewhere on the platform.
        if response.clicked() {
            self.checked = !self.checked;
            self.indeterminate = false;
            response.mark_changed();
        }

        if ui.is_rect_visible(rect) {
            // Disabled checkboxes fade through a *cloned* painter so the opacity
            // never leaks into the widgets painted after this one.
            let mut painter = ui.painter().clone();
            if !interactive {
                painter.multiply_opacity(DISABLED_OPACITY);
            }

            let box_rect = Rect::from_center_size(
                egui::pos2(rect.left() + BOX_SIZE / 2.0, rect.center().y),
                Vec2::splat(BOX_SIZE),
            );
            let radius = CornerRadius::same(RADIUS_CHECK as u8);

            if self.checked || self.indeterminate {
                // A filled box drops the hairline edge — design `.cb.on`/`.cb.ind`
                // both set `box-shadow:none`.
                painter.rect_filled(box_rect, radius, colors.accent);
                let glyph = if self.indeterminate {
                    egui_phosphor::regular::MINUS
                } else {
                    egui_phosphor::regular::CHECK
                };
                painter.text(
                    box_rect.center(),
                    Align2::CENTER_CENTER,
                    glyph,
                    phosphor_font_id(GLYPH_SIZE),
                    get_contrast_text_color(colors.accent),
                );
            } else {
                painter.rect(
                    box_rect,
                    radius,
                    colors.surface,
                    edge_stroke(&colors),
                    StrokeKind::Inside,
                );
            }

            if !self.label.is_empty() {
                let text_pos = egui::pos2(
                    box_rect.right() + LABEL_GAP,
                    rect.center().y - galley.size().y / 2.0,
                );
                painter.galley(text_pos, galley, colors.fg);
            }
        }

        if interactive {
            response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        }

        response
    }
}
