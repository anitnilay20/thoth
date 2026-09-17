#[cfg(feature = "egui")]
mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

/// One entry in a [`ContextMenu`].
///
/// A separator is an item too, rather than a gap the caller inserts, so a menu
/// built by filtering a list cannot end up with a rule at its top or bottom.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct ContextMenuItem {
    /// What the entry does, e.g. "Copy value".
    #[builder(default)]
    #[serde(default)]
    pub label: String,
    /// Optional leading Phosphor glyph.
    #[serde(default)]
    pub icon: Option<String>,
    /// Keyboard equivalent, shown right-aligned and quietened — a reminder,
    /// not a control.
    #[serde(default)]
    pub shortcut: Option<String>,
    /// Draw a tick, for entries that represent a current choice.
    #[builder(default)]
    #[serde(default)]
    pub checked: bool,
    /// Shown but not selectable, so a menu keeps a stable shape instead of
    /// entries appearing and vanishing between openings.
    #[builder(default)]
    #[serde(default)]
    pub disabled: bool,
    /// A rule instead of an entry. Its other fields are ignored.
    #[builder(default)]
    #[serde(default)]
    pub separator: bool,
}

impl ContextMenuItem {
    /// A dividing rule.
    pub fn separator() -> Self {
        Self {
            separator: true,
            ..Self::default()
        }
    }
}

/// A right-click menu.
///
/// Render inside egui's context-menu closure, which owns opening, placement and
/// dismissal:
///
/// ```ignore
/// response.context_menu(|ui| {
///     if let Some(picked) = ContextMenu::builder().items(items).build().show(ui) {
///         // `picked` indexes `items`
///     }
/// });
/// ```
///
/// [`show`](ContextMenu::show) reports the index of the chosen entry and closes
/// the menu, so a caller matches on position rather than on label text.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct ContextMenu {
    /// Entries, in order.
    #[builder(default)]
    #[serde(default)]
    pub items: Vec<ContextMenuItem>,
    /// Minimum width in points. Defaults to the design's 148px, which is wide
    /// enough that a short label does not produce a cramped menu.
    #[builder(default = 148.0)]
    #[serde(default = "default_min_width")]
    pub min_width: f32,
}

fn default_min_width() -> f32 {
    148.0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_separator_ignores_its_other_fields() {
        let sep = ContextMenuItem::separator();
        assert!(sep.separator);
        assert!(sep.label.is_empty());
        assert!(!sep.checked);
    }

    #[test]
    fn a_menu_defaults_to_the_design_width() {
        let menu = ContextMenu::builder().build();
        assert_eq!(menu.min_width, 148.0);
        assert!(menu.items.is_empty());
    }

    #[test]
    fn items_round_trip_through_serialization() {
        // The node crosses the plugin boundary as JSON, so an item's shortcut
        // and checked state have to survive or a plugin's menu arrives wrong.
        let menu = ContextMenu::builder()
            .items(vec![
                ContextMenuItem::builder()
                    .label("Copy value")
                    .shortcut("⌘C")
                    .build(),
                ContextMenuItem::separator(),
                ContextMenuItem::builder()
                    .label("Table")
                    .checked(true)
                    .disabled(true)
                    .build(),
            ])
            .build();

        let back: ContextMenu =
            serde_json::from_str(&serde_json::to_string(&menu).unwrap()).unwrap();
        assert_eq!(back.items.len(), 3);
        assert_eq!(back.items[0].shortcut.as_deref(), Some("⌘C"));
        assert!(back.items[1].separator);
        assert!(back.items[2].checked && back.items[2].disabled);
    }

    #[test]
    fn an_item_without_a_shortcut_deserializes_from_a_bare_label() {
        let item: ContextMenuItem = serde_json::from_str(r#"{"label":"Copy path"}"#).unwrap();
        assert_eq!(item.label, "Copy path");
        assert!(item.shortcut.is_none());
        assert!(!item.separator);
    }
}
