use egui::{Color32, CornerRadius, Rect, Response, Sense, Vec2, Widget};

use crate::theme::{RADIUS_PILL, ThemeColors, thumb_shadow};

use super::ToggleSwitch;

/// Track size — design `.switch { width:32px; height:19px }`.
const TRACK_SIZE: Vec2 = Vec2::new(32.0, 19.0);
/// Thumb diameter — design `.switch::after { width:13px; height:13px }`.
const THUMB_DIAMETER: f32 = 13.0;
/// Thumb inset from the track edge, on all four sides — design
/// `.switch::after { top:3px; left:3px }` / `.switch.on::after { left:16px }`.
const THUMB_INSET: f32 = 3.0;
/// Opacity of a disabled switch — design `.switch.dis { opacity:.4 }`.
const DISABLED_OPACITY: f32 = 0.4;

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
        let interactive = !self.disabled && ui.is_enabled();

        let (rect, mut response) = ui.allocate_exact_size(
            TRACK_SIZE,
            if interactive {
                Sense::click()
            } else {
                Sense::hover()
            },
        );

        if ui.is_rect_visible(rect) {
            let animation_id = egui::Id::new("toggle_switch_animation").with(response.id);
            let t = ui.ctx().animate_bool(animation_id, self.enabled);

            // Disabled switches fade through a *cloned* painter so the opacity
            // never leaks into the widgets painted after this one.
            let mut painter = ui.painter().clone();
            if !interactive {
                painter.multiply_opacity(DISABLED_OPACITY);
            }

            // `RADIUS_PILL` saturates to `u8::MAX`; the tessellator clamps the
            // corner radius to half the height, which is the full pill we want.
            let track_radius = CornerRadius::same(RADIUS_PILL as u8);
            let track = lerp_color(colors.surface_raised, colors.accent, t);
            painter.rect_filled(rect, track_radius, track);

            // The thumb slides between a 3px inset on the leading and trailing
            // side, so its travel is the track width minus both insets and itself.
            let travel = TRACK_SIZE.x - THUMB_DIAMETER - 2.0 * THUMB_INSET;
            let thumb_rect = Rect::from_min_size(
                rect.min + Vec2::splat(THUMB_INSET) + Vec2::new(travel * t, 0.0),
                Vec2::splat(THUMB_DIAMETER),
            );
            painter.add(thumb_shadow().as_shape(thumb_rect, CornerRadius::same(RADIUS_PILL as u8)));
            let thumb = lerp_color(colors.fg, colors.bg_sunken, t);
            painter.circle_filled(thumb_rect.center(), THUMB_DIAMETER / 2.0, thumb);
        }

        if interactive {
            response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        }
        if let Some(hover_text) = self.hover_text {
            response = crate::theme::hover_text(response, hover_text);
        }

        response
    }
}
