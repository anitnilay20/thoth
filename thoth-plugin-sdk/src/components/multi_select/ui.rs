use crate::components::select::ui::{paint_trigger, paint_truncated, show_popover};
use crate::theme::{
    FONT_CONTROL, RADIUS_CHECK, RADIUS_CHIP, ThemeColors, edge_stroke, get_contrast_text_color,
    phosphor_font_id, with_alpha,
};

use super::MultiSelect;

// ── Design metrics ────────────────────────────────────────────────────────────

/// Option row height — design the multi-select `.popover` rows (32px).
const ROW_HEIGHT: f32 = 32.0;
/// Option row horizontal padding — design `padding:0 6px`.
const ROW_PAD_X: f32 = 6.0;
/// Gap between the checkbox and its label — design `.opt-lbl{gap:8px}`.
const ROW_GAP: f32 = 8.0;
/// Checkbox side — design `.cb{width:16px;height:16px}`.
const CB_SIZE: f32 = 16.0;
/// Checkbox glyph size — design `.cb{font-size:11px}`.
const CB_GLYPH: f32 = 11.0;
/// Row-hover wash, matching the single select's `.opt:hover` (text@7%).
const HOVER_ALPHA: u8 = 18;
/// Rows shown before the list starts scrolling.
const MAX_VISIBLE: usize = 8;

impl MultiSelect {
    /// Render the dropdown, updating [`value`](MultiSelect::value) in place.
    /// Returns `true` if the selection changed this frame.
    pub fn show(&mut self, ui: &mut egui::Ui) -> bool {
        let colors = ThemeColors::from_ctx(ui.ctx());
        let (font_size, trigger_h) = self.size.field_metrics();
        let mut changed = false;

        ui.add_enabled_ui(!self.disabled, |ui| {
            ui.vertical(|ui| {
                if !self.label.is_empty() {
                    ui.label(&self.label);
                }

                // Derived from the `ui` (not a global `Id::new`) so two instances
                // sharing a string id keep distinct open/closed state.
                let id = ui.make_persistent_id(&self.id);
                let mut is_open: bool = ui.ctx().data(|d| d.get_temp(id).unwrap_or(false));

                // ── Trigger ───────────────────────────────────────────────────
                let trigger_w = self.width.unwrap_or_else(|| ui.available_width());
                let (trigger_rect, trigger_resp) = paint_trigger(
                    ui,
                    &colors,
                    egui::vec2(trigger_w, trigger_h),
                    font_size,
                    &self.summary(),
                    is_open,
                    // No leading glyph on a multi-select trigger — its label is a
                    // selection count, not a named value.
                    None,
                );

                if trigger_resp.clicked() {
                    is_open = !is_open;
                    ui.ctx().data_mut(|d| d.insert_temp(id, is_open));
                }
                if !is_open {
                    return;
                }

                // ── Popover ───────────────────────────────────────────────────
                let scroll_h = ROW_HEIGHT * MAX_VISIBLE.min(self.options.len().max(1)) as f32;
                let text_color = if ui.is_enabled() {
                    colors.fg
                } else {
                    colors.fg_faint()
                };

                // Collected rather than applied inline: the row loop borrows
                // `options`, so `value` can only be edited once it's done.
                let mut toggled: Option<String> = None;

                let (popover_resp, ()) = show_popover(
                    ui,
                    id.with("_area"),
                    trigger_rect,
                    &colors,
                    |ui, popup_w| {
                        egui::ScrollArea::vertical()
                            .max_height(scroll_h)
                            .auto_shrink([false, true])
                            .show(ui, |ui| {
                                ui.set_min_width(popup_w);
                                ui.spacing_mut().item_spacing.y = 0.0;
                                for opt in &self.options {
                                    let on = self.value.contains(&opt.value);
                                    if row(ui, &colors, &opt.label, on, text_color) {
                                        toggled = Some(opt.value.clone());
                                    }
                                }
                            });
                    },
                );

                if let Some(value) = toggled {
                    changed = true;
                    if self.value.contains(&value) {
                        self.value.retain(|v| v != &value);
                    } else {
                        self.value.push(value);
                    }
                }

                let escape = ui.ctx().input(|i| i.key_pressed(egui::Key::Escape));
                let interact_pos = ui
                    .ctx()
                    .input(|i| i.pointer.interact_pos())
                    .unwrap_or_default();
                // Toggling a row keeps the popover open, so it only closes on a
                // click outside it and the trigger — or on Escape.
                let click_outside =
                    popover_resp.clicked_elsewhere() && !trigger_rect.contains(interact_pos);
                if escape || click_outside {
                    ui.ctx().data_mut(|d| d.insert_temp::<bool>(id, false));
                }
            });
        });

        changed
    }

    /// The trigger label: a count of what's selected, e.g. `"2 columns"`.
    fn summary(&self) -> String {
        let n = self.value.len();
        match &self.item_noun {
            Some(noun) if n == 1 => format!("1 {noun}"),
            Some(noun) => format!("{n} {noun}s"),
            None if n == 0 => "None selected".to_owned(),
            None => format!("{n} selected"),
        }
    }
}

/// Paint one checkbox row of the popover — design an `.opt-lbl` (checkbox + label,
/// 8px gap) inside a 32px row. Returns `true` when the row was clicked.
fn row(
    ui: &mut egui::Ui,
    colors: &ThemeColors,
    label: &str,
    checked: bool,
    text_color: egui::Color32,
) -> bool {
    let (rect, resp) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), ROW_HEIGHT),
        egui::Sense::click(),
    );

    if ui.is_rect_visible(rect) {
        if resp.hovered() {
            ui.painter()
                .rect_filled(rect, RADIUS_CHIP, with_alpha(colors.fg, HOVER_ALPHA));
        }

        // ── Checkbox — design `.cb` / `.cb.on` ────────────────────────────────
        let cb_rect = egui::Rect::from_center_size(
            egui::pos2(rect.min.x + ROW_PAD_X + CB_SIZE / 2.0, rect.center().y),
            egui::Vec2::splat(CB_SIZE),
        );
        if checked {
            // Checked drops the edge entirely and fills with mauve.
            ui.painter()
                .rect_filled(cb_rect, RADIUS_CHECK, colors.accent);
            ui.painter().text(
                cb_rect.center(),
                egui::Align2::CENTER_CENTER,
                egui_phosphor::regular::CHECK,
                phosphor_font_id(CB_GLYPH),
                get_contrast_text_color(colors.accent),
            );
        } else {
            ui.painter().rect(
                cb_rect,
                RADIUS_CHECK,
                colors.surface,
                edge_stroke(colors),
                egui::StrokeKind::Inside,
            );
        }

        // ── Label ─────────────────────────────────────────────────────────────
        let label_x = cb_rect.max.x + ROW_GAP;
        paint_truncated(
            ui.painter(),
            egui::pos2(label_x, rect.center().y),
            label,
            egui::FontId::proportional(FONT_CONTROL),
            text_color,
            (rect.max.x - ROW_PAD_X - label_x).max(0.0),
        );

        if resp.hovered() {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }
    }

    resp.clicked()
}
