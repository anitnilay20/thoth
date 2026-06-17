use bon::Builder;
use serde::{Deserialize, Serialize};

/// A rendered Markdown block.
///
/// ```
/// use thoth_plugin_sdk::components::Markdown;
///
/// let md = Markdown::builder().value("# Title\n\nSome **bold** text.").build();
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct Markdown {
    /// The Markdown source.
    pub value: String,
}

#[cfg(feature = "egui")]
impl Markdown {
    /// Render the Markdown into `ui`.
    pub fn show(&self, ui: &mut egui::Ui) {
        // A per-call cache is fine for text-only Markdown; image-heavy content
        // would want a cache persisted in egui memory.
        let mut cache = egui_commonmark::CommonMarkCache::default();
        egui_commonmark::CommonMarkViewer::new().show(ui, &mut cache, &self.value);
    }
}
