use bon::Builder;
use serde::{Deserialize, Serialize};

/// An editable, syntax-highlighted code editor. Owns its `value`;
/// [`CodeEditor::show`] edits it in place.
///
/// ```
/// use thoth_plugin_sdk::components::CodeEditor;
///
/// let mut ed = CodeEditor::builder().value("{}").syntax("json").build();
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct CodeEditor {
    /// Widget id used for event routing.
    #[builder(default)]
    #[serde(default)]
    pub id: String,
    /// The editor's text content.
    #[builder(default)]
    #[serde(default)]
    pub value: String,
    /// Font size in points; defaults to 13.
    #[serde(default)]
    pub font_size: Option<f32>,
    /// Optional syntax language (e.g. `"rust"`, `"sql"`); defaults to plain.
    #[serde(default)]
    pub syntax: Option<String>,
    /// Minimum number of visible text rows. Defaults to the editor's own
    /// default when unset.
    #[serde(default)]
    pub rows: Option<usize>,
    /// Disable editing (renders read-only / dimmed).
    #[builder(default)]
    #[serde(default)]
    pub disabled: bool,
    /// Draw a themed border around the whole editor. Defaults to `true`.
    #[builder(default = true)]
    #[serde(default = "default_true")]
    pub bordered: bool,
}

fn default_true() -> bool {
    true
}

#[cfg(feature = "egui")]
impl CodeEditor {
    /// Render the editor, editing [`value`](CodeEditor::value) in place.
    /// Returns `true` when the text changed this frame.
    pub fn show(&mut self, ui: &mut egui::Ui) -> bool {
        use crate::theme::ThemeColors;
        use egui_code_editor::{CodeEditor as Editor, Syntax};
        let colors = ThemeColors::from_ctx(ui.ctx());
        let syntax = match self.syntax.as_deref() {
            Some("rust") => Syntax::rust(),
            Some("sql") => Syntax::sql(),
            Some("shell") | Some("sh") | Some("bash") => Syntax::shell(),
            _ => Syntax::default(),
        };
        let id_source = if self.id.is_empty() {
            "sdk_code_editor"
        } else {
            self.id.as_str()
        };
        let theme = colors.code_editor_theme();

        // A single themed border around the whole editor; the inner `TextEdit`'s
        // own frame/focus stroke is suppressed so it doesn't draw a second border
        // around the code area on hover/selection.
        let stroke = if self.bordered {
            egui::Stroke::new(1.0, colors.surface_raised)
        } else {
            egui::Stroke::NONE
        };
        egui::Frame::new()
            .fill(colors.bg)
            .stroke(stroke)
            .corner_radius(4)
            // Top padding so the first line sits a little below the border.
            .inner_margin(egui::Margin {
                left: 0,
                right: 0,
                top: 6,
                bottom: 0,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.visuals_mut().widgets.inactive.bg_stroke = egui::Stroke::NONE;
                ui.visuals_mut().widgets.hovered.bg_stroke = egui::Stroke::NONE;
                ui.visuals_mut().widgets.active.bg_stroke = egui::Stroke::NONE;
                ui.visuals_mut().selection.stroke = egui::Stroke::NONE;
                ui.add_enabled_ui(!self.disabled, |ui| {
                    let mut editor = Editor::default()
                        .id_source(id_source)
                        .with_fontsize(self.font_size.unwrap_or(13.0))
                        .with_theme(theme)
                        .with_syntax(syntax);
                    if let Some(rows) = self.rows {
                        editor = editor.with_rows(rows);
                    }
                    editor.show(ui, &mut self.value).response.changed()
                })
                .inner
            })
            .inner
    }
}
