use bon::Builder;
use serde::{Deserialize, Serialize};

/// A hyperlink that opens `url` when clicked.
///
/// ```
/// use thoth_plugin_sdk::components::Link;
///
/// let link = Link::builder().label("Docs").url("https://example.com").build();
/// ```
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct Link {
    /// Visible link text.
    pub label: String,
    /// Target URL.
    pub url: String,
}

#[cfg(feature = "egui")]
impl egui::Widget for Link {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        ui.hyperlink_to(self.label, self.url)
    }
}
