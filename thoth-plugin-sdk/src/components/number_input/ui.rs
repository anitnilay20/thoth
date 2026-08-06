use egui::{Align2, Color32, CornerRadius, Rect, Sense, Stroke, TextStyle, Vec2};

use crate::theme::{
    FIELD_HEIGHT, FONT_BODY, FONT_CAPTION, RADIUS_CONTROL, ThemeColors, edge_stroke,
    phosphor_font_id, with_alpha,
};

use super::NumberInput;

/// Width of each spin button — design `.num button{width:28px}`.
const SPIN_W: f32 = 28.0;
/// Width of the value area — design `.num input{width:50px}`.
const VALUE_W: f32 = 50.0;
/// Padding after the unit suffix — design `.num .unit{padding-right:10px}`.
const UNIT_PAD: f32 = 10.0;
/// Spin-button glyph size — design `.num button{font-size:16px}`.
const SPIN_GLYPH: f32 = 16.0;

impl NumberInput {
    /// Render the input, editing [`value`](NumberInput::value) in place.
    ///
    /// The returned response reports `changed()` for both spin-button clicks and
    /// drag/keyboard edits of the value.
    pub fn show(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let colors = ThemeColors::from_ctx(ui.ctx());
        if !self.label.is_empty() {
            ui.label(&self.label);
        }

        let min = self.min.unwrap_or(f64::NEG_INFINITY);
        let max = self.max.unwrap_or(f64::INFINITY);
        let step = self.step.unwrap_or(1.0);

        // The unit is laid out up front so the control is exactly as wide as its
        // content (design `.num` is an inline-flex box).
        let unit = self.unit.as_deref().filter(|u| !u.is_empty()).map(|u| {
            ui.painter().layout_no_wrap(
                u.to_owned(),
                egui::FontId::proportional(FONT_CAPTION),
                colors.fg_faint(),
            )
        });
        let unit_w = unit.as_ref().map_or(0.0, |g| g.size().x + UNIT_PAD);
        let total_w = 2.0 * SPIN_W + VALUE_W + unit_w;

        let (rect, _) = ui.allocate_exact_size(egui::vec2(total_w, FIELD_HEIGHT), Sense::hover());
        // `overflow:hidden` in the design — the spin hover fills below are given
        // matching outer corners so nothing pokes past the rounded box.
        let radius = CornerRadius::from(RADIUS_CONTROL);
        if ui.is_rect_visible(rect) {
            ui.painter().rect_filled(rect, radius, colors.surface);
            ui.painter()
                .rect_stroke(rect, radius, edge_stroke(&colors), egui::StrokeKind::Inside);
        }

        let dec_rect = Rect::from_min_size(rect.min, egui::vec2(SPIN_W, rect.height()));
        let value_rect =
            Rect::from_min_size(dec_rect.right_top(), egui::vec2(VALUE_W, rect.height()));
        let inc_rect = Rect::from_min_size(
            egui::pos2(rect.max.x - SPIN_W, rect.min.y),
            egui::vec2(SPIN_W, rect.height()),
        );

        let id = ui.make_persistent_id((self.id.as_str(), "num"));
        let mut spun = false;

        let inner = ui.scope(|ui| {
            if self.disabled {
                ui.disable();
            }

            // ── Spin buttons ──────────────────────────────────────────────────
            let dec = ui.interact(dec_rect, id.with("dec"), Sense::click());
            let inc = ui.interact(inc_rect, id.with("inc"), Sense::click());
            for (resp, glyph, corners) in [
                (
                    &dec,
                    egui_phosphor::regular::MINUS,
                    CornerRadius {
                        nw: radius.nw,
                        sw: radius.sw,
                        ne: 0,
                        se: 0,
                    },
                ),
                (
                    &inc,
                    egui_phosphor::regular::PLUS,
                    CornerRadius {
                        nw: 0,
                        sw: 0,
                        ne: radius.ne,
                        se: radius.se,
                    },
                ),
            ] {
                let hovered = resp.hovered();
                if hovered {
                    ui.painter()
                        .rect_filled(resp.rect, corners, with_alpha(colors.fg, 20)); // text@8%
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                ui.painter().text(
                    resp.rect.center(),
                    Align2::CENTER_CENTER,
                    glyph,
                    phosphor_font_id(SPIN_GLYPH),
                    if hovered { colors.fg } else { colors.fg_muted },
                );
            }
            if dec.clicked() {
                self.value = (self.value - step).clamp(min, max);
                spun = true;
            }
            if inc.clicked() {
                self.value = (self.value + step).clamp(min, max);
                spun = true;
            }

            // ── Value ─────────────────────────────────────────────────────────
            let builder = egui::UiBuilder::new().max_rect(value_rect).layout(
                egui::Layout::centered_and_justified(egui::Direction::LeftToRight),
            );
            let drag = ui
                .scope_builder(builder, |ui| {
                    let style = ui.style_mut();
                    style.spacing.button_padding = Vec2::ZERO;
                    style.spacing.interact_size = egui::vec2(VALUE_W, FIELD_HEIGHT);
                    // Design `.num input{font:500 13px/1 var(--mono)}`.
                    style
                        .text_styles
                        .insert(TextStyle::Monospace, egui::FontId::monospace(FONT_BODY));
                    style.drag_value_text_style = TextStyle::Monospace;
                    // The value carries no chrome of its own; the box behind it is
                    // already painted.
                    for w in [
                        &mut style.visuals.widgets.inactive,
                        &mut style.visuals.widgets.hovered,
                        &mut style.visuals.widgets.active,
                    ] {
                        w.weak_bg_fill = Color32::TRANSPARENT;
                        w.bg_fill = Color32::TRANSPARENT;
                        w.bg_stroke = Stroke::NONE;
                        w.fg_stroke.color = colors.fg;
                        w.expansion = 0.0;
                    }
                    ui.add(egui::DragValue::new(&mut self.value).range(min..=max))
                })
                .inner;

            // ── Unit suffix ───────────────────────────────────────────────────
            if let Some(galley) = unit {
                let pos = egui::pos2(value_rect.max.x, rect.center().y - galley.size().y * 0.5);
                ui.painter().galley(pos, galley, colors.fg_faint());
            }

            drag | dec | inc
        });

        let mut response = inner.inner;
        if spun {
            response.mark_changed();
        }
        response
    }
}
