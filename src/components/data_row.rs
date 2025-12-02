use std::sync::Arc;

use crate::components::traits::StatelessComponent;
use crate::theme::{ROW_HEIGHT, TextPalette, TextToken, hover_row_bg};
use eframe::egui::{self, Color32, RichText, Ui, WidgetText, text::LayoutJob};

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
}

/// Output from DataRow component (events flow up)
pub struct DataRowOutput {
    pub clicked: bool,
    pub right_clicked: bool,
    pub response: egui::Response,
}

/// DataRow is a stateless component that renders a single tree row for any file format
///
/// It handles:
/// - Syntax-highlighted text
/// - Selection highlighting
/// - Click interactions
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

        // Calculate background with hover overlay
        let background = if resp.hovered() {
            // Blend hover overlay on top of background
            blend_colors(props.background, hover_row_bg(ui))
        } else {
            props.background
        };

        let highlight_bg = ui.visuals().selection.bg_fill;
        let highlight_fg = ui.visuals().strong_text_color();

        let _frame_response = egui::Frame::new().fill(background).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                // Key part (with syntax highlighting)
                let key_label_text = format!("{}{}", key_part, if has_colon { ":" } else { "" });
                let key_label = highlighted_text(
                    ui,
                    &key_label_text,
                    palette.color(props.text_tokens.0),
                    &props.highlights.key_ranges,
                    highlight_bg,
                    highlight_fg,
                );
                ui.add(egui::Label::new(key_label).sense(egui::Sense::hover()));

                // Value part (if exists, with different token)
                if let Some(value_token) = props.text_tokens.1 {
                    let value_label = highlighted_text(
                        ui,
                        value_part,
                        palette.color(value_token),
                        &props.highlights.value_ranges,
                        highlight_bg,
                        highlight_fg,
                    );
                    ui.add(egui::Label::new(value_label).sense(egui::Sense::hover()));
                }
            });
        });

        DataRowOutput {
            clicked: resp.clicked(),
            right_clicked: resp.secondary_clicked(),
            response: resp,
        }
    }
}

/// Blend two colors (overlay on top of background) using alpha compositing
fn blend_colors(background: Color32, overlay: Color32) -> Color32 {
    let bg = background.to_array();
    let ov = overlay.to_array();

    // Simple alpha blending
    let alpha = ov[3] as f32 / 255.0;
    let inv_alpha = 1.0 - alpha;

    Color32::from_rgba_unmultiplied(
        ((bg[0] as f32 * inv_alpha) + (ov[0] as f32 * alpha)) as u8,
        ((bg[1] as f32 * inv_alpha) + (ov[1] as f32 * alpha)) as u8,
        ((bg[2] as f32 * inv_alpha) + (ov[2] as f32 * alpha)) as u8,
        bg[3],
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
