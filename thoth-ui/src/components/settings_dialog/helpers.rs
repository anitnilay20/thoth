// Shared building blocks for the settings panes — design `.shead`, `.grouplabel`
// and `.srow`. Rows are flat: no card frame, just a hairline along the bottom
// edge of each row.

use eframe::egui;

use crate::theme::{ThemeColors, phosphor_font_id};
use thoth_plugin_sdk::components::{Separator, Slider, Typography, TypographyVariant};
use thoth_plugin_sdk::theme::{FIELD_HEIGHT, RADIUS_PANEL, color_to_hex, with_alpha};

// ── Section head — design `.shead` ───────────────────────────────────────────

/// Icon tile beside the section title — design `.shead .si{width:34px}`.
const TILE: f32 = 34.0;
/// …with an 18px glyph centred in it — design `.shead .si{font-size:18px}`.
const TILE_GLYPH: f32 = 18.0;
/// Tile tint — design `.si{background:color-mix(mauve 14%,transparent)}`.
const TILE_TINT_ALPHA: u8 = 36; // 14% of 255
/// Gap between the tile and the title block — design `.shead{gap:11px}`.
const HEAD_GAP: f32 = 11.0;
/// Space below the whole head — design `.shead{margin-bottom:16px}`.
const HEAD_BOTTOM: f32 = 16.0;
/// Title size — design `.shead .st{font-size:15px;font-weight:600}`.
const HEAD_TITLE: f32 = 15.0;

// ── Group label — design `.grouplabel` ───────────────────────────────────────

/// Space above a group label — design `.grouplabel{margin:14px 0 7px}`.
const GROUP_TOP: f32 = 14.0;
/// …and below it.
const GROUP_BOTTOM: f32 = 7.0;

// ── Setting row — design `.srow` ─────────────────────────────────────────────

/// Vertical padding — design `.srow{padding:11px 0}`.
const ROW_PAD_V: f32 = 11.0;
/// Gap between the label block and the control — design `.srow{gap:16px}`.
const ROW_GAP: f32 = 16.0;
/// Description size — design `.srow .rd{font-size:11.5px}`.
const ROW_DESC: f32 = 11.5;
/// Description offset under the label — design `.srow .rd{margin-top:2px}`.
const ROW_DESC_GAP: f32 = 2.0;
/// Bottom hairline — design `.srow{box-shadow:inset 0 -1px 0 surface1@22%}`.
const ROW_RULE_ALPHA: u8 = 56; // 22% of 255
/// Width reserved for the right-hand control column. The design lets `.ctl` size
/// itself and `.lt` take the rest, which egui can't resolve in one pass, so the
/// control side gets a fixed slot instead.
const ROW_CONTROL_W: f32 = 220.0;
/// Unsaved-change marker — design `.nitem .dot{width:6px}`.
const DIRTY_DOT_RADIUS: f32 = 3.0;
/// Gap between a label and its dirty dot.
const DIRTY_DOT_GAP: f32 = 4.0;

/// Slider cell width — design `.sldr{width:150px}` plus the SDK slider's own
/// 12px gap and 34px value readout.
const SLIDER_W: f32 = 196.0;

/// A small accent dot marking a value that differs from the baseline.
pub fn dirty_dot(ui: &mut egui::Ui, colors: &ThemeColors) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(
            DIRTY_DOT_RADIUS * 2.0 + DIRTY_DOT_GAP,
            DIRTY_DOT_RADIUS * 2.0,
        ),
        egui::Sense::hover(),
    );
    ui.painter()
        .circle_filled(rect.center(), DIRTY_DOT_RADIUS, colors.accent);
}

/// Render a pane's head: an accent icon tile, the section title, and a muted
/// one-line description under it — design `.shead`.
pub fn section_header(
    ui: &mut egui::Ui,
    icon: &str,
    title: &str,
    subtitle: &str,
    colors: &ThemeColors,
) {
    ui.horizontal_top(|ui| {
        // Every gap here is explicit — design `.shead{gap:11px}`.
        ui.spacing_mut().item_spacing.x = 0.0;

        let (tile, _) = ui.allocate_exact_size(egui::vec2(TILE, TILE), egui::Sense::hover());
        ui.painter().rect_filled(
            tile,
            egui::CornerRadius::from(RADIUS_PANEL),
            with_alpha(colors.accent, TILE_TINT_ALPHA),
        );
        ui.painter().text(
            tile.center(),
            egui::Align2::CENTER_CENTER,
            icon,
            phosphor_font_id(TILE_GLYPH),
            colors.accent,
        );

        ui.add_space(HEAD_GAP);
        ui.vertical(|ui| {
            ui.add(
                Typography::builder()
                    .text(title)
                    .variant(TypographyVariant::Heading)
                    .size(HEAD_TITLE)
                    .build(),
            );
            if !subtitle.is_empty() {
                ui.add_space(ROW_DESC_GAP);
                Typography::body_muted(ui, subtitle);
            }
        });
    });
    ui.add_space(HEAD_BOTTOM);
}

