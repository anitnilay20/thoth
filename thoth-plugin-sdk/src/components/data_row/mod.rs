#[cfg(feature = "egui")]
mod ui;

#[cfg(feature = "egui")]
pub use ui::DataRowOutput;

use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::tokens::TextToken;

/// Search-highlight ranges (byte offsets) within a row's key and value text.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct RowHighlights {
    /// Byte ranges to emphasise within the key text.
    #[serde(default)]
    pub key_ranges: Vec<std::ops::Range<usize>>,
    /// Byte ranges to emphasise within the value text.
    #[serde(default)]
    pub value_ranges: Vec<std::ops::Range<usize>>,
}

/// An optional leading icon for a [`DataRow`].
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct DataRowIcon {
    /// The Phosphor glyph to render.
    pub glyph: String,
    /// Colour as a `#rrggbb` hex string; defaults to muted when unset.
    #[serde(default)]
    pub color: Option<String>,
}

/// A single tree/data row: indentation, an optional expand caret or leaf
/// spacer, an optional leading icon, syntax-highlighted `key: value` content,
/// optional trailing text, and hover/selection chrome.
///
/// Render with [`show`](DataRow::show), which returns click / right-click /
/// caret-click flags.
///
/// ```
/// use thoth_plugin_sdk::components::DataRow;
/// use thoth_plugin_sdk::TextToken;
///
/// let row = DataRow::builder()
///     .display_text("name: \"thoth\"")
///     .row_id("row-0")
///     .key_token(TextToken::Key)
///     .value_token(TextToken::Str)
///     .build();
/// ```
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct DataRow {
    /// Display text for the row, formatted as `key` or `key: value`.
    pub display_text: String,
    /// Stable id for interaction (hover/click), unique within the parent.
    pub row_id: String,
    /// Token class used to colour the key part.
    #[builder(default = TextToken::Key)]
    #[serde(default = "default_key_token")]
    pub key_token: TextToken,
    /// Token class for the value part; `None` for a key-only row.
    #[serde(default)]
    pub value_token: Option<TextToken>,
    /// Background fill as a `#rrggbb`/`#rrggbbaa` hex string; transparent when
    /// unset.
    #[serde(default)]
    pub background: Option<String>,
    /// Search-highlight ranges within the key/value text.
    #[builder(default)]
    #[serde(default)]
    pub highlights: RowHighlights,
    /// Apply syntax colouring to the key/value tokens.
    #[builder(default)]
    #[serde(default)]
    pub syntax_highlighting: bool,
    /// Indentation depth (multiplied by a fixed step).
    #[builder(default)]
    #[serde(default)]
    pub indent: usize,
    /// `Some(expanded)` renders an expand/collapse caret; `None` renders an
    /// aligned spacer (leaf row).
    #[serde(default)]
    pub caret: Option<bool>,
    /// Optional leading icon rendered before the content.
    #[serde(default)]
    pub leading_icon: Option<DataRowIcon>,
    /// Optional right-aligned muted text (e.g. a count or type).
    #[serde(default)]
    pub trailing: Option<String>,
    /// Persistent selection highlight.
    #[builder(default)]
    #[serde(default)]
    pub selected: bool,
}

fn default_key_token() -> TextToken {
    TextToken::Key
}
