use crate::components::traits::StatelessComponent;
use crate::theme::{TextPalette, TextToken};
use eframe::egui::{self, RichText, Ui};

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
        let visuals = ui.visuals();
        let palette = TextPalette::for_visuals(visuals);

        // Parse display text into key and value parts
        let mut parts = props.display_text.splitn(2, ':');
        let key_part = parts.next().unwrap_or("");
        let value_part = parts.next().unwrap_or("");
        let has_colon = !value_part.is_empty() && props.text_tokens.1.is_some();

        let frame_response = egui::Frame::new().fill(props.background).show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                // Indentation
                ui.add_space(props.indent as f32 * 12.0);

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

        // Interact with the row for clicks
        let id = ui.id().with(props.row_id);
        let resp = ui.interact(frame_response.response.rect, id, egui::Sense::click());

        DataRowOutput {
            clicked: resp.clicked(),
            right_clicked: resp.secondary_clicked(),
            response: resp,
        }
    }
}
