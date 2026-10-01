#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

use super::{ContextMenuItem, Size};

/// A button that drops a menu of actions — design `.select` + `.menu`.
///
/// The difference from [`Select`](super::Select) is what the entries *are*. A
/// select offers values and keeps the one you pick; a menu offers actions and
/// keeps nothing. The saved-query control is a menu because "Save this query"
/// and "Settings…" sit in the same list as the queries themselves, and those
/// are not values of the same kind.
///
/// Entries are [`ContextMenuItem`]s, the same type the right-click menu uses,
/// so a separator, a tick, a shortcut hint and a second line all work here too.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct Menu {
    /// Distinguishes this menu's open state from any other on screen.
    #[builder(default)]
    #[serde(default)]
    pub id: String,
    /// What the trigger says — for a menu that carries a current choice, that
    /// choice's name.
    #[builder(default)]
    #[serde(default)]
    pub label: String,
    /// Optional leading Phosphor glyph on the trigger.
    #[serde(default)]
    pub icon: Option<String>,
    /// Entries, in order.
    #[builder(default)]
    #[serde(default)]
    pub items: Vec<ContextMenuItem>,
    /// Trigger height and type size.
    #[builder(default)]
    #[serde(default)]
    pub size: Size,
    /// Trigger width. Defaults to the width available.
    #[serde(default)]
    pub width: Option<f32>,
    /// Minimum width of the dropped sheet — design `.menu{min-width:148px}`.
    #[builder(default = 148.0)]
    #[serde(default = "default_min_width")]
    pub min_width: f32,
    /// Hover text for the trigger.
    #[serde(default)]
    pub hover_text: Option<String>,
    /// Shown but not openable.
    #[builder(default)]
    #[serde(default)]
    pub disabled: bool,
}

fn default_min_width() -> f32 {
    148.0
}
