use std::sync::Arc;

use crate::components::icon_button::{IconButton, IconButtonProps};
use crate::components::traits::StatelessComponent;
use crate::theme::{ROW_HEIGHT, TextPalette, TextToken, hover_row_bg, icon_rich_text};
use eframe::egui::{self, Color32, RichText, Ui, WidgetText, text::LayoutJob};

/// Indentation step per tree depth level, in logical pixels.
const INDENT_STEP: f32 = 16.0;

#[derive(Default, Clone)]
pub struct RowHighlights {
    pub key_ranges: Vec<std::ops::Range<usize>>,
    pub value_ranges: Vec<std::ops::Range<usize>>,
}

/// Props for DataRow component (immutable, data flows down)
pub struct DataRowProps<'a> {
    /// Display text for the row (already formatted)
    pub display_text: &'a str,

    /// Text tokens for syntax coloring (key_token, value_token)
    pub text_tokens: (TextToken, Option<TextToken>),

    /// Background color
    pub background: egui::Color32,

    /// Unique ID for interaction
    pub row_id: &'a str,

    /// Highlight terms to emphasize within key/value text
    pub highlights: RowHighlights,

    /// Enable syntax highlighting
    pub syntax_highlighting: bool,

    // ── tree-row chrome (default to a flat, caret-less row) ──────────────────
    /// Indentation depth (multiplied by [`INDENT_STEP`]).
    pub indent: usize,
    /// `Some(expanded)` renders an expand/collapse caret; `None` renders an
    /// aligned spacer (a leaf row). Caret clicks are reported via `caret_clicked`.
    pub caret: Option<bool>,
    /// Optional leading icon `(glyph, color)` rendered before the content.
    pub leading_icon: Option<(&'a str, Color32)>,
    /// Optional right-aligned muted text (e.g. a count or a column type).
    pub trailing: Option<&'a str>,
    /// Persistent selection highlight.
    pub selected: bool,
}

impl<'a> DataRowProps<'a> {
    /// A plain content row (no indent/caret/icon/trailing) — the common case.
    pub fn new(
        display_text: &'a str,
        text_tokens: (TextToken, Option<TextToken>),
        background: egui::Color32,
        row_id: &'a str,
        highlights: RowHighlights,
        syntax_highlighting: bool,
    ) -> Self {
        Self {
            display_text,
            text_tokens,
            background,
            row_id,
            highlights,
            syntax_highlighting,
            indent: 0,
            caret: None,
            leading_icon: None,
            trailing: None,
            selected: false,
        }
    }
}

/// Output from DataRow component (events flow up)
pub struct DataRowOutput {
    pub clicked: bool,
    pub right_clicked: bool,
    /// The expand/collapse caret was clicked (takes precedence over `clicked`).
    pub caret_clicked: bool,
    pub response: egui::Response,
}

/// DataRow is a stateless component that renders a single tree row for any file
/// format or data tree.
///
/// It handles:
/// - Indentation + an optional expand/collapse caret (or an aligned leaf spacer)
/// - An optional leading icon and trailing muted text
/// - Syntax-highlighted key/value content
/// - Hover + selection highlighting and click interactions
///
/// The component is pure: same props = same output
pub struct DataRow;

impl StatelessComponent for DataRow {
    type Props<'a> = DataRowProps<'a>;
    type Output = DataRowOutput;

