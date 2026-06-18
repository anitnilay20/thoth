use eframe::egui::{self, Color32, RichText};

use thoth_plugin_sdk::components::Typography;
use crate::theme::{
    CARD_OUTER_H, CARD_RADIUS, CONTROL_WIDTH, DIRTY_DOT_RADIUS, GROUP_SPACING, ROW_INNER_H,
    ROW_PADDING_H, ROW_PADDING_V, ThemeColors, icon_rich_text,
};

pub fn dirty_dot(ui: &mut egui::Ui, colors: &ThemeColors) {
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(DIRTY_DOT_RADIUS * 2.0 + 4.0, DIRTY_DOT_RADIUS * 2.0),
        egui::Sense::hover(),
    );
    ui.painter()
        .circle_filled(rect.center(), DIRTY_DOT_RADIUS, colors.accent);
}

pub fn section_header(
    ui: &mut egui::Ui,
    icon: &str,
    title: &str,
    subtitle: &str,
    colors: &ThemeColors,
) {
    ui.add_space(24.0);
    ui.horizontal(|ui| {
        ui.add_space(ROW_PADDING_H);
        ui.label(icon_rich_text(icon, 20.0).color(colors.fg));
        ui.add_space(10.0);
        ui.vertical(|ui| {
            Typography::heading(ui, title);
            if !subtitle.is_empty() {
                ui.add_space(2.0);
                Typography::body_muted(ui, subtitle);
            }
        });
    });
    ui.add_space(20.0);
    // Separator inset by CARD_OUTER_H so it aligns with the card edges below
    let r = ui.available_rect_before_wrap();
    ui.painter().hline(
        egui::Rangef::new(r.left() + CARD_OUTER_H, r.right() - CARD_OUTER_H),
        ui.cursor().top(),
        egui::Stroke::new(1.0, colors.surface_raised),
    );
    ui.add_space(1.0);
}

/// Render a titled card that wraps setting rows.
/// Rows drawn inside the closure share borders and rounded corners.
/// Pass a stable `id` string unique to this group (used to track the first-row flag).
pub fn group_rows(
    ui: &mut egui::Ui,
    title: &str,
    id: &str,
    colors: &ThemeColors,
    content: impl FnOnce(&mut egui::Ui),
) {
    // Group title (small-caps label above the card)
    ui.add_space(GROUP_SPACING);
    ui.horizontal(|ui| {
        ui.add_space(ROW_PADDING_H);
        Typography::group_label(ui, title);
    });
    ui.add_space(6.0);

    // Reset "first row" flag so the first setting_row inside this card omits its top separator
    let flag_id = egui::Id::new(("group_first_row", id));
    ui.ctx().data_mut(|d| d.insert_temp(flag_id, true));

    // Card frame: bg_panel fill, 1px surface border, 8px radius, indented by CARD_OUTER_H
    let stroke_color = colors.surface_raised;
    egui::Frame::new()
        .fill(colors.bg_panel)
        .stroke(egui::Stroke::new(1.0, stroke_color))
        .corner_radius(egui::CornerRadius::same(CARD_RADIUS as u8))
        .outer_margin(egui::Margin {
            left: CARD_OUTER_H as i8,
            right: CARD_OUTER_H as i8,
            top: 0,
            bottom: 0,
        })
        .inner_margin(egui::Margin::ZERO)
        .show(ui, |ui| {
            // Store the flag id so setting_row can find it
            ui.ctx()
                .data_mut(|d| d.insert_temp(egui::Id::new("current_group_flag"), flag_id));
            content(ui);
        });
}

/// Render a two-column setting row inside a `group_rows` card.
///
/// Automatically draws a top separator between consecutive rows (not before the first).
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
    // Draw top separator for every row except the first in the group
    let flag_id: Option<egui::Id> = ui
        .ctx()
        .data(|d| d.get_temp(egui::Id::new("current_group_flag")));

    if let Some(fid) = flag_id {
        let is_first: bool = ui.ctx().data(|d| d.get_temp(fid).unwrap_or(false));
        if !is_first {
            let sep_y = ui.cursor().top();
            // Use the frame's available rect, not clip_rect, so the line stays
            // within the card's visual bounds and respects its corner radius.
            let r = ui.available_rect_before_wrap();
            ui.painter().hline(
                egui::Rangef::new(r.left(), r.right()),
                sep_y,
                egui::Stroke::new(1.0, colors.surface_raised),
            );
        } else {
            ui.ctx().data_mut(|d| d.insert_temp(fid, false));
        }
    }

    egui::Frame::new()
        .fill(Color32::TRANSPARENT)
        .inner_margin(egui::Margin {
            left: ROW_INNER_H as i8,
            right: ROW_INNER_H as i8,
            top: ROW_PADDING_V as i8,
            bottom: ROW_PADDING_V as i8,
        })
        .show(ui, |ui| {
            let available_w = ui.available_width();
            let label_width = (available_w - CONTROL_WIDTH - 8.0).max(0.0);
            let has_extra = hint.is_some() || error.is_some();

            if has_extra {
                // Multi-line rows: auto-height, label top-aligned, hint stacks below.
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.set_min_width(label_width);
                        ui.horizontal(|ui| {
                            Typography::body_large(ui, label);
                            if dirty {
                                ui.add_space(4.0);
                                dirty_dot(ui, colors);
                            }
                        });
                        if let Some(h) = hint {
                            ui.add_space(1.0);
                            Typography::caption(ui, h);
                        }
                        if let Some(e) = error {
                            ui.horizontal(|ui| {
                                ui.label(
                                    icon_rich_text(egui_phosphor::regular::WARNING, 11.0)
                                        .color(colors.error),
                                );
                                ui.add_space(2.0);
                                ui.label(RichText::new(e).size(11.0).color(colors.error));
                            });
                        }
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        content(ui);
                    });
                });
            } else {
                // Single-line rows: allocate a finite row height so Align::Center
                // actually centres the label text with the control widget.
                let row_h = ui.text_style_height(&egui::TextStyle::Body) + 8.0;
                ui.allocate_ui_with_layout(
                    egui::vec2(available_w, row_h),
                    egui::Layout::left_to_right(egui::Align::Center),
                    |ui| {
                        // Label side
                        ui.scope(|ui| {
                            ui.set_min_width(label_width);
                            Typography::body_large(ui, label);
                            if dirty {
                                ui.add_space(4.0);
                                dirty_dot(ui, colors);
                            }
                        });
                        // Control side
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            content(ui);
                        });
                    },
                );
            }
        });
}
