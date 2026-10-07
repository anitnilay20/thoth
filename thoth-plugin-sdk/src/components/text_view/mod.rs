//! A document shown as the text it is.
//!
//! [`DataView`](crate::components::DataView) is for data: it leads with a
//! table picker and a format switcher and carries Copy and Export, because
//! rows can be drawn several ways and taken elsewhere. A log file has one
//! sensible rendering and no columns to export, so all of that chrome is
//! answering questions nobody asked. This draws the file and nothing else.

use bon::Builder;
use serde::{Deserialize, Serialize};

/// A read-only view of a plain-text document.
///
/// ```
/// use thoth_plugin_sdk::components::TextView;
///
/// let view = TextView::builder()
///     .value("starting worker\nshard-02 ready\n")
///     .build();
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct TextView {
    /// Stable id for this view (keeps scroll position across frames).
    #[builder(default)]
    #[serde(default)]
    pub id: String,
    /// The document's text.
    #[builder(default)]
    #[serde(default)]
    pub value: String,
    /// Optional syntax language (e.g. `"sql"`, `"json"`). `None` renders the
    /// text plain, which is the honest default for a file whose language
    /// nothing has established.
    #[serde(default)]
    pub syntax: Option<String>,
    /// A line stating what the view is *not* showing — a document read in
    /// part, most often.
    ///
    /// Drawn above the text, following the handoff's `.tnote`: a view that has
    /// to compromise says so once, at the top, before the content it is
    /// compromising on. A truncated document that says nothing looks like a
    /// complete one.
    #[serde(default)]
    pub caption: Option<String>,
    /// Font size in points; defaults to the editor's own.
    #[serde(default, rename = "font-size")]
    pub font_size: Option<f32>,
}

#[cfg(feature = "egui")]
impl TextView {
    /// Caption padding — design `.tnote{padding:8px 12px}`.
    const NOTE_PAD_X: i8 = 12;
    const NOTE_PAD_Y: i8 = 8;
    /// Caption size — design `.tnote{font-size:11px}`.
    const NOTE_FONT: f32 = 11.0;

    /// Draw the document, filling the space it is given.
    pub fn show(&self, ui: &mut egui::Ui) {
        use crate::components::{CodeEditor, Typography, TypographyVariant};
        use crate::theme::ThemeColors;

        let colors = ThemeColors::from_ctx(ui.ctx());
        ui.spacing_mut().item_spacing.y = 0.0;

        if let Some(caption) = self.caption.as_deref().filter(|c| !c.is_empty()) {
            let note = egui::Frame::NONE
                .inner_margin(egui::Margin::symmetric(Self::NOTE_PAD_X, Self::NOTE_PAD_Y))
                .show(ui, |ui| {
                    ui.set_width(ui.available_width());
                    ui.add(
                        Typography::builder()
                            .text(caption)
                            .variant(TypographyVariant::Mono)
                            .color("fg_muted")
                            .size(Self::NOTE_FONT)
                            .build(),
                    );
                });
            // Design `.tnote{box-shadow:inset 0 -1px 0 var(--hairline)}`.
            let rect = note.response.rect;
            ui.painter().hline(
                rect.x_range(),
                rect.bottom() - 0.5,
                crate::theme::edge_stroke(&colors),
            );
        }

        // The editor lays out every line it is given, so the scrolling is
        // here rather than inside it.
        let node_id = if self.id.is_empty() {
            "sdk_text_view".to_string()
        } else {
            self.id.clone()
        };
        egui::ScrollArea::both()
            .id_salt((node_id.as_str(), "text_view_scroll"))
            .show(ui, |ui| {
                CodeEditor::builder()
                    .id(format!("{node_id}_text"))
                    .value(self.value.clone())
                    .maybe_syntax(self.syntax.clone())
                    .maybe_font_size(self.font_size)
                    // As tall as the document: the scroll area above provides
                    // the window, and an editor shorter than its text would
                    // scroll inside a scroll.
                    .rows(self.value.lines().count().max(1))
                    // Read-only, not disabled: the text is there to be read,
                    // and dimming it would say the opposite.
                    .read_only(true)
                    // The pane it sits in already owns the fill and the edge.
                    .bordered(false)
                    .build()
                    .show(ui);
            });
    }
}

#[cfg(feature = "egui")]
impl egui::Widget for TextView {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let before = ui.cursor().min;
        self.show(ui);
        let rect = egui::Rect::from_min_max(before, ui.cursor().min);
        ui.allocate_rect(rect, egui::Sense::hover())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_document_survives_serialization() {
        // The node crosses the plugin boundary as JSON.
        let view = TextView::builder()
            .id("log")
            .value("one\ntwo")
            .caption("first 2 of 900 lines")
            .build();
        let back: TextView = serde_json::from_str(&serde_json::to_string(&view).unwrap()).unwrap();
        assert_eq!(back.value, "one\ntwo");
        assert_eq!(back.caption.as_deref(), Some("first 2 of 900 lines"));
        assert!(back.syntax.is_none());
    }

    #[test]
    fn a_plain_document_needs_nothing_but_its_text() {
        let view = TextView::builder().value("hello").build();
        assert!(view.caption.is_none());
        assert!(view.syntax.is_none());
        assert!(view.font_size.is_none());

        // An older node without the optional fields still deserializes.
        let legacy: TextView = serde_json::from_str(r#"{"value":"hi"}"#).unwrap();
        assert_eq!(legacy.value, "hi");
    }
}
