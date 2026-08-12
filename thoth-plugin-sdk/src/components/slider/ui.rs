use egui::{CornerRadius, Rect, Sense, Stroke, Vec2};

use crate::theme::{RADIUS_PILL, ThemeColors, slider_thumb_shadow, with_alpha};

use super::Slider;

/// Track thickness — design `input[type=range] { height:5px }`.
const TRACK_HEIGHT: f32 = 5.0;
/// Thumb diameter — design `::-webkit-slider-thumb { width:15px; height:15px }`.
const THUMB_DIAMETER: f32 = 15.0;
/// Halo ring around the thumb — design thumb `box-shadow: 0 0 0 3px mauve@30%`.
const HALO_WIDTH: f32 = 3.0;
/// 30% of 255 — the halo's accent alpha.
const HALO_ALPHA: u8 = 77;
/// Value readout — design `.sval { font:500 12px mono; min-width:34px }`.
const READOUT_FONT: f32 = 12.0;
/// Minimum readout width, so the track doesn't twitch as digits change.
const READOUT_MIN_WIDTH: f32 = 34.0;
/// Gap between track and readout — design `.row { gap:12px }` on the slider row.
const READOUT_GAP: f32 = 12.0;
/// Opacity of a disabled slider. The design sheet has no disabled slider
/// variant; match the switch (`.switch.dis { opacity:.4 }`).
const DISABLED_OPACITY: f32 = 0.4;

