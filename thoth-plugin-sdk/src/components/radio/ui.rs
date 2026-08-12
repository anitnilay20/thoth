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
                // Which option holds focus is tracked across frames: egui applies
                // an arrow key's focus move on the *next* pass, so the move can
                // only be spotted by comparing indices.
                let focus_id = ui.make_persistent_id((self.id.as_str(), "radio_focus"));
                let was_focused: Option<usize> = ui.ctx().data(|d| d.get_temp(focus_id)).flatten();
                let mut focused: Option<usize> = None;
                // Tab hands focus on within this same pass; it walks *out* of the
                // group rather than choosing inside it, so it must not select.
                let tab_nav = ui.input(|i| i.key_pressed(egui::Key::Tab));

                for (i, opt) in self.options.iter().enumerate() {
                    let selected = self.value == opt.value;
                    let response = option(ui, &opt.label, selected, &colors, interactive);
                    if response.clicked() && !selected {
                        changed = Some(opt.value.clone());
                    }
                    if response.has_focus() {
                        focused = Some(i);
                    }
                    // Arrow keys move focus *within* the group, and a radio group
                    // carries its selection along with it. Focus arriving from
                    // outside (Tab) leaves the selection alone, hence the check
                    // that focus was already on another option.
                    if response.gained_focus()
                        && !selected
                        && !tab_nav
                        && was_focused.is_some_and(|prev| prev != i)
                    {
                        changed = Some(opt.value.clone());
                    }
                }

                if focused != was_focused {
                    ui.ctx().data_mut(|d| d.insert_temp(focus_id, focused));
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
    // Laid out only when there's something to lay out; an icon-only option is
    // sized by its circle alone.
    let galley = (!label.is_empty()).then(|| {
        ui.painter().layout_no_wrap(
            label.to_owned(),
            egui::FontId::proportional(FONT_CONTROL),
            colors.fg,
        )
    });
    let label_w = galley.as_ref().map_or(0.0, |g| LABEL_GAP + g.size().x);
    let text_h = galley.as_ref().map_or(CIRCLE_SIZE, |g| g.size().y);
    let desired = Vec2::new(CIRCLE_SIZE + label_w, text_h.max(CIRCLE_SIZE));

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

        if let Some(galley) = galley {
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
