#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

/// A right-aligned, tooltipped icon action shown in a [`SidebarHeader`].
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
pub struct SidebarHeaderAction<'a> {
    /// The icon glyph (a Phosphor character).
    pub icon: &'a str,
    /// Tooltip shown on hover.
    pub tooltip: &'a str,
}

/// A uniform sidebar section header: a panel title, optional trailing text, and
/// optional right-aligned icon actions, followed by a divider.
///
/// Rendering through [`SidebarHeader::show`](Self::show) reports which action
/// was clicked; the plain [`egui::Widget`] impl discards that.
///
/// ```
/// use thoth_plugin_sdk::components::{SidebarHeader, SidebarHeaderAction};
///
/// let header = SidebarHeader::builder()
///     .title("RECENT FILES")
///     .trailing_text("3 of 12")
///     .actions(vec![SidebarHeaderAction::builder().icon("\u{e3d0}").tooltip("Clear").build()])
///     .build();
/// ```
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
pub struct SidebarHeader<'a> {
    /// Section title, rendered as a panel header (small, typically upper-case).
    pub title: &'a str,
    /// Optional muted text shown on the right (e.g. a count like "3 of 12").
    #[serde(default)]
    pub trailing_text: Option<&'a str>,
    /// Optional right-aligned icon buttons. The clicked index is reported by
    /// [`SidebarHeader::show`](Self::show).
    #[builder(default)]
    #[serde(default, borrow)]
    pub actions: Vec<SidebarHeaderAction<'a>>,
}
