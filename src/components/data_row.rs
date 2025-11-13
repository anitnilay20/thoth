use crate::components::traits::StatelessComponent;
use crate::theme::{TextPalette, TextToken};
use eframe::egui::{self, RichText, Ui};

/// Props for DataRow component (immutable, data flows down)
pub struct DataRowProps<'a> {
    /// Display text for the row (already formatted)
    pub display_text: &'a str,

    /// Indentation level
    pub indent: usize,

    /// Whether this row is expandable (shows +/- icon)
    pub is_expandable: bool,

    /// Whether this row is currently expanded
    pub is_expanded: bool,

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
    pub toggle_clicked: bool,
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
        let mut clicked = false;
        let mut right_clicked = false;
        let mut toggle_clicked = false;

        let visuals = ui.visuals();
        let palette = TextPalette::for_visuals(visuals);

        // Parse display text into key and value parts
        let mut parts = props.display_text.splitn(2, ':');
        let key_part = parts.next().unwrap_or("");
        let value_part = parts.next().unwrap_or("");
        let has_colon = !value_part.is_empty() && props.text_tokens.1.is_some();

        egui::Frame::new().fill(props.background).show(ui, |ui| {
            let rect = ui.max_rect();
            let id = ui.id().with(props.row_id);
            let resp = ui.interact(rect, id, egui::Sense::click());

            clicked = resp.clicked();
            right_clicked = resp.clicked_by(egui::PointerButton::Secondary);

            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                // Indentation
                ui.add_space(props.indent as f32 * 12.0);

                // Expand/collapse icon
                if props.is_expandable {
                    let toggle_icon = if props.is_expanded { "-" } else { "+" };
                    if ui
                        .selectable_label(false, RichText::new(toggle_icon).monospace())
                        .clicked()
                    {
                        toggle_clicked = true;
                    }
                } else {
                    ui.add_space(23.0);
                }

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
            clicked,
            right_clicked,
            toggle_clicked,
        }
    }
}
