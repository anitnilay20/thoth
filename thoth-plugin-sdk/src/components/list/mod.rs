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

/// How a [`List`]'s rows are chromed.
///
/// The design sheet draws list rows two ways, and they are not interchangeable:
///
/// * In a sidebar panel each row is its own **card** — an app-mockup `.card`:
///   base fill, hairline edge, 6px gap to the next row, no dividers.
/// * Inside a framed list each row is **flush** — a display-sheet `.li`:
///   transparent, separated from its neighbour by a 1px inset hairline.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ListStyle {
    /// Pick from context: [`Flush`](Self::Flush) for a [`framed`](List::framed)
    /// list or a [`compact`](List::compact) strip, [`Card`](Self::Card)
    /// otherwise. This is the default.
    #[default]
    Auto,
    /// Sidebar shape — app-mockup `.card`: each row is a card on the panel
    /// (base fill, hairline edge, 10px padding, 6px gaps, 3px accent stripe).
    Card,
    /// Framed/dense shape — display `.li`: transparent rows with 8px padding, a
    /// 1px hairline between neighbours, and a 2px accent stripe.
    Flush,
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
    /// A 48×48 embedded image: raw bytes (e.g. a PNG) a plugin ships with its
    /// wasm, rendered via egui's image loaders (installed by the host). Unlike
    /// [`IconFile`](Self::IconFile) this never touches host disk, so plugins can
    /// use it to show real logos.
    Image {
        /// Stable cache key for the image loader (e.g. `"bytes://pg"`).
        uri: String,
        /// Encoded image bytes (PNG/JPEG/…).
        bytes: Vec<u8>,
    },
}

/// Per-tier type overrides for a [`List`]'s rows.
///
/// Every field defaults to "leave it alone", so a list that doesn't set one
/// renders exactly as it always has. Reach for this when a screen's rows carry a
/// different type ramp from the default flush/card row — design `.nrow`'s
/// 12.5/11.5/10.5 stack, or `.pl .by`'s monospace author line.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub struct ListTextStyle {
    /// Title size in points; the row shape's own size when unset.
    #[serde(default)]
    pub title_size: Option<f32>,
    /// Thicken the title. Defaults to `false`.
    #[builder(default)]
    #[serde(default)]
    pub title_bold: bool,
    /// Title colour (hex or theme token) for every row; `fg` when unset. A single
    /// row can override it again with [`ListItem::title_color`].
    #[serde(default)]
    pub title_color: Option<String>,
    /// Description size in points; [`FONT_CAPTION`](crate::theme::FONT_CAPTION)
    /// when unset.
    #[serde(default)]
    pub description_size: Option<f32>,
    /// Description colour (hex or theme token); `fg-muted` when unset.
    #[serde(default)]
    pub description_color: Option<String>,
    /// Meta-line size in points; 10.5 when unset.
    #[serde(default)]
    pub meta_size: Option<f32>,
    /// Meta-line colour (hex or theme token); `fg-faint` when unset.
    #[serde(default)]
    pub meta_color: Option<String>,
    /// Render the meta line in the monospace family — design `.pl .by`. Defaults
    /// to `false` (proportional).
    #[builder(default)]
    #[serde(default)]
    pub meta_mono: bool,
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
    /// Plain trailing text, with no chip chrome at all — design `.cat .c`, a
    /// bare count pinned to the row's right edge.
    Text {
        /// The text.
        text: String,
        /// Colour (hex or token); defaults to `fg-muted`.
        #[serde(default)]
        color: Option<String>,
        /// Render in the monospace family — design `.cat .c{font-family:mono}`.
        #[serde(default)]
        mono: bool,
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
    /// Optional third, quietest text line below the description (timestamps,
    /// origins). Rendered one step under the description in size and colour.
    #[serde(default)]
    pub meta: Option<String>,
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
    /// Optional title colour for *this* row (hex or theme token), overriding both
    /// the row shape's default and [`ListTextStyle::title_color`] — design
    /// `.cat.on{color:var(--accent)}`, where a selected category's label itself
    /// goes accent rather than just its background.
    #[serde(default, rename = "title-color")]
    pub title_color: Option<String>,
    /// Persistent highlight — used for the active/selected row.
    #[builder(default)]
    #[serde(default)]
    pub selected: bool,
}

/// A scrollable list of rich rows with optional prefix, badge, description, tags,
/// postfix, and per-row action buttons. Render with [`List::show`], which reports
/// the clicked row, action, or postfix.
///
/// Rows are sized by their content, never pinned to a fixed height: a row is its
/// padding around the taller of its leading media and its stacked text. See
/// [`ListStyle`] for the two chrome shapes and how they're picked.
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
    /// Use compact rows (navigation / category strips) — the dense 22px
    /// data-row height from design `.dr`. No description, tile prefix, or tags.
    #[builder(default)]
    #[serde(default)]
    pub compact: bool,
    /// How rows are chromed — cards on a panel, or flush rows in a frame.
    /// Defaults to [`ListStyle::Auto`], which picks from `framed`/`compact`.
    #[builder(default)]
    #[serde(default)]
    pub style: ListStyle,
    /// Draw a separator line between rows. Defaults to `true`. Only applies to
    /// [`ListStyle::Flush`] rows — design puts a hairline between the rows of a
    /// framed `.list`, never between sidebar cards (which are already separated
    /// by a gap and their own edge) or between `.dr`-height compact rows.
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
    /// Per-tier type overrides for the rows' title / description / meta lines.
    /// Defaults to the row shape's own ramp.
    #[builder(default)]
    #[serde(default, rename = "text-style")]
    pub text_style: ListTextStyle,
}
