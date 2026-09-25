#[cfg(feature = "egui")]
pub(crate) mod ui;

use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::components::Size;

/// A single option in a [`Select`].
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct SelectOption {
    /// Stable value matched against [`Select::value`].
    pub value: String,
    /// Human-readable label shown in the list.
    pub label: String,
    /// Optional trailing detail, right-aligned in monospace at the row's end —
    /// design `.tablemenu .n`, the row count beside each table's name. `None`
    /// (the default) leaves the row as label-only.
    ///
    /// When any option carries one, every row reserves the tick's width so the
    /// details line up in a column rather than stepping in and out by a glyph.
    #[serde(default)]
    pub detail: Option<String>,
}

/// The outcome of rendering a [`Select`] for one frame.
#[derive(Clone, Debug, Default)]
pub struct SelectResponse {
    /// A value the user picked this frame (a dropdown item click).
    pub selected: Option<String>,
    /// The search query, reported on the frame it changed (searchable selects
    /// only). Owners can use it to fetch or replace [`Select::options`] on
    /// demand — the dropdown always also filters the current options locally.
    pub search: Option<String>,
}

/// A dropdown select (combo box) with a custom-painted trigger and popup list.
///
/// Stateful: it owns the currently-selected [`value`](Select::value). Render
/// with [`show`](Select::show), which updates `value` on selection and reports
/// the newly-chosen value.
///
/// ```
/// use thoth_plugin_sdk::components::{Select, SelectOption};
///
/// let select = Select::builder()
///     .id("sort")
///     .value("name")
///     .options(vec![
///         SelectOption::builder().value("name").label("Name").build(),
///         SelectOption::builder().value("date").label("Date").build(),
///     ])
///     .build();
/// ```
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct Select {
    /// Stable id salt — must be unique per on-screen instance (used for the
    /// open/closed popup state and event routing).
    #[builder(default)]
    #[serde(default)]
    pub id: String,
    /// Currently selected value (matched against [`SelectOption::value`]).
    #[builder(default)]
    #[serde(default)]
    pub value: String,
    /// Available options, in display order.
    #[builder(default)]
    #[serde(default)]
    pub options: Vec<SelectOption>,
    /// Optional static prefix shown before the selected label, e.g. `"Sort: "`.
    #[serde(default)]
    pub prefix_label: Option<String>,
    /// Optional leading glyph in the trigger, before the label — design
    /// `.viewsel`/`.selbox`, which lead with a table, database or plug icon.
    #[serde(default)]
    pub icon: Option<String>,
    /// Colour for [`icon`](Select::icon) as a theme token name (e.g. `"accent"`).
    /// Defaults to the muted foreground, like the trailing caret.
    #[serde(default, rename = "icon-color")]
    pub icon_color: Option<String>,
    /// Trigger size. Defaults to [`Size::Medium`].
    #[builder(default)]
    #[serde(default)]
    pub size: Size,
    /// Fixed trigger width. When `None`, the trigger fills the available width.
    #[serde(default)]
    pub width: Option<f32>,
    /// When true, the dropdown shows a search box that filters options live
    /// (and reports query changes via [`SelectResponse::search`]), and the list
    /// is virtualized so large option sets don't all render at once.
    #[builder(default)]
    #[serde(default)]
    pub searchable: bool,
    /// Optional trailing figure in the *trigger*, between the label and the
    /// caret — design `.select .cnt`: monospace, tabular, muted. The label
    /// gives up the width it takes, so a long label ellipsises rather than
    /// pushing the figure under the caret.
    #[serde(default)]
    pub count: Option<String>,
    /// Greys the trigger out and stops it opening — design
    /// `.select[disabled]`. For a picker that has nothing to pick between: it
    /// stays on screen, still naming what is shown, instead of vanishing.
    #[builder(default)]
    #[serde(default)]
    pub disabled: bool,
    /// Minimum popover width — design `.menu.tablemenu{min-width:248px}`, a
    /// menu wider than the trigger it hangs from. The popover never narrows
    /// below the trigger, so this only ever widens it.
    #[serde(default, rename = "menu-width")]
    pub menu_width: Option<f32>,
    /// Maximum height of the popover's option list — design
    /// `.menu.tablemenu{max-height:340px}`. Unset falls back to the shared
    /// eight-row cap.
    #[serde(default, rename = "menu-max-height")]
    pub menu_max_height: Option<f32>,
}
