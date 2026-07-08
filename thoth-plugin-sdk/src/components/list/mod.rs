#[cfg(feature = "egui")]
mod ui;

#[cfg(feature = "egui")]
pub use ui::ListEvent;

use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::components::{Button, IconButton, Progress};

fn default_true() -> bool {
    true
}

/// A right-aligned icon action on a [`ListItem`] (hover-revealed trailing icons).
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct ListItemAction {
    /// The action's Phosphor glyph.
    pub icon: String,
    /// Optional tooltip shown on hover.
    #[serde(default)]
    pub tooltip: Option<String>,
}

/// A colored badge shown *before* a [`ListItem`]'s title (e.g. an HTTP method).
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct ListItemBadge {
    /// Badge text.
    pub text: String,
    /// Fill colour as a `#rrggbb` hex string or a theme token; defaults to the
    /// secondary accent. The text colour is chosen automatically for contrast.
    #[serde(default)]
    pub color: Option<String>,
}

/// A leading element rendered before a row's content area.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ListItemPrefix {
    /// A single Phosphor glyph; `color` is a hex/token, defaults to muted fg.
    Icon {
        /// Glyph to render.
        glyph: String,
        /// Optional colour (hex or theme token).
        #[serde(default)]
        color: Option<String>,
    },
    /// A 32×32 rounded tile with a centred glyph, tinted by `color`.
    IconTile {
        /// Glyph to render.
        glyph: String,
        /// Accent colour (hex or theme token) for the glyph and tile tint.
        color: String,
    },
    /// A 48×48 image loaded from a host filesystem path. **Host-only**: skipped
    /// by serde so it can never cross the plugin→host wire (a plugin can't turn
    /// list rendering into a local-file read); the host constructs it directly
    /// in Rust.
    #[serde(skip)]
    IconFile {
        /// Filesystem path to a PNG/ICO icon.
        path: String,
    },
}

/// An always-visible element on the right of a row's title (unlike hover-revealed
/// [`actions`](ListItem::actions)).
#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ListItemPostfix {
    /// A small pill badge.
    Badge {
        /// Badge text.
        text: String,
        /// Fill colour (hex or token); defaults to the secondary accent.
        #[serde(default)]
        bg: Option<String>,
        /// Text colour (hex or token); defaults to a contrasting colour.
        #[serde(default)]
        fg: Option<String>,
    },
    /// A full button. Reported via [`ListEvent::PostfixClicked`].
    Button(Button),
    /// A single icon button. Reported via [`ListEvent::PostfixClicked`].
    IconButton(IconButton),
    /// An embedded [`Progress`] bar (constrained to ~80px wide). Carries its own
    /// value/colour/height, so callers reuse the shared component rather than a
    /// bespoke bar.
    Progress(Progress),
}

/// One row in a [`List`].
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct ListItem {
    /// Primary title text.
    pub title: String,
    /// Optional secondary description line (`\n` splits into two lines).
    #[serde(default)]
    pub description: Option<String>,
    /// Optional leading element rendered before the content area.
    #[serde(default)]
    pub prefix: Option<ListItemPrefix>,
    /// Optional badge shown *before* the title.
    #[serde(default)]
    pub badge: Option<ListItemBadge>,
    /// Optional always-visible element on the right of the title.
    #[serde(default)]
    pub postfix: Option<ListItemPostfix>,
    /// Hover-revealed trailing action icons.
    #[builder(default)]
    #[serde(default)]
    pub actions: Vec<ListItemAction>,
    /// Optional category/tag pills rendered below the description.
    #[builder(default)]
    #[serde(default)]
    pub tags: Vec<String>,
    /// Optional left accent border colour (hex or token); non-compact rows only.
    #[serde(default)]
    pub accent: Option<String>,
    /// Persistent highlight — used for the active/selected row.
    #[builder(default)]
    #[serde(default)]
    pub selected: bool,
}

/// A scrollable list of rich rows with optional prefix, badge, description, tags,
/// postfix, and per-row action buttons. Render with [`List::show`], which reports
/// the clicked row, action, or postfix.
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
#[non_exhaustive]
pub struct List {
    /// Widget id used for event routing.
    #[builder(default)]
    #[serde(default)]
    pub id: String,
    /// The rows, in order.
    #[builder(default)]
    #[serde(default)]
    pub items: Vec<ListItem>,
    /// Message shown when `items` is empty.
    #[serde(default)]
    pub empty_label: Option<String>,
    /// Use compact 26px rows (navigation / category strips). No description,
    /// tile prefix, or tags.
    #[builder(default)]
    #[serde(default)]
    pub compact: bool,
    /// Draw a separator line between rows. Defaults to `true`.
    #[builder(default = true)]
    #[serde(default = "default_true")]
    pub show_separators: bool,
    /// Wrap the list in a bordered, filled card (panel background + surface
    /// border + rounded corners + margin). Defaults to `false`.
    #[builder(default)]
    #[serde(default)]
    pub framed: bool,
    /// Shrink the scroll area to content height instead of filling available
    /// space. Use for inline strips; default `false` for sidebar lists.
    #[builder(default)]
    #[serde(default)]
    pub shrink_to_fit: bool,
    /// Cap the scroll area at this height (px) and scroll beyond it.
    #[serde(default)]
    pub max_height: Option<f32>,
}