/// Label a run of related setting rows — design `.grouplabel`. Rows are flat, so
/// this only prints the label and spaces the rows beneath it.
pub fn group_rows(ui: &mut egui::Ui, title: &str, content: impl FnOnce(&mut egui::Ui)) {
    ui.add_space(GROUP_TOP);
    Typography::group_label(ui, title);
    ui.add_space(GROUP_BOTTOM);
    // Rows abut: their own padding is the whole gap, so the hairline between two
    // of them lands exactly on the boundary.
    ui.vertical(|ui| {
        ui.spacing_mut().item_spacing.y = 0.0;
        content(ui);
    });
}

/// Render a setting row: label (plus optional description and error) on the
/// left, the control right-aligned, and a hairline along the bottom edge —
/// design `.srow`.
///
/// `dirty = true` shows an accent dot next to the label.
pub fn setting_row(
    ui: &mut egui::Ui,
    label: &str,
    hint: Option<&str>,
    dirty: bool,
    error: Option<&str>,
    colors: &ThemeColors,
    content: impl FnOnce(&mut egui::Ui),
) {
    let row = egui::Frame::new()
        .inner_margin(egui::Margin::symmetric(0, ROW_PAD_V as i8))
        .show(ui, |ui| {
            // Always span the pane's content column so the hairline below lines
            // up from row to row.
            ui.set_min_width(ui.available_width());
            let label_w = (ui.available_width() - ROW_CONTROL_W - ROW_GAP).max(0.0);
            let has_extra = hint.is_some() || error.is_some();

            // The label block: 13px label, then the 11.5px muted description.
            let label_block = |ui: &mut egui::Ui| {
                ui.horizontal(|ui| {
                    Typography::body_large(ui, label);
                    if dirty {
                        dirty_dot(ui, colors);
                    }
                });
                if let Some(h) = hint {
                    ui.add_space(ROW_DESC_GAP);
                    ui.add(
                        Typography::builder()
                            .text(h)
                            .variant(TypographyVariant::Caption)
                            .size(ROW_DESC)
                            .build(),
                    );
                }
                if let Some(e) = error {
                    ui.horizontal(|ui| {
                        ui.label(
                            crate::theme::icon_rich_text(egui_phosphor::regular::WARNING, ROW_DESC)
                                .color(colors.error),
                        );
                        ui.add_space(2.0);
                        ui.add(
                            Typography::builder()
                                .text(e)
                                .variant(TypographyVariant::Caption)
                                .size(ROW_DESC)
                                .color(thoth_plugin_sdk::theme::color_to_hex(colors.error))
                                .build(),
                        );
                    });
                }
            };

            if has_extra {
                // Multi-line rows grow to fit; the control centres against the block.
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_min_width(label_w);
                        label_block(ui);
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), content);
                });
            } else {
                // Single-line rows get a control-height row so the label centres
                // against the widget beside it.
                ui.allocate_ui_with_layout(
                    egui::vec2(ui.available_width(), FIELD_HEIGHT),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        ui.scope(|ui| {
                            ui.set_min_width(label_w);
                            label_block(ui);
                        });
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), content);
                    },
                );
            }
        });

    // Bottom hairline — design `.srow{box-shadow:inset 0 -1px 0 …}`. Painted
    // rather than added: the row is already laid out, and a separator that
    // claimed a point of height would push the next row down.
    let r = row.response.rect;
    Separator::rule(color_to_hex(with_alpha(
        colors.surface_raised,
        ROW_RULE_ALPHA,
    )))
    .paint_at(ui, r.x_range(), r.bottom());
}

/// Render an SDK [`Slider`] at the design's fixed track width, with the unit
/// suffix after the value readout — design `.sldr` + `.pfield .val`.
///
/// Call from inside a [`setting_row`] control closure (a right-to-left layout,
/// so the unit is added first to land to the right of the track). Returns the
/// new value when the user moves the slider.
pub fn slider_control(
    ui: &mut egui::Ui,
    value: f64,
    min: f64,
    max: f64,
    unit: &str,
) -> Option<f64> {
    if !unit.is_empty() {
        Typography::caption(ui, unit);
    }

    let mut changed = None;
    ui.allocate_ui(egui::vec2(SLIDER_W, FIELD_HEIGHT), |ui| {
        let mut slider = Slider::builder().value(value).min(min).max(max).build();
        if slider.show(ui).changed() {
            changed = Some(slider.value);
        }
    });
    changed
}
