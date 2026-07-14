use egui::{Color32, CornerRadius, Response, Sense, Vec2, Widget};

use crate::theme::ThemeColors;

use super::ToggleSwitch;

#[inline]
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t.clamp(0.0, 1.0)) as u8
}

#[inline]
fn lerp_color(a: Color32, b: Color32, t: f32) -> Color32 {
    Color32::from_rgba_unmultiplied(
        lerp_u8(a.r(), b.r(), t),
        lerp_u8(a.g(), b.g(), t),
        lerp_u8(a.b(), b.b(), t),
        lerp_u8(a.a(), b.a(), t),
    )
}

impl Widget for ToggleSwitch {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let colors = ThemeColors::from_ctx(ui.ctx());

        let (rect, mut response) = ui.allocate_exact_size(Vec2::new(36.0, 20.0), Sense::click());

        if ui.is_rect_visible(rect) {
            let animation_id = egui::Id::new("toggle_switch_animation").with(response.id);
            let t = ui.ctx().animate_bool(animation_id, self.enabled);

            let bg = lerp_color(colors.surface_active, colors.accent, t);
            ui.painter().rect_filled(rect, CornerRadius::same(10), bg);

            let knob_x = egui::lerp((rect.left() + 10.0)..=(rect.right() - 10.0), t);
            let knob = lerp_color(colors.fg, colors.bg, t);
            ui.painter()
                .circle_filled(egui::pos2(knob_x, rect.center().y), 8.0, knob);
        }

        response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        if let Some(hover_text) = self.hover_text {
            response = crate::theme::hover_text(response, hover_text);
        }

        response
    }
}
