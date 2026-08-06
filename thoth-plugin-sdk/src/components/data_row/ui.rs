use std::sync::Arc;

use egui::{Color32, RichText, Ui, WidgetText, text::LayoutJob};

use crate::components::IconButton;
use crate::theme::{
    FONT_BODY, FONT_CAPTION, RADIUS_CHECK, RADIUS_CHIP, ROW_HEIGHT, TextPalette, TextToken,
    ThemeColors, phosphor_font_id, resolve_color, with_alpha,
};

use super::DataRow;

/// Indentation step per tree depth level — design nests a depth-1 row to
/// `padding-left:24px`, i.e. 16px past the row's own 8px padding.
const INDENT_STEP: f32 = 16.0;
/// Horizontal row padding — design `.dr{padding:0 8px}`.
const ROW_PAD_X: i8 = 8;
/// Gap between the row's parts (caret, icon, key, `:`, value) — design
/// `.dr{gap:5px}`.
const PART_GAP: f32 = 5.0;
/// Caret slot width; the glyph is centred in it — design `.dr .car{width:13px}`.
const CARET_SLOT: f32 = 13.0;
/// Row text size — design `.tree{font-size:12px}`, inherited by `.dr`.
const ROW_FONT: f32 = 12.0;
/// Gap before the trailing metadata — design `.dr .trail{padding-left:14px}`.
const TRAIL_PAD: f32 = 14.0;
/// Hover wash — design `.dr:hover{background:text 7%}`.
const HOVER_ALPHA: u8 = 18;
/// Selection wash — design `.dr.sel{background:accent 16%}`.
const SELECT_ALPHA: u8 = 41;
/// Search-highlight wash — design `.dr .hl{background:accent2 34%}`.
const HIGHLIGHT_ALPHA: u8 = 87;
/// Inline type chip — design `.dr .tybadge{font-size:9.5px}`…
const CHIP_FONT: f32 = 9.5;
/// …`padding:1px 5px`…
const CHIP_PAD_X: f32 = 5.0;
const CHIP_PAD_Y: f32 = 1.0;
/// …and `margin-left:6px`, i.e. one point past the row's own 5px part gap.
const CHIP_MARGIN: f32 = 6.0;

/// Outcome of rendering a [`DataRow`].
pub struct DataRowOutput {
    /// The row body or its content was clicked.
    pub clicked: bool,
    /// The row was right-clicked (context menu).
    pub right_clicked: bool,
    /// The expand/collapse caret was clicked (takes precedence over `clicked`).
    pub caret_clicked: bool,
    /// The trailing action icon was clicked (takes precedence over `clicked`).
    pub action_clicked: bool,
    /// The row's interaction response.
    pub response: egui::Response,
}

