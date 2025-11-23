use crate::components::traits::StatelessComponent;
use crate::theme::{ROW_HEIGHT, TextPalette, TextToken, hover_row_bg};
use eframe::egui::{self, Color32, RichText, Ui};

/// Props for DataRow component (immutable, data flows down)
pub struct DataRowProps<'a> {
    /// Display text for the row (already formatted)
    pub display_text: &'a str,

    /// Indentation level (in pixels or units)
    pub indent: usize,

    /// Text tokens for syntax coloring (key_token, value_token)
    pub text_tokens: (TextToken, Option<TextToken>),

    /// Background color
    pub background: egui::Color32,

    /// Unique ID for interaction
    pub row_id: &'a str,
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
/// - Indentation
/// - Expand/collapse icon
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

        let _frame_response = egui::Frame::new().fill(background).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                // Indentation (VS Code design system: 16px per level)
                ui.add_space(props.indent as f32 * 16.0);

                // Key part (with syntax highlighting)
                ui.add(egui::Label::new(
                    RichText::new(format!("{}{}", key_part, if has_colon { ":" } else { "" }))
                        .monospace()
                        .color(palette.color(props.text_tokens.0)),
                ));

                // Value part (if exists, with different token)
                if let Some(value_token) = props.text_tokens.1 {
                    ui.add(egui::Label::new(
                        RichText::new(value_part)
                            .monospace()
                            .color(palette.color(value_token)),
                    ));
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
