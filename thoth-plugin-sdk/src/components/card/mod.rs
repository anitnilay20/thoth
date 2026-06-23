#[cfg(feature = "egui")]
mod ui;

#[cfg(feature = "egui")]
pub use ui::CardEvent;

use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::render_node::RenderNode;

/// A card's leading icon — either a Phosphor glyph or an embedded image.
///
/// The image variant carries raw bytes (e.g. a PNG) rather than a filesystem
/// path, so the SDK never touches host disk; it renders via egui's image
/// loaders (the host installs them).
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CardIcon {
    /// A Phosphor glyph.
    Glyph(String),
    /// An embedded image: a stable `uri` key and its raw bytes.
    Image {
        /// Stable cache key for the image loader (e.g. `"bytes://icon-1"`).
        uri: String,
        /// Encoded image bytes (PNG/JPEG/…).
        bytes: Vec<u8>,
    },
    /// A PNG/ICO loaded from a filesystem path. Host-only — plugins cannot
    /// supply host paths, so this renders nothing under wasm.
    IconFile {
        /// Filesystem path to the icon.
        path: String,
    },
}

/// A card action button.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct CardAction {
    /// Button label.
    pub label: String,
    /// Render as a destructive (danger) action.
    #[builder(default)]
    #[serde(default)]
    pub danger: bool,
}

/// A content card: a leading icon, title/subtitle/meta, optional tags, an
/// optional enable toggle, an optional [`RenderNode`] body, and action buttons.
///
/// Render with [`Card::show`], which reports toggle / action events.
///
/// ```
/// use thoth_plugin_sdk::components::Card;
///
/// let card = Card::builder().title("My Plugin").subtitle("Does things").build();
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct Card {
    /// Title text.
    pub title: String,
    /// Optional subtitle line.
    #[serde(default)]
    pub subtitle: Option<String>,
    /// Optional single-line metadata (e.g. "v1.0 · by Author").
    #[serde(default)]
    pub meta: Option<String>,
    /// Optional small pill tags.
    #[builder(default)]
    #[serde(default)]
    pub tags: Vec<String>,
    /// `Some(on)` shows an enable toggle in the header; `None` hides it.
    #[serde(default)]
    pub enabled: Option<bool>,
    /// Optional leading icon.
    #[serde(default)]
    pub icon: Option<CardIcon>,
    /// Optional rich body rendered below the header.
    #[serde(default)]
    pub body: Option<RenderNode>,
    /// Bottom-right action buttons.
    #[builder(default)]
    #[serde(default)]
    pub actions: Vec<CardAction>,
}
