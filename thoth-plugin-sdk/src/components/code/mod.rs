use bon::Builder;
use serde::{Deserialize, Serialize};

/// A read-only code block rendered in a monospace surface.
///
/// ```
/// use thoth_plugin_sdk::components::Code;
///
/// let code = Code::builder().value("let x = 1;").language("rust").build();
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct Code {
    /// The code text.
    pub value: String,
    /// Optional language hint (currently informational).
    #[serde(default)]
    pub language: Option<String>,
}

#[cfg(feature = "egui")]
impl egui::Widget for Code {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        egui::ScrollArea::horizontal()
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(egui::RichText::new(&self.value).monospace())
                        .wrap_mode(egui::TextWrapMode::Extend),
                )
            })
            .inner
    }
}
