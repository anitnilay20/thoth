#[cfg(feature = "egui")]
mod ui;

#[cfg(feature = "egui")]
pub use ui::ListEvent;

use bon::Builder;
use serde::{Deserialize, Serialize};

/// A right-aligned icon action on a [`ListItem`].
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct ListItemAction {
    /// The action's Phosphor glyph.
    pub icon: String,
    /// Optional tooltip shown on hover.
    #[serde(default)]
    pub tooltip: Option<String>,
}

/// A colored badge shown before a [`ListItem`]'s title (e.g. an HTTP method).
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct ListItemBadge {
    /// Badge text.
    pub text: String,
    /// Fill colour as `#rrggbb` hex; defaults to the secondary accent.
    #[serde(default)]
    pub color: Option<String>,
}

/// One row in a [`List`].
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct ListItem {
    /// Primary title text.
    pub title: String,
    /// Optional secondary description line.
    #[serde(default)]
    pub description: Option<String>,
    /// Optional leading Phosphor icon glyph.
    #[serde(default)]
    pub icon: Option<String>,
    /// Optional badge shown before the title.
    #[serde(default)]
    pub badge: Option<ListItemBadge>,
    /// Right-aligned action buttons.
    #[builder(default)]
    #[serde(default)]
    pub actions: Vec<ListItemAction>,
}

/// A scrollable list of rich rows with optional icon, badge, description, and
/// per-row action buttons. Render with [`List::show`], which reports the
/// clicked row or action.
///
/// ```
/// use thoth_plugin_sdk::components::{List, ListItem};
///
/// let list = List::builder()
///     .items(vec![ListItem::builder().title("My request").build()])
///     .build();
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct List {
    /// The rows, in order.
    #[builder(default)]
    #[serde(default)]
    pub items: Vec<ListItem>,
    /// Message shown when `items` is empty.
    #[serde(default)]
    pub empty_label: Option<String>,
}
