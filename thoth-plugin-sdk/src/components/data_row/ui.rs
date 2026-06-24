use std::sync::Arc;

use egui::{Color32, RichText, Ui, WidgetText, text::LayoutJob};

use crate::components::IconButton;
use crate::theme::{
    ROW_HEIGHT, TextPalette, ThemeColors, hover_row_bg, phosphor_font_id, resolve_color,
};

use super::DataRow;

/// Indentation step per tree depth level, in logical pixels.
const INDENT_STEP: f32 = 16.0;

/// Outcome of rendering a [`DataRow`].
pub struct DataRowOutput {
    /// The row body or its content was clicked.
    pub clicked: bool,
    /// The row was right-clicked (context menu).
    pub right_clicked: bool,
    /// The expand/collapse caret was clicked (takes precedence over `clicked`).
    pub caret_clicked: bool,
    /// The row's interaction response.
    pub response: egui::Response,
}

impl DataRow {
    /// Render the row and report interaction.
    pub fn show(&self, ui: &mut Ui) -> DataRowOutput {
        let palette = TextPalette::from_ctx(ui.ctx());

        let mut parts = self.display_text.splitn(2, ':');
        let key_part = parts.next().unwrap_or("");
        let value_part = parts.next().unwrap_or("");
        let has_colon = !value_part.is_empty() && self.value_token.is_some();

        let id = ui.id().with(&self.row_id);
        let available_rect = ui.available_rect_before_wrap();
        let interact_rect = egui::Rect::from_min_size(
            available_rect.min,
            egui::vec2(ui.available_width(), ROW_HEIGHT),
        );
        let resp = ui.interact(interact_rect, id, egui::Sense::click());

        let colors = ThemeColors::from_ctx(ui.ctx());
        let caller_bg = self
            .background
            .as_deref()
            .and_then(|c| resolve_color(c, &colors))
            .unwrap_or(Color32::TRANSPARENT);
        let base_bg = if self.selected {
            ui.visuals().selection.bg_fill
        } else {
            caller_bg
        };
        let hovered = resp.hovered() || ui.rect_contains_pointer(interact_rect);
        let background = if hovered {
            blend_colors(base_bg, hover_row_bg(ui))
        } else {
            base_bg
        };

        let highlight_bg = ui.visuals().selection.bg_fill;
        let highlight_fg = ui.visuals().strong_text_color();
        let base_text_color = ui.visuals().text_color();
        let muted = ui.visuals().weak_text_color();

        let mut caret_clicked = false;
        let mut body_clicked = false;
        let mut body_secondary = false;

        egui::Frame::new().fill(background).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                if self.indent > 0 {
                    ui.add_space(self.indent as f32 * INDENT_STEP);
                }

                match self.caret {
                    Some(expanded) => {
                        let glyph = if expanded {
                            egui_phosphor::regular::CARET_DOWN
                        } else {
                            egui_phosphor::regular::CARET_RIGHT
                        };
                        let clicked = ui
                            .add(
                                IconButton::builder()
                                    .icon(glyph)
                                    .tooltip(if expanded { "Collapse" } else { "Expand" })
                                    .build(),
                            )
                            .clicked();
                        if clicked {
                            caret_clicked = true;
                        }
                    }
                    None => {
                        // Aligned, invisible spacer so leaf rows line up with caret rows.
                        ui.add_enabled_ui(false, |ui| {
                            ui.visuals_mut().widgets.inactive.bg_fill = Color32::TRANSPARENT;
                            ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                            ui.add(IconButton::builder().icon(" ").build());
                        });
                    }
                }

                if let Some(icon) = &self.leading_icon {
                    let color = icon
                        .color
                        .as_deref()
                        .and_then(|c| resolve_color(c, &colors))
                        .unwrap_or(muted);
                    body_label(
                        ui,
                        RichText::new(&icon.glyph)
                            .font(phosphor_font_id(13.0))
                            .color(color)
                            .into(),
                        false,
                        &mut body_clicked,
                        &mut body_secondary,
                    );
                    ui.add_space(4.0);
                }

                let key_label_text = format!("{}{}", key_part, if has_colon { ":" } else { "" });
                let key_color = palette.color_with_highlighting(
                    self.key_token,
                    self.syntax_highlighting,
                    base_text_color,
                );
                let key_label = highlighted_text(
                    ui,
                    &key_label_text,
                    key_color,
                    &self.highlights.key_ranges,
                    highlight_bg,
                    highlight_fg,
                );
                body_label(ui, key_label, true, &mut body_clicked, &mut body_secondary);

                if let Some(value_token) = self.value_token {
                    let value_color = palette.color_with_highlighting(
                        value_token,
                        self.syntax_highlighting,
                        base_text_color,
                    );
                    let value_label = highlighted_text(
                        ui,
                        value_part,
                        value_color,
                        &self.highlights.value_ranges,
                        highlight_bg,
                        highlight_fg,
                    );
                    body_label(
                        ui,
                        value_label,
                        true,
                        &mut body_clicked,
                        &mut body_secondary,
                    );
                }

                if let Some(trailing) = &self.trailing {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        body_label(
                            ui,
                            RichText::new(trailing).color(muted).size(11.0).into(),
                            false,
                            &mut body_clicked,
                            &mut body_secondary,
                        );
                    });
                }
            });
        });

        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        DataRowOutput {
            clicked: resp.clicked() || body_clicked,
            right_clicked: resp.secondary_clicked() || body_secondary,
            caret_clicked,
            response: resp,
        }
    }
}

