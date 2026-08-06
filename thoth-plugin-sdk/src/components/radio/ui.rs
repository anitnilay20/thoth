use egui::{Sense, Stroke, Vec2};

use crate::theme::{FONT_CONTROL, ThemeColors};

use super::Radio;

/// Circle side — design `.rd { width:16px; height:16px }`.
const CIRCLE_SIZE: f32 = 16.0;
/// Ring thickness — design `.rd { box-shadow: inset 0 0 0 2px … }`. The ring is
/// *inset*, so it must not grow the 16px control.
const RING_WIDTH: f32 = 2.0;
/// Selected-dot diameter — design `.rd.on::after { width:7px; height:7px }`.
const DOT_DIAMETER: f32 = 7.0;
/// Gap between the circle and its label — design `.opt-lbl { gap:8px }`.
const LABEL_GAP: f32 = 8.0;
/// Opacity of a disabled group. The design sheet has no disabled radio variant;
/// match the switch (`.switch.dis { opacity:.4 }`).
const DISABLED_OPACITY: f32 = 0.4;

impl Radio {
    /// Render the radio group, updating [`value`](Radio::value) in place.
    /// Returns `Some(value)` when the selection changed this frame.
    pub fn show(&mut self, ui: &mut egui::Ui) -> Option<String> {
        let colors = ThemeColors::from_ctx(ui.ctx());
        let interactive = !self.disabled && ui.is_enabled();

        let mut changed = None;
        ui.vertical(|ui| {
            if !self.label.is_empty() {
                ui.label(&self.label);
            }
            ui.horizontal(|ui| {
                for opt in &self.options {
                    let selected = self.value == opt.value;
                    let response = option(ui, &opt.label, selected, &colors, interactive);
                    if response.clicked() && !selected {
                        changed = Some(opt.value.clone());
                    }
                }
            });
        });
        if let Some(v) = &changed {
            self.value = v.clone();
        }
        changed
    }
}

/// Paint one `circle + label` option and return its response.
fn option(
    ui: &mut egui::Ui,
    label: &str,
    selected: bool,
    colors: &ThemeColors,
    interactive: bool,
) -> egui::Response {
    let galley = ui.painter().layout_no_wrap(
        label.to_owned(),
        egui::FontId::proportional(FONT_CONTROL),
        colors.fg,
    );
    let label_w = if label.is_empty() {
        0.0
    } else {
        LABEL_GAP + galley.size().x
    };
    let desired = Vec2::new(CIRCLE_SIZE + label_w, galley.size().y.max(CIRCLE_SIZE));

    // The label is part of the hit area — design wraps both in one `.opt-lbl`.
    let (rect, mut response) = ui.allocate_exact_size(
        desired,
        if interactive {
            Sense::click()
        } else {
            Sense::hover()
        },
    );

    if ui.is_rect_visible(rect) {
        // Disabled groups fade through a *cloned* painter so the opacity never
        // leaks into the widgets painted after this one.
        let mut painter = ui.painter().clone();
        if !interactive {
            painter.multiply_opacity(DISABLED_OPACITY);
        }

        let center = egui::pos2(rect.left() + CIRCLE_SIZE / 2.0, rect.center().y);
        // Stroke centre-line pulled in by half the ring width keeps the ring
        // wholly inside the 16px bounds, matching the CSS inset shadow.
        let ring_radius = CIRCLE_SIZE / 2.0 - RING_WIDTH / 2.0;
        let ring = if selected {
            colors.accent
        } else {
            colors.surface_raised
        };
        painter.circle_stroke(center, ring_radius, Stroke::new(RING_WIDTH, ring));
        if selected {
            painter.circle_filled(center, DOT_DIAMETER / 2.0, colors.accent);
        }

        if !label.is_empty() {
            let text_pos = egui::pos2(
                center.x + CIRCLE_SIZE / 2.0 + LABEL_GAP,
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
