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
pub struct CodeEditor {
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
}

#[cfg(feature = "egui")]
impl CodeEditor {
    /// Render the editor, editing [`value`](CodeEditor::value) in place.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        use egui_code_editor::{CodeEditor as Editor, ColorTheme, Syntax};
        let syntax = match self.syntax.as_deref() {
            Some("rust") => Syntax::rust(),
            Some("sql") => Syntax::sql(),
            Some("shell") | Some("sh") | Some("bash") => Syntax::shell(),
            _ => Syntax::default(),
        };
        Editor::default()
            .id_source("sdk_code_editor")
            .with_fontsize(self.font_size.unwrap_or(13.0))
            .with_theme(ColorTheme::GRUVBOX)
            .with_syntax(syntax)
            .show(ui, &mut self.value);
    }
}