impl DataRow {
    /// Render the row and report interaction. Design `.dr`.
    pub fn show(&self, ui: &mut Ui) -> DataRowOutput {
        let colors = ThemeColors::from_ctx(ui.ctx());
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

        // The washes composite bottom-up: the caller's fill (a JsonTree zebra
        // stripe, say) sits under the selection wash, which sits under hover —
        // so a striped row still reads as selected and as hovered.
        let mut background = self
            .background
            .as_deref()
            .and_then(|c| resolve_color(c, &colors))
            .unwrap_or(Color32::TRANSPARENT);
        if self.selected {
            background = blend_colors(background, with_alpha(colors.accent, SELECT_ALPHA));
        }
        let hovered = resp.hovered() || ui.rect_contains_pointer(interact_rect);
        if hovered {
            background = blend_colors(background, with_alpha(colors.fg, HOVER_ALPHA));
        }

        let highlight_bg = with_alpha(colors.accent_secondary, HIGHLIGHT_ALPHA);
        let highlight_fg = colors.fg;
        let base_text_color = colors.fg;
        let muted = colors.fg_muted;

        let mut caret_clicked = false;
        let mut action_clicked = false;
        let mut body_clicked = false;
        let mut body_secondary = false;

        egui::Frame::NONE
            .fill(background)
            .corner_radius(RADIUS_CHIP)
            .inner_margin(egui::Margin::symmetric(ROW_PAD_X, 0))
            .show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = PART_GAP;
                    if self.indent > 0 {
                        // The layout gap that follows completes the indent step.
                        ui.add_space(self.indent as f32 * INDENT_STEP - PART_GAP);
                    }

                    // Design `.dr .car`: a fixed 13px slot with the glyph centred in
                    // it, so leaf rows line up under their parent's caret.
                    let caret_slot = egui::vec2(CARET_SLOT, ROW_HEIGHT);
                    match self.caret {
                        Some(expanded) => {
                            let glyph = if expanded {
                                egui_phosphor::regular::CARET_DOWN
                            } else {
                                egui_phosphor::regular::CARET_RIGHT
                            };
                            let (rect, caret_resp) =
                                ui.allocate_exact_size(caret_slot, egui::Sense::click());
                            ui.painter().text(
                                rect.center(),
                                egui::Align2::CENTER_CENTER,
                                glyph,
                                phosphor_font_id(FONT_CAPTION),
                                muted,
                            );
                            if caret_resp.clicked() {
                                caret_clicked = true;
                            }
                        }
                        // Leaf row: the slot is reserved but left empty.
                        None => {
                            ui.allocate_exact_size(caret_slot, egui::Sense::hover());
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
                                .font(phosphor_font_id(icon.size.unwrap_or(FONT_BODY)))
                                .color(color)
                                .into(),
                            false,
                            &mut body_clicked,
                            &mut body_secondary,
                        );
                    }

                    let key_color = if self.summary_value && !has_colon {
                        // A key-less collapsed container — the whole row is summary.
                        muted
                    } else {
                        palette.color_with_highlighting(
                            self.key_token,
                            self.syntax_highlighting,
                            base_text_color,
                        )
                    };
                    let key_label = highlighted_text(
                        key_part,
                        key_color,
                        &self.highlights.key_ranges,
                        highlight_bg,
                        highlight_fg,
                    );

                    // The separator is its own punctuation-coloured part — design
                    // `<span class="k">key</span><span class="p">:</span>`.
                    let colon_label = has_colon.then(|| {
                        let color = palette.color_with_highlighting(
                            TextToken::Bracket,
                            self.syntax_highlighting,
                            base_text_color,
                        );
                        mono_text(":", color)
                    });

                    // `display_text` separates key and value with `": "`; the 5px
                    // part gap replaces that space, so drop it and shift the
                    // caller's highlight ranges (byte offsets into the untrimmed
                    // value) by as much.
                    let trimmed = value_part.len() - value_part.trim_start().len();
                    let value_text = &value_part[trimmed..];
                    let value_ranges: Vec<std::ops::Range<usize>> = self
                        .highlights
                        .value_ranges
                        .iter()
                        .map(|r| r.start.saturating_sub(trimmed)..r.end.saturating_sub(trimmed))
                        .collect();
                    let value_label =
                        self.value_token
                            .filter(|_| !value_text.is_empty())
                            .map(|value_token| {
                                let value_color = if self.summary_value {
                                    // Design `.dr .sum` — a collapsed-container summary
                                    // reads as metadata, not as a value token.
                                    muted
                                } else {
                                    palette.color_with_highlighting(
                                        value_token,
                                        self.syntax_highlighting,
                                        base_text_color,
                                    )
                                };
                                highlighted_text(
                                    value_text,
                                    value_color,
                                    &value_ranges,
                                    highlight_bg,
                                    highlight_fg,
                                )
                            });

                    // Inline type chip — design `.dr .tybadge`. Laid out up front
                    // so its width can be reserved *before* the row's text
                    // truncates, otherwise a long name would push it off the row.
                    let chip = self
                        .chip
                        .as_deref()
                        .filter(|t| !t.is_empty())
                        .map(|t| chip_galley(ui, t, muted));

                    // Trailing count + action, added right-to-left so they pin to the
                    // right edge. Flags are passed in (not captured) so the label/
                    // truncate code below can still borrow them.
                    let action_icon = self.action_icon.clone();
                    let action_tooltip = self.action_tooltip.clone();
                    let trailing_text = self.trailing.clone();
                    let render_trailing = |ui: &mut Ui, bc: &mut bool, bs: &mut bool| -> bool {
                        let mut clicked = false;
                        if let Some(glyph) = &action_icon {
                            clicked = ui
                                .add(
                                    IconButton::builder()
                                        .icon(glyph.as_str())
                                        .maybe_tooltip(action_tooltip.clone())
                                        .build(),
                                )
                                .clicked();
                        }
                        if let Some(t) = &trailing_text {
                            body_label(
                                ui,
                                RichText::new(t)
                                    .font(egui::FontId::monospace(FONT_CAPTION))
                                    .color(muted)
                                    .into(),
                                false,
                                bc,
                                bs,
                            );
                            // Design `.dr .trail{padding-left:14px}` — laid out
                            // right-to-left, so this space lands to its left.
                            ui.add_space(TRAIL_PAD - PART_GAP);
                        }
                        clicked
                    };

                    if self.truncate {
                        // Full-width row: pin trailing/action right, and truncate the
                        // key/value in the middle with an ellipsis so nothing bleeds.
                        let remaining = egui::vec2(ui.available_width(), ROW_HEIGHT);
                        ui.allocate_ui_with_layout(
                            remaining,
                            egui::Layout::right_to_left(egui::Align::Center),
                            |ui| {
                                if render_trailing(ui, &mut body_clicked, &mut body_secondary) {
                                    action_clicked = true;
                                }
                                ui.with_layout(
                                    egui::Layout::left_to_right(egui::Align::Center),
                                    |ui| {
                                        ui.style_mut().wrap_mode =
                                            Some(egui::TextWrapMode::Truncate);
                                        let text = |ui: &mut Ui| {
                                            body_label(
                                                ui,
                                                key_label,
                                                true,
                                                &mut body_clicked,
                                                &mut body_secondary,
                                            );
                                            if let Some(colon_label) = colon_label {
                                                body_label(
                                                    ui,
                                                    colon_label,
                                                    true,
                                                    &mut body_clicked,
                                                    &mut body_secondary,
                                                );
                                            }
                                            if let Some(value_label) = value_label {
                                                body_label(
                                                    ui,
                                                    value_label,
                                                    true,
                                                    &mut body_clicked,
                                                    &mut body_secondary,
                                                );
                                            }
                                        };
                                        match chip {
                                            Some(chip) => {
                                                // Cap the text at what's left once
                                                // the chip has its room, so the
                                                // ellipsis lands before it rather
                                                // than shoving it off the row.
                                                let room = (ui.available_width()
                                                    - chip_width(&chip))
                                                .max(0.0);
                                                ui.horizontal(|ui| {
                                                    ui.set_max_width(room);
                                                    text(ui);
                                                });
                                                chip_label(ui, chip, muted, &colors);
                                            }
                                            None => text(ui),
                                        }
                                    },
                                );
                            },
                        );
                    } else {
                        // Extend: key/value keep their full width (so a horizontally
                        // scrolling container can reveal long JSON), trailing after.
                        body_label(ui, key_label, true, &mut body_clicked, &mut body_secondary);
                        if let Some(colon_label) = colon_label {
                            body_label(
                                ui,
                                colon_label,
                                true,
                                &mut body_clicked,
                                &mut body_secondary,
                            );
                        }
                        if let Some(value_label) = value_label {
                            body_label(
                                ui,
                                value_label,
                                true,
                                &mut body_clicked,
                                &mut body_secondary,
                            );
                        }
                        if let Some(chip) = chip {
                            chip_label(ui, chip, muted, &colors);
                        }
                        if action_icon.is_some() || trailing_text.is_some() {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    if render_trailing(ui, &mut body_clicked, &mut body_secondary) {
                                        action_clicked = true;
                                    }
                                },
                            );
                        }
                    }
                });
            });

        if hovered {
            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
        }

        DataRowOutput {
            // A caret or action click takes precedence and must not surface as a
            // row click.
            clicked: !caret_clicked && !action_clicked && (resp.clicked() || body_clicked),
            right_clicked: resp.secondary_clicked() || body_secondary,
            caret_clicked,
            action_clicked,
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

/// Lay out an inline type chip's label — design `.dr .tybadge`, a proportional
/// 9.5pt run (it carries a type name, not a code token).
fn chip_galley(ui: &Ui, text: &str, color: Color32) -> Arc<egui::Galley> {
    ui.painter().layout_no_wrap(
        text.to_owned(),
        egui::FontId::proportional(CHIP_FONT),
        color,
    )
}

/// Total width an inline chip occupies, including the extra point of left margin
/// it carries over the row's part gap.
fn chip_width(galley: &egui::Galley) -> f32 {
    galley.size().x + CHIP_PAD_X * 2.0 + (CHIP_MARGIN - PART_GAP)
}

/// Draw the chip: a surface-filled tag riding just after the row's text. The
/// design's radius (4px) is below the ladder's smallest rung, so the nearest one
/// ([`RADIUS_CHECK`]) stands in.
fn chip_label(ui: &mut Ui, galley: Arc<egui::Galley>, color: Color32, colors: &ThemeColors) {
    // `.tybadge{margin-left:6px}` is one point more than the row's 5px part gap,
    // so the chip carries the extra point inside its own allocation.
    let lead = CHIP_MARGIN - PART_GAP;
    let pad = egui::vec2(CHIP_PAD_X, CHIP_PAD_Y);
    let size = galley.size() + pad * 2.0 + egui::vec2(lead, 0.0);
    let (rect, _) = ui.allocate_exact_size(size, egui::Sense::hover());
    if ui.is_rect_visible(rect) {
        let chip = egui::Rect::from_min_size(
            rect.min + egui::vec2(lead, 0.0),
            rect.size() - egui::vec2(lead, 0.0),
        );
        ui.painter().rect_filled(chip, RADIUS_CHECK, colors.surface);
        ui.painter().galley(chip.min + pad, galley, color);
    }
}

/// One row part in the design's monospace row font.
fn mono_text(text: &str, color: Color32) -> WidgetText {
    RichText::new(text)
        .font(egui::FontId::monospace(ROW_FONT))
        .color(color)
        .into()
}

/// A row part with the caller's search matches washed in `highlight_bg` —
/// design `.dr .hl`. `TextFormat::expand_bg` defaults to the design's 1px
/// horizontal padding.
fn highlighted_text(
    text: &str,
    base_color: Color32,
    ranges: &[std::ops::Range<usize>],
    highlight_bg: Color32,
    highlight_fg: Color32,
) -> WidgetText {
    if text.is_empty() || ranges.is_empty() {
        return mono_text(text, base_color);
    }

    let mut job = LayoutJob::default();
    let base_format = egui::TextFormat {
        font_id: egui::FontId::monospace(ROW_FONT),
        color: base_color,
        ..Default::default()
    };
    let highlight_format = egui::TextFormat {
        font_id: egui::FontId::monospace(ROW_FONT),
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