impl Slider {
    /// Render the slider, editing [`value`](Slider::value) in place.
    pub fn show(&mut self, ui: &mut egui::Ui) -> egui::Response {
        let colors = ThemeColors::from_ctx(ui.ctx());
        let interactive = !self.disabled && ui.is_enabled();

        if !self.label.is_empty() {
            ui.label(&self.label);
        }

        ui.horizontal(|ui| {
            // The 12px track↔readout gap is spaced explicitly, so drop the
            // layout's own item spacing and keep the geometry exact.
            ui.spacing_mut().item_spacing.x = 0.0;

            // Only the readout's *slot* is measured up front — the galley itself is
            // laid out at paint time, after the drag handler below has moved
            // `self.value`, so the number never lags a frame behind the thumb. The
            // slot is measured from the range's endpoints (the widest the readout
            // can get) rather than the current value, so the track keeps a stable
            // width as digits come and go.
            let span = self.max - self.min;
            let readout_w = [self.min, self.max]
                .into_iter()
                .map(|v| {
                    ui.painter()
                        .layout_no_wrap(
                            format_value(v, span),
                            egui::FontId::monospace(READOUT_FONT),
                            colors.fg_subtle(),
                        )
                        .size()
                        .x
                })
                .fold(READOUT_MIN_WIDTH, f32::max);
            // The halo sits outside the thumb, so the row must be tall enough for both.
            let row_h = THUMB_DIAMETER + 2.0 * HALO_WIDTH;
            let track_w = (ui.available_width() - readout_w - READOUT_GAP).max(THUMB_DIAMETER);

            let (rect, mut response) = ui.allocate_exact_size(
                Vec2::new(track_w, row_h),
                if interactive {
                    Sense::click_and_drag()
                } else {
                    Sense::hover()
                },
            );

            // The thumb centre stays a half-thumb inside each end, so the usable
            // travel is narrower than the track itself.
            let half = THUMB_DIAMETER / 2.0;
            let (x0, x1) = (rect.left() + half, rect.right() - half);

            if (response.clicked() || response.dragged())
                && let Some(pos) = response.interact_pointer_pos()
            {
                let t = ((pos.x - x0) / (x1 - x0).max(1.0)).clamp(0.0, 1.0);
                // Snap to the precision the readout shows, so the value the plugin
                // receives is the number the user read off the slider.
                let scale = 10f64.powi(decimals_for(span) as i32);
                let value = ((self.min + t as f64 * span) * scale).round() / scale;
                if value != self.value {
                    self.value = value;
                    response.mark_changed();
                }
            }

            // Keyboard control. `Sense::click_and_drag` already makes the track
            // focusable, so without this a keyboard user can tab to the slider but
            // never move it. One arrow press steps by the readout's own precision;
            // Home/End jump to the ends.
            if interactive && response.has_focus() {
                let step = span.signum() * 10f64.powi(-(decimals_for(span) as i32));
                let (lo, hi) = if self.min <= self.max {
                    (self.min, self.max)
                } else {
                    (self.max, self.min)
                };
                let target = ui.input(|i| {
                    use egui::Key;
                    if i.key_pressed(Key::ArrowLeft) || i.key_pressed(Key::ArrowDown) {
                        Some(self.value - step)
                    } else if i.key_pressed(Key::ArrowRight) || i.key_pressed(Key::ArrowUp) {
                        Some(self.value + step)
                    } else if i.key_pressed(Key::Home) {
                        Some(self.min)
                    } else if i.key_pressed(Key::End) {
                        Some(self.max)
                    } else {
                        None
                    }
                });
                if let Some(value) = target {
                    let value = value.clamp(lo, hi);
                    if value != self.value {
                        self.value = value;
                        response.mark_changed();
                    }
                }
            }
            // Assistive technologies read the value and label from here.
            response.widget_info(|| {
                egui::WidgetInfo::slider(interactive, self.value, self.label.as_str())
            });

            // Right-aligned readout in its own slot, so the track keeps a stable
            // width regardless of the value's digit count.
            ui.add_space(READOUT_GAP);
            let (readout_rect, _) =
                ui.allocate_exact_size(Vec2::new(readout_w, row_h), Sense::hover());

            if ui.is_rect_visible(rect) {
                // Disabled sliders fade through a *cloned* painter so the opacity
                // never leaks into the widgets painted after this one.
                let mut painter = ui.painter().clone();
                if !interactive {
                    painter.multiply_opacity(DISABLED_OPACITY);
                }

                let t = if span.abs() > f64::EPSILON {
                    ((self.value - self.min) / span).clamp(0.0, 1.0) as f32
                } else {
                    0.0
                };
                let thumb_center = egui::pos2(egui::lerp(x0..=x1, t), rect.center().y);
                // Laid out here, from the value the drag handler just wrote.
                let readout = painter.layout_no_wrap(
                    format_value(self.value, span),
                    egui::FontId::monospace(READOUT_FONT),
                    colors.fg_subtle(),
                );

                // `RADIUS_PILL` saturates to `u8::MAX`; the tessellator clamps the
                // corner radius to half the height, which is the full pill we want.
                let pill = CornerRadius::same(RADIUS_PILL as u8);
                let track =
                    Rect::from_center_size(rect.center(), Vec2::new(rect.width(), TRACK_HEIGHT));
                painter.rect_filled(track, pill, colors.surface_raised);
                let filled = Rect::from_min_max(track.min, egui::pos2(thumb_center.x, track.max.y));
                painter.rect_filled(filled, pill, colors.accent);

                // Thumb drop shadow — design `0 2px 6px black@45%`.
                painter.add(slider_thumb_shadow().as_shape(
                    Rect::from_center_size(thumb_center, Vec2::splat(THUMB_DIAMETER)),
                    pill,
                ));
                painter.circle_stroke(
                    thumb_center,
                    half + HALO_WIDTH / 2.0,
                    Stroke::new(HALO_WIDTH, with_alpha(colors.accent, HALO_ALPHA)),
                );
                painter.circle_filled(thumb_center, half, colors.fg);

                let text_pos = egui::pos2(
                    readout_rect.right() - readout.size().x,
                    readout_rect.center().y - readout.size().y / 2.0,
                );
                // Faux medium weight: a second pass shifted 0.5px right thickens
                // the vertical strokes (design `.sval { font-weight:500 }`).
                painter.galley(
                    text_pos + Vec2::new(0.5, 0.0),
                    readout.clone(),
                    colors.fg_subtle(),
                );
                painter.galley(text_pos, readout, colors.fg_subtle());
            }

            if interactive {
                response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
            }

            response
        })
        .inner
    }
}

/// Decimal places the readout shows for a range of `span`: wide ranges read as
/// whole numbers, narrow ones keep the decimals that make dragging legible. Also
/// the precision the dragged value is snapped to, so the two can't disagree.
fn decimals_for(span: f64) -> usize {
    if span.abs() >= 10.0 {
        0
    } else if span.abs() >= 1.0 {
        2
    } else {
        3
    }
}

/// Format the readout at [`decimals_for`]'s precision.
fn format_value(value: f64, span: f64) -> String {
    let decimals = decimals_for(span);
    format!("{value:.decimals$}")
}