    fn render(ui: &mut Ui, props: Self::Props<'_>) -> Self::Output {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        let palette = TextPalette::from_context(ui.ctx());

        // Parse display text into key and value parts
        let mut parts = props.display_text.splitn(2, ':');
        let key_part = parts.next().unwrap_or("");
        let value_part = parts.next().unwrap_or("");
        let has_colon = !value_part.is_empty() && props.text_tokens.1.is_some();

        // First, create an interact rect to detect hover
        let id = ui.id().with(props.row_id);
        let available_rect = ui.available_rect_before_wrap();
        let interact_rect = egui::Rect::from_min_size(
            available_rect.min,
            egui::vec2(ui.available_width(), ROW_HEIGHT),
        );
        let resp = ui.interact(interact_rect, id, egui::Sense::click());

        // Base background: selection wins, then the caller's background, then a
        // hover overlay blended on top.
        let base_bg = if props.selected {
            ui.visuals().selection.bg_fill
        } else {
            props.background
        };
        let background = if resp.hovered() {
            blend_colors(base_bg, hover_row_bg(ui))
        } else {
            base_bg
        };

        let highlight_bg = ui.visuals().selection.bg_fill;
        let highlight_fg = ui.visuals().strong_text_color();
        let base_text_color = ui.visuals().text_color();
        let muted = ui.visuals().weak_text_color();

        let mut caret_clicked = false;
        // Clicks that land on the body labels (text/icon). We OR these into the
        // row click so clicking the text acts on the row — while the labels stay
        // selectable (drag-to-select still works) and show a pointer cursor.
        let mut body_clicked = false;
        let mut body_secondary = false;

        let _frame_response = egui::Frame::new().fill(background).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                if props.indent > 0 {
                    ui.add_space(props.indent as f32 * INDENT_STEP);
                }

                // Expand caret, or an aligned (invisible) spacer for leaf rows.
                match props.caret {
                    Some(expanded) => {
                        let glyph = if expanded {
                            egui_phosphor::regular::CARET_DOWN
                        } else {
                            egui_phosphor::regular::CARET_RIGHT
                        };
                        let out = IconButton::render(
                            ui,
                            IconButtonProps {
                                icon: glyph,
                                frame: false,
                                tooltip: Some(if expanded { "Collapse" } else { "Expand" }),
                                badge_color: None,
                                size: None,
                                disabled: false,
                                icon_size: None,
                                selected: false,
                            },
                        );
                        if out.clicked {
                            caret_clicked = true;
                        }
                    }
                    None => {
                        ui.add_enabled_ui(false, |ui| {
                            ui.visuals_mut().widgets.inactive.bg_fill = Color32::TRANSPARENT;
                            ui.visuals_mut().widgets.inactive.weak_bg_fill = Color32::TRANSPARENT;
                            IconButton::render(
                                ui,
                                IconButtonProps {
                                    icon: " ",
                                    frame: false,
                                    tooltip: None,
                                    badge_color: None,
                                    size: None,
                                    disabled: false,
                                    icon_size: None,
                                    selected: false,
                                },
                            );
                        });
                    }
                }

                // Optional leading icon (phosphor glyph).
                if let Some((glyph, color)) = props.leading_icon {
                    body_label(
                        ui,
                        icon_rich_text(glyph, 13.0).color(color).into(),
                        false,
                        &mut body_clicked,
                        &mut body_secondary,
                    );
                    ui.add_space(4.0);
                }

                // Key part (with syntax highlighting if enabled)
                let key_label_text = format!("{}{}", key_part, if has_colon { ":" } else { "" });
                let key_color = palette.color_with_highlighting(
                    props.text_tokens.0,
                    props.syntax_highlighting,
                    base_text_color,
                );
                let key_label = highlighted_text(
                    ui,
                    &key_label_text,
                    key_color,
                    &props.highlights.key_ranges,
                    highlight_bg,
                    highlight_fg,
                );
                body_label(ui, key_label, true, &mut body_clicked, &mut body_secondary);

                // Value part (if exists, with different token)
                if let Some(value_token) = props.text_tokens.1 {
                    let value_color = palette.color_with_highlighting(
                        value_token,
                        props.syntax_highlighting,
                        base_text_color,
                    );
                    let value_label = highlighted_text(
                        ui,
                        value_part,
                        value_color,
                        &props.highlights.value_ranges,
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

                // Optional right-aligned trailing text (count / type).
                if let Some(trailing) = props.trailing {
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

        // The row is clickable, so a pointer cursor over its empty areas; the
        // labels carry their own pointer cursor (see `body_label`).
        if resp.hovered() {
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

/// Add one body label that participates in the row's click. Selectable labels
/// (text) keep drag-to-select; a plain click reports `clicked()`, which we OR
/// into the row. Non-selectable labels (icons) just sense clicks. Either way the
/// cursor is a pointer, signalling the row is clickable.
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

/// Composite `overlay` over `background` (proper source-over alpha blending).
/// Works when the background is translucent/transparent too, so a hover overlay
/// stays visible on a transparent row (e.g. a tree row with no zebra fill).
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