/// Add one body label that participates in the row's click.
fn body_label(
    ui: &mut Ui,
    text: WidgetText,
    selectable: bool,
    clicked: &mut bool,
    secondary: &mut bool,
) {
    let label = if selectable {
        egui::Label::new(text).selectable(true)
    } else {
        egui::Label::new(text).sense(egui::Sense::click())
    };
    let resp = ui
        .add(label)
        .on_hover_cursor(egui::CursorIcon::PointingHand);
    if resp.clicked() {
        *clicked = true;
    }
    if resp.secondary_clicked() {
        *secondary = true;
    }
}

/// Composite `overlay` over `background` (source-over alpha blending).
fn blend_colors(background: Color32, overlay: Color32) -> Color32 {
    let bg = background.to_array();
    let ov = overlay.to_array();
    let oa = ov[3] as f32 / 255.0;
    let ba = bg[3] as f32 / 255.0;
    let out_a = oa + ba * (1.0 - oa);
    if out_a <= 0.0 {
        return Color32::TRANSPARENT;
    }
    let chan = |b: u8, o: u8| -> u8 {
        (((o as f32 * oa) + (b as f32 * ba * (1.0 - oa))) / out_a).round() as u8
    };
    Color32::from_rgba_unmultiplied(
        chan(bg[0], ov[0]),
        chan(bg[1], ov[1]),
        chan(bg[2], ov[2]),
        (out_a * 255.0).round() as u8,
    )
}

fn highlighted_text(
    ui: &Ui,
    text: &str,
    base_color: Color32,
    ranges: &[std::ops::Range<usize>],
    highlight_bg: Color32,
    highlight_fg: Color32,
) -> WidgetText {
    if text.is_empty() || ranges.is_empty() {
        return RichText::new(text).monospace().color(base_color).into();
    }

    let mut job = LayoutJob::default();
    let font_size = ui.style().text_styles[&egui::TextStyle::Monospace].size;
    let base_format = egui::TextFormat {
        font_id: egui::FontId::monospace(font_size),
        color: base_color,
        ..Default::default()
    };
    let highlight_format = egui::TextFormat {
        font_id: egui::FontId::monospace(font_size),
        color: highlight_fg,
        background: highlight_bg,
        ..Default::default()
    };

    let mut cursor = 0;
    for range in ranges {
        let start = range.start.min(text.len());
        let end = range.end.min(text.len());
        if start > cursor {
            job.append(&text[cursor..start], 0.0, base_format.clone());
        }
        if start < end {
            job.append(&text[start..end], 0.0, highlight_format.clone());
        }
        cursor = end;
    }
    if cursor < text.len() {
        job.append(&text[cursor..], 0.0, base_format);
    }

    WidgetText::LayoutJob(Arc::new(job))
}

#[cfg(test)]
mod tests {
    use super::blend_colors;
    use egui::Color32;

    #[test]
    fn opaque_overlay_replaces_background() {
        let bg = Color32::from_rgb(10, 20, 30);
        let overlay = Color32::from_rgb(200, 100, 50);
        assert_eq!(blend_colors(bg, overlay), overlay);
    }

    #[test]
    fn fully_transparent_overlay_keeps_background() {
        let bg = Color32::from_rgb(10, 20, 30);
        let overlay = Color32::from_rgba_unmultiplied(255, 0, 0, 0);
        assert_eq!(blend_colors(bg, overlay), bg);
    }

    #[test]
    fn transparent_over_transparent_stays_transparent() {
        let out = blend_colors(Color32::TRANSPARENT, Color32::TRANSPARENT);
        assert_eq!(out, Color32::TRANSPARENT);
    }
}
