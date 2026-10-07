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
#[non_exhaustive]
pub struct Markdown {
    /// Stable id, used to key the render cache. Two Markdown blocks that share
    /// one share a cache, which is what you want for the same document drawn
    /// in two places and not what you want for two different documents.
    #[builder(default)]
    #[serde(default)]
    pub id: String,
    /// The Markdown source.
    pub value: String,
}

#[cfg(feature = "egui")]
impl Markdown {
    /// Render the Markdown into `ui`.
    ///
    /// The `CommonMarkCache` is kept in egui memory rather than rebuilt per
    /// call: the SDK reconstructs component structs every frame, and a fresh
    /// cache means re-parsing the whole document on each one — which a long
    /// README makes obvious. The cache also holds decoded images, so throwing
    /// it away re-decodes them too.
    pub fn show(&self, ui: &mut egui::Ui) {
        use egui_commonmark::{CommonMarkCache, CommonMarkViewer};
        use std::sync::{Arc, Mutex};

        let cache_id = ui.make_persistent_id(("sdk_markdown_cache", self.id.as_str()));
        let cache = ui.ctx().data_mut(|d| {
            d.get_temp::<Arc<Mutex<CommonMarkCache>>>(cache_id)
                .unwrap_or_else(|| Arc::new(Mutex::new(CommonMarkCache::default())))
        });
        {
            let mut cache = cache.lock().unwrap_or_else(|e| e.into_inner());
            CommonMarkViewer::new().show(ui, &mut cache, &self.value);
        }
        ui.ctx().data_mut(|d| d.insert_temp(cache_id, cache));
    }
}

#[cfg(all(test, feature = "egui"))]
mod tests {
    use super::*;

    #[test]
    fn a_block_survives_serialization() {
        let md = Markdown::builder().id("readme").value("# Title").build();
        let back: Markdown = serde_json::from_str(&serde_json::to_string(&md).unwrap()).unwrap();
        assert_eq!(back.id, "readme");
        assert_eq!(back.value, "# Title");

        // An older node without the id still deserializes.
        let legacy: Markdown = serde_json::from_str(r#"{"value":"hi"}"#).unwrap();
        assert!(legacy.id.is_empty());
    }
}
