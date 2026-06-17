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
        let bg = ui.visuals().code_bg_color;
        egui::Frame::new()
            .fill(bg)
            .corner_radius(4)
            .inner_margin(egui::Margin::symmetric(8, 6))
            .show(ui, |ui| {
                ui.add(
                    egui::Label::new(egui::RichText::new(&self.value).monospace())
                        .selectable(true),
                );
            })
            .response
    }
}
