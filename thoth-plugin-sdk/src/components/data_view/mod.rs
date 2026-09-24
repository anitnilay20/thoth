//! A **data-bound** render node: the plugin publishes a dataset to the host
//! (via `dataset-bus.publish`) and embeds the returned `handle` here. The host
//! draws the data itself — as a **table**, a **JSON** tree, **raw**
//! (pretty-printed JSON text) or a **chart** of a grouped result,
//! user-switchable — reading the rows from its single-owned registry through
//! the installed resolver ([`crate::dataset`]). The plugin delegates *display*
//! and never holds the rows in its own UI.
//!
//! For a *document* rather than a dataset, reach for
//! [`TextView`](crate::components::TextView): this node's table picker, format
//! switcher, Copy and Export all answer questions a log file does not raise.
//!
//! A document is not always one table. When the producer names several
//! ([`DataView::tables`]) the header leads with a picker for them, because
//! which table is being shown is the first thing the pane has to answer.

use bon::Builder;
use serde::{Deserialize, Serialize};

/// One of the tables a document holds, as offered by a [`DataView`]'s picker.
///
/// A file is not always one table — a JSON envelope carries a collection per
/// key, a database a table per relation — and a viewer that shows only the
/// first of them misrepresents the file.
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct DataTable {
    /// Stable value, reported back in the [`SELECT_TABLE`] event and matched
    /// against [`DataView::selected_table`].
    ///
    /// [`SELECT_TABLE`]: crate::actions::SELECT_TABLE
    pub value: String,
    /// Name shown in the picker.
    pub label: String,
    /// What is known about this table — its row count once it has been read,
    /// its size before that. Shown beside the name in the menu, and beside the
    /// label in the trigger for the selected one (design `.tablemenu .n` /
    /// `.select .cnt`).
    #[serde(default)]
    pub detail: Option<String>,
}

/// Renders a host-owned dataset (referenced by `handle`) as a table, JSON tree,
/// or raw JSON text.
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct DataView {
    /// Stable id for this node (persists the table/JSON/raw toggle across frames).
    #[builder(default)]
    #[serde(default)]
    pub id: String,
    /// Registry handle returned by `dataset-bus.publish`.
    pub handle: String,
    /// Optional header summary shown in place of the auto `"N rows"` count —
    /// lets the producer surface a richer line (e.g. `"100 rows (capped) · SELECT 101"`).
    #[serde(default)]
    pub caption: Option<String>,
    /// A command for the JSON view's tree, from the host's shortcut handling.
    /// Ignored by the other views, which have no tree to act on.
    #[serde(default)]
    pub tree_action: Option<crate::components::TreeAction>,
    /// View to open in the first time this node is shown — `"table"`, `"json"`,
    /// `"raw"`, `"chart"`, or `"plugin:<id>"`. Defaults to `"table"`.
    ///
    /// Only the *initial* view: once the user picks one it is remembered per
    /// node, and this is ignored.
    #[serde(default)]
    pub default_view: Option<String>,
    /// The tables this document holds. Empty — the common case, a file that is
    /// one table — hides the picker entirely.
    ///
    /// A single entry still draws it, greyed out: a document with one
    /// collection has a name worth showing, and a control that appears only
    /// once there are two of something is a control nobody finds.
    #[builder(default)]
    #[serde(default)]
    pub tables: Vec<DataTable>,
    /// Which of [`tables`](DataView::tables) the `handle` currently points at.
    #[serde(default, rename = "selected-table")]
    pub selected_table: Option<String>,
}

#[cfg(feature = "egui")]
impl DataView {
    /// Max rows the host draws (the registry also caps a single read).
    const LIMIT: u32 = 1000;

    /// Header strip height — design `.dvbar{height:40px}`.
    const HEAD_H: f32 = 40.0;
    /// Header strip side padding — design `.dvbar{padding:0 12px}`.
    const HEAD_PAD_X: i8 = 12;
    /// Gap between header strip controls — design `.dvbar{gap:12px}`.
    const HEAD_GAP: f32 = 12.0;
    /// Row-count label size — design `.rcount{font-size:12px}` (monospace).
    const COUNT_FONT: f32 = 12.0;
    /// The strip's inset top and bottom hairlines — design `surface1 @ 26%`.
    const HEAD_DIVIDER_ALPHA: u8 = 66;
    /// Which table comes first and gets the room — design
    /// `.dvbar .tablesel{width:212px}`.
    const TABLE_SEL_W: f32 = 212.0;
    /// Its menu is wider than its trigger — design
    /// `.menu.tablemenu{min-width:248px; max-height:340px}`.
    const TABLE_MENU_W: f32 = 248.0;
    const TABLE_MENU_MAX_H: f32 = 340.0;
    /// How to draw it comes second, and narrower — design
    /// `.dvbar .viewsel{width:124px}`.
    const VIEW_SEL_W: f32 = 124.0;

    /// Draw the referenced dataset (table, JSON tree, or raw JSON) from the
    /// host registry.
    ///
    /// The table picker emits the chosen table on the reserved
    /// [`SELECT_TABLE`](crate::actions::SELECT_TABLE) id into `events`, because
    /// pointing the node at another table is the producer's work, not the
    /// view's. Everything else — the view toggle, Copy, Export — is handled
    /// in-widget or on [`EXPORT_DATASET`](crate::actions::EXPORT_DATASET).
    pub fn show(&self, ui: &mut egui::Ui, events: &mut Vec<crate::render_node::UiEvent>) {
        use crate::components::{SelectOption, Typography, TypographyVariant};
        use crate::dataset::{renderers, resolve_dataset};
        use crate::theme::{ThemeColors, with_alpha};

        // How many rows exist, without reading any. `total` is a lookup; the
        // page below is not, so it must not be fetched unless a view needs it.
        let access = crate::dataset::dataset_access();
        let total = match access {
            Some(access) => Some((access.total)(&self.handle)),
            None => resolve_dataset(&self.handle, 0).map(|p| p.total),
        };
        let Some(total) = total else {
            ui.add(
                Typography::builder()
                    .text("This dataset is no longer available.")
                    .variant(TypographyVariant::BodyMuted)
                    .build(),
            );
            return;
        };

        // Fall back to the (unique) handle when no explicit id is set, so two
        // id-less DataViews don't share egui state (view toggle, scroll pos).
        let node_id = if self.id.is_empty() {
            self.handle.as_str()
        } else {
            self.id.as_str()
        };

        // View options: built-in table/json/raw + one per installed renderer
        // plugin (value "plugin:<id>"), shown in a dropdown like Export.
        let renderer_plugins = renderers();
        // Ways of drawing *data*. A document has one sensible rendering and
        // belongs in `TextView`, not behind a format switcher here.
        let mut view_options: Vec<SelectOption> = ["table", "json", "raw", "chart"]
            .iter()
            .map(|v| {
                SelectOption::builder()
                    .value(*v)
                    .label(match *v {
                        "json" => "JSON",
                        "raw" => "Raw",
                        "chart" => "Chart",
                        _ => "Table",
                    })
                    .build()
            })
            .collect();
        for r in &renderer_plugins {
            view_options.push(
                SelectOption::builder()
                    .value(format!("plugin:{}", r.id))
                    .label(r.label.clone())
                    .build(),
            );
        }

        // Current view, remembered across frames; falls back to Table if a
        // previously-selected renderer plugin is no longer installed.
        let mem_id = ui.make_persistent_id((node_id, "data_view_view"));
        let fallback = self
            .default_view
            .clone()
            .filter(|v| view_options.iter().any(|o| &o.value == v))
            .unwrap_or_else(|| "table".to_string());
        let mut view: String = ui
            .data(|d| d.get_temp::<String>(mem_id))
            .filter(|v| view_options.iter().any(|o| &o.value == v))
            .unwrap_or(fallback);
        let opened_with = view.clone();

        let colors = ThemeColors::from_ctx(ui.ctx());
        // No outer frame, corners or edge of its own. `DataView` always fills a
        // surface that already floats — a dock leaf, or a plugin's results pane —
        // and that surface owns the fill, hairline and rounding. Adding them here
        // insets the whole view from its container, which reads as a stray margin
        // around the data. See app-mockup.html, where `.dvbar` and the grid run
        // edge to edge inside the results panel.
        ui.spacing_mut().item_spacing.y = 0.0;

        // Header strip — design `.dvbar`: mantle fill, 40px tall, 12px side
        // padding, 12px gaps, and a 1px hairline along *both* its top and bottom
        // edges. [View ▾] · count · <spacer> · Copy · Export · Charts.
        let head = egui::Frame::NONE
            .fill(colors.bg_panel)
            .inner_margin(egui::Margin::symmetric(Self::HEAD_PAD_X, 0))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = Self::HEAD_GAP;
                ui.horizontal(|ui| {
                    ui.set_min_height(Self::HEAD_H);
                    self.header(ui, events, total, node_id, view_options, &mut view);
                });
            });
        let rule = egui::Stroke::new(
            1.0,
            with_alpha(colors.surface_raised, Self::HEAD_DIVIDER_ALPHA),
        );
        let strip = head.response.rect;
        ui.painter().hline(strip.x_range(), strip.top() + 0.5, rule);
        ui.painter()
            .hline(strip.x_range(), strip.bottom() - 0.5, rule);
        // Only a view the *user* picked is remembered. A file is a text
        // preview while it indexes and a table once it lands, so the offered
        // views change under it; persisting the fallback we computed for the
        // preview would leave a JSON file stuck in the view its half-read
        // state could offer, long after the tree became available.
        if view != opened_with {
            ui.data_mut(|d| d.insert_temp(mem_id, view.clone()));
        }

        self.body(ui, events, total, node_id, &view);
    }

    /// Draw the header strip's controls: the view selector, the row-count label,
    /// then the trailing actions (Copy · Export · Charts) pushed to the right
    /// edge by a flexible spacer.
    fn header(
        &self,
        ui: &mut egui::Ui,
        events: &mut Vec<crate::render_node::UiEvent>,
        total: u64,
        node_id: &str,
        view_options: Vec<crate::components::SelectOption>,
        view: &mut String,
    ) {
        use crate::components::{
            Button, ButtonType, Select, SelectOption, Size, Typography, TypographyVariant,
        };
        use crate::dataset::resolve_dataset;
        use crate::render_node::UiEvent;

        // Which table, before how to draw it: a pane showing one of several
        // collections has to say which one before anything it draws means
        // something. Searchable because a document can hold dozens, and a menu
        // you have to scroll to read is a list, not a picker.
        if !self.tables.is_empty() {
            let selected = self.selected_table.clone().unwrap_or_default();
            let picked = Select::builder()
                .id(format!("{node_id}_tables"))
                .value(selected.clone())
                .options(
                    self.tables
                        .iter()
                        .map(|t| {
                            SelectOption::builder()
                                .value(t.value.clone())
                                .label(t.label.clone())
                                .maybe_detail(t.detail.clone())
                                .build()
                        })
                        .collect::<Vec<_>>(),
                )
                .icon(egui_phosphor::regular::TABLE)
                // The figure beside the name is the selected table's own size,
                // not the row count of the result — that is what `.rcount` says.
                .maybe_count(
                    self.tables
                        .iter()
                        .find(|t| t.value == selected)
                        .and_then(|t| t.detail.clone()),
                )
                .size(Size::Medium)
                .width(Self::TABLE_SEL_W)
                .menu_width(Self::TABLE_MENU_W)
                .menu_max_height(Self::TABLE_MENU_MAX_H)
                .searchable(self.tables.len() > 1)
                // One table is not a choice, but it is still worth naming.
                .disabled(self.tables.len() < 2)
                .build()
                .show(ui)
                .inner
                .selected;
            if let Some(table) = picked.filter(|t| *t != selected) {
                events.push(UiEvent {
                    id: crate::actions::SELECT_TABLE.to_string(),
                    kind: "click".to_string(),
                    value: table,
                });
            }
        }

        // Design `.viewsel` is a 28px-tall select trigger at 12.5px — exactly
        // `Size::Medium`'s field metrics, so the shared `Select` is used as-is —
        // and it leads with a glyph for the current view.
        let view_glyph = match view.as_str() {
            "json" => egui_phosphor::regular::BRACKETS_CURLY,
            "raw" => egui_phosphor::regular::CODE,
            "chart" => egui_phosphor::regular::CHART_BAR,
            _ => egui_phosphor::regular::TABLE,
        };
        if let Some(v) = Select::builder()
            .id(format!("{node_id}_views"))
            .value(view.clone())
            .options(view_options)
            .icon(view_glyph)
            .size(Size::Medium)
            .width(Self::VIEW_SEL_W)
            .build()
            .show(ui)
            .inner
            .selected
        {
            *view = v;
        }
        // The true row count, not the size of a page — the views read windows,
        // so there is no "shown" figure to report and no reason to fetch rows
        // just to label them.
        let count = self
            .caption
            .clone()
            .unwrap_or_else(|| format!("{total} rows"));
        // Design `.rcount` is monospace at 12px in `fg-muted` — the figures line
        // up as the row count changes, which a proportional face won't do.
        ui.add(
            Typography::builder()
                .text(count)
                .variant(TypographyVariant::Mono)
                .color("fg_muted")
                .size(Self::COUNT_FONT)
                .build(),
        );

        // Actions hang off the right edge — the flexible spacer is the
        // right-to-left layout claiming the rest of the strip — so add
        // rightmost-first to read Copy · Export left-to-right on screen.
        //
        // Charts is no longer one of them: a bar chart of the result is a view
        // of this dataset like the table and the tree, so it belongs in the
        // view menu beside them rather than as a button that leaves the pane.
        // Chart Studio is still reached from the sidebar, where a chart bound
        // to a tab actually lives.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            // Export dropdown — lists installed exporter plugins; picking one
            // emits an EXPORT_DATASET action the host runs against this handle.
            // Value stays empty so the trigger always reads "Export" (it's an
            // action menu, not a persisted selection).
            let exporters = crate::dataset::exporters();
            if !exporters.is_empty() {
                let options = exporters
                    .iter()
                    .map(|e| {
                        SelectOption::builder()
                            .value(e.id.clone())
                            .label(format!("{} (.{})", e.label, e.extension))
                            .build()
                    })
                    .collect();
                let selected = Select::builder()
                    .id(format!("{node_id}_export"))
                    .value("")
                    .prefix_label("Export")
                    .options(options)
                    // Design `.dvbar` renders Export as a `.viewsel`, the same
                    // framed 28px trigger as the view switcher — it opens a menu,
                    // so it reads as a control rather than a flat text action.
                    .size(Size::Medium)
                    .width(92.0)
                    .build()
                    .show(ui)
                    .inner
                    .selected;
                if let Some(exporter) = selected {
                    events.push(UiEvent {
                        id: crate::actions::EXPORT_DATASET.to_string(),
                        kind: "click".to_string(),
                        value: serde_json::json!({ "handle": self.handle, "exporter": exporter })
                            .to_string(),
                    });
                }
            }

            // Copy is handled in-widget (no plugin round-trip). Serialised on the
            // click rather than through `Button::copy`, which would re-encode the
            // whole page into a String every frame just to have it ready.
            if ui
                .add(
                    Button::builder()
                        .label("Copy")
                        .icon(egui_phosphor::regular::COPY)
                        .button_type(ButtonType::Text)
                        .hover_text("Copy as JSON")
                        .build(),
                )
                .clicked()
                && let Some(page) = resolve_dataset(&self.handle, Self::LIMIT)
            {
                ui.ctx().copy_text(records_json(&page, true));
            }
        });
    }

    /// Draw the body: the selected view's content, flush against the container
    /// with no padding of its own (design `.dvbody{padding:0}`).
    fn body(
        &self,
        ui: &mut egui::Ui,
        events: &mut Vec<crate::render_node::UiEvent>,
        total: u64,
        node_id: &str,
        view: &str,
    ) {
        use crate::components::{
            Code, ColumnType, JsonTree, TableView, Typography, TypographyVariant,
        };
        use crate::dataset::{PluginRenderResult, render_with_plugin, resolve_dataset};
        use crate::render_node::RenderNode;

        // The grid scrolls itself: `TableView` wraps it in a horizontal scroll area
        // and `TableBuilder::body.rows` virtualises vertically off *its* viewport.
        // Nesting it in the body's own scroll area would hand it an infinite
        // viewport, and it would lay out every row of the page instead of the
        // visible ones. Every other view is plain content, and scrolls here.
        scrolled(ui, !table_view(view), (node_id, "data_view_scroll"), |ui| {
            match view {
                "json" => {
                    // Bound to the handle: the tree reads only the nodes it is
                    // showing, so this draws a dataset far larger than any page
                    // and fetches nothing at all until a record is expanded.
                    let mut tree = JsonTree::builder()
                        .id(format!("{node_id}_tree"))
                        // DataView already owns the rounded panel and hairline
                        // edge, so the tree must not draw a second one inside it
                        // (same reason `TableView` is unframed here).
                        .framed(false)
                        .build();
                    tree.action = self.tree_action;
                    if crate::dataset::dataset_access().is_some() {
                        tree.handle = Some(self.handle.clone());
                    } else if let Some(page) = resolve_dataset(&self.handle, Self::LIMIT) {
                        tree.value = records_value(&page);
                    }
                    tree.show(ui);
                }
                // Bars for a grouped result. Nothing is aggregated here: the
                // chart draws the rows it is given, so the query is the only
                // place a total is ever computed.
                "chart" => {
                    if let Some(page) = resolve_dataset(&self.handle, Self::LIMIT) {
                        chart(ui, &page);
                    }
                }
                // Raw is a text dump, so it is inherently bounded by LIMIT --
                // and it is the one view that has to materialize to render.
                "raw" => {
                    let text = resolve_dataset(&self.handle, Self::LIMIT)
                        .map(|page| records_json(&page, true))
                        .unwrap_or_default();
                    ui.add(Code::builder().value(text).language("json").build());
                }
                // A renderer plugin: the host reads the rows, gates consent, runs
                // the plugin, and hands back a RenderNode tree we draw here.
                // Renderer output is **display-only** — the producing plugin is
                // stateless (push model) and can't handle events, so any
                // interactions are discarded rather than routed to the producer.
                plugin if plugin.starts_with("plugin:") => {
                    match render_with_plugin(&plugin["plugin:".len()..], &self.handle) {
                        PluginRenderResult::Rendered(mut node) => node.show(ui, &mut Vec::new()),
                        PluginRenderResult::ConsentPending => {
                            ui.add(
                                Typography::builder()
                                    .text("Approve access to render this dataset with the selected plugin.")
                                    .variant(TypographyVariant::BodyMuted)
                                    .build(),
                            );
                        }
                        PluginRenderResult::Unavailable => {
                            ui.add(
                                Typography::builder()
                                    .text("This view is unavailable.")
                                    .variant(TypographyVariant::BodyMuted)
                                    .build(),
                            );
                        }
                    }
                }
                _ => {
                    let Some(page) = resolve_dataset(&self.handle, Self::LIMIT) else {
                        return;
                    };
                    let _ = total;
                    let headers: Vec<String> =
                        page.columns.iter().map(|c| c.name.clone()).collect();
                    let column_types: Vec<ColumnType> = page
                        .columns
                        .iter()
                        .map(|c| ColumnType::from_sql(&c.type_hint))
                        .collect();
                    // Cells are styled by their column's type, so numeric and
                    // temporal values read right-aligned in tinted mono (design
                    // `.tv td.r`) instead of as plain text.
                    let rows: Vec<Vec<RenderNode>> = page
                        .rows
                        .iter()
                        .enumerate()
                        .map(|(r, row)| {
                            row.iter()
                                .zip(&page.columns)
                                .enumerate()
                                .map(|(c, (cell, col))| {
                                    // A NULL is not an empty cell: the mask is
                                    // the only thing that separates a field the
                                    // record does not carry from one that
                                    // carries "".
                                    let value = if page.is_null(r, c) {
                                        serde_json::Value::Null
                                    } else {
                                        typed_cell(cell, &col.type_hint)
                                    };
                                    RenderNode::typed_cell(
                                        &value,
                                        ColumnType::from_sql(&col.type_hint),
                                    )
                                })
                                .collect()
                        })
                        .collect();
                    // The grid draws flush inside this container, which already
                    // owns the background, edge, and corners.
                    TableView::builder()
                        .headers(headers)
                        .rows(rows)
                        .column_types(column_types)
                        .framed(false)
                        .build()
                        .show(ui, events);
                }
            }
        });
    }
}

/// Whether `view` selects the built-in grid — the one branch of
/// [`DataView::body`] that owns its own scrolling. Kept beside the match it
/// mirrors: table is the fallback, so anything that isn't a named built-in or a
/// renderer plugin lands there.
#[cfg(feature = "egui")]
fn table_view(view: &str) -> bool {
    !matches!(view, "json" | "raw" | "chart") && !view.starts_with("plugin:")
}

// ── Chart view ───────────────────────────────────────────────────────────────

/// Bars drawn before the head says it is showing a top slice — design's
/// `CHART_ROWS`. Beyond this a bar chart is a texture, not a reading.
#[cfg(feature = "egui")]
const CHART_ROWS: usize = 40;
/// Bar row height — design `.crow{height:var(--row)}`.
#[cfg(feature = "egui")]
const CHART_ROW_H: f32 = 22.0;
/// Track height — design `.cbar{height:9px}`.
#[cfg(feature = "egui")]
const CHART_BAR_H: f32 = 9.0;
/// Label column — design `.crow{grid-template-columns:minmax(96px,210px) …}`.
#[cfg(feature = "egui")]
const CHART_LABEL_W: (f32, f32) = (96.0, 210.0);
/// Value column — design's third grid track, `78px`.
#[cfg(feature = "egui")]
const CHART_VALUE_W: f32 = 78.0;
/// Column gap — design `.crow{gap:12px}`.
#[cfg(feature = "egui")]
const CHART_GAP: f32 = 12.0;
/// Chart padding — design `.chart{padding:10px 14px 18px}`.
#[cfg(feature = "egui")]
const CHART_PAD: (f32, f32, f32) = (10.0, 14.0, 18.0);
/// Head caption size — design `.chead{font-size:10.5px}`.
#[cfg(feature = "egui")]
const CHART_HEAD_SIZE: f32 = 10.5;
/// Label and value size — design `.clabel`/`.cval{font-size:11.5px}`.
#[cfg(feature = "egui")]
const CHART_TEXT_SIZE: f32 = 11.5;
/// Fill of the bar itself — design `color-mix(accent-2 52%, transparent)`: a
/// data bar, not an accent wash.
#[cfg(feature = "egui")]
const CHART_BAR_ALPHA: u8 = 133; // 52% of 255
/// Bar corner radius — design `.cbar{border-radius:2px}`.
#[cfg(feature = "egui")]
const CHART_BAR_RADIUS: u8 = 2;

/// The `(label, value)` columns a page can be charted by, or `None` when it
/// cannot be.
///
/// A bar needs a name and a number, and one bar per name: the first
/// non-numeric column supplies the names, the first numeric one the sizes, and
/// a name that repeats within the drawn slice means the rows were never
/// grouped. Charting them anyway would draw one bar per record and read as a
/// distribution that nothing computed.
#[cfg(feature = "egui")]
fn chart_columns(page: &crate::dataset::DatasetPage) -> Option<(usize, usize)> {
    use crate::components::ColumnType;

    let numeric = |c: &crate::dataset::DatasetColumn| {
        matches!(
            ColumnType::from_sql(&c.type_hint),
            ColumnType::Integer | ColumnType::Float
        )
    };
    let label_col = page.columns.iter().position(|c| !numeric(c))?;
    let value_col = page.columns.iter().position(numeric)?;

    let mut seen = std::collections::HashSet::new();
    let mut drawn = 0usize;
    for row in page.rows.iter().take(CHART_ROWS) {
        if !seen.insert(row.get(label_col).map(String::as_str).unwrap_or("")) {
            return None;
        }
        drawn += 1;
    }
    (drawn > 0).then_some((label_col, value_col))
}

/// Draw a page as one horizontal bar per row, per [`chart_columns`].
#[cfg(feature = "egui")]
fn chart(ui: &mut egui::Ui, page: &crate::dataset::DatasetPage) {
    use crate::components::{Typography, TypographyVariant};
    use crate::theme::{ThemeColors, with_alpha};

    let Some((label_col, value_col)) = chart_columns(page) else {
        ui.add(
            Typography::builder()
                .text("Nothing to chart yet")
                .variant(TypographyVariant::Body)
                .build(),
        );
        ui.add(
            Typography::builder()
                .text("A bar needs one row per group. Group the query by a field and compute a number over it.")
                .variant(TypographyVariant::BodyMuted)
                .build(),
        );
        return;
    };

    let rows: Vec<(&str, Option<f64>)> = page
        .rows
        .iter()
        .take(CHART_ROWS)
        .map(|row| {
            let label = row.get(label_col).map(String::as_str).unwrap_or("");
            let value = row.get(value_col).and_then(|v| v.parse::<f64>().ok());
            (label, value)
        })
        .collect();

    let colors = ThemeColors::from_ctx(ui.ctx());
    let peak = rows
        .iter()
        .filter_map(|(_, v)| v.map(f64::abs))
        .fold(0.0_f64, f64::max)
        .max(f64::MIN_POSITIVE);

    egui::Frame::NONE
        .inner_margin(egui::Margin {
            left: CHART_PAD.1 as i8,
            right: CHART_PAD.1 as i8,
            top: CHART_PAD.0 as i8,
            bottom: CHART_PAD.2 as i8,
        })
        .show(ui, |ui| {
            ui.spacing_mut().item_spacing = egui::vec2(CHART_GAP, 0.0);

            let head = match page.rows.len() > CHART_ROWS {
                true => format!(
                    "{} by {} · top {CHART_ROWS} of {}",
                    page.columns[value_col].name,
                    page.columns[label_col].name,
                    page.rows.len()
                ),
                false => format!(
                    "{} by {}",
                    page.columns[value_col].name, page.columns[label_col].name
                ),
            };
            ui.add(
                Typography::builder()
                    .text(head)
                    .variant(TypographyVariant::Mono)
                    .color("fg_muted")
                    .size(CHART_HEAD_SIZE)
                    .build(),
            );
            ui.add_space(8.0);

            // The label track flexes between its two bounds so a narrow pane
            // still leaves the bar room to be read.
            let label_w = (ui.available_width() * 0.28)
                .clamp(CHART_LABEL_W.0, CHART_LABEL_W.1)
                .min(ui.available_width());
            let bar_w = (ui.available_width() - label_w - CHART_VALUE_W - CHART_GAP * 2.0).max(0.0);

            for (label, value) in &rows {
                let (row_rect, row_resp) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), CHART_ROW_H),
                    egui::Sense::hover(),
                );
                if !ui.is_rect_visible(row_rect) {
                    continue;
                }
                if row_resp.hovered() {
                    ui.painter()
                        .rect_filled(row_rect, 0.0, colors.sidebar_hover);
                }

                super::select::ui::paint_truncated(
                    ui.painter(),
                    egui::pos2(row_rect.min.x, row_rect.center().y),
                    label,
                    egui::FontId::monospace(CHART_TEXT_SIZE),
                    colors.fg,
                    label_w,
                );

                let track = egui::Rect::from_min_size(
                    egui::pos2(
                        row_rect.min.x + label_w + CHART_GAP,
                        row_rect.center().y - CHART_BAR_H / 2.0,
                    ),
                    egui::vec2(bar_w, CHART_BAR_H),
                );
                ui.painter()
                    .rect_filled(track, CHART_BAR_RADIUS, colors.surface);
                if let Some(v) = value {
                    let filled = (v.abs() / peak).clamp(0.0, 1.0) as f32 * bar_w;
                    ui.painter().rect_filled(
                        egui::Rect::from_min_size(track.min, egui::vec2(filled, CHART_BAR_H)),
                        CHART_BAR_RADIUS,
                        with_alpha(colors.accent_secondary, CHART_BAR_ALPHA),
                    );
                }

                ui.painter().text(
                    egui::pos2(row_rect.max.x, row_rect.center().y),
                    egui::Align2::RIGHT_CENTER,
                    // Whole numbers stay whole; anything else settles at two
                    // places, so the column reads as one figure per row.
                    match value {
                        Some(v) if v.fract() == 0.0 => format!("{v:.0}"),
                        Some(v) => format!("{v:.2}"),
                        None => "—".to_string(),
                    },
                    egui::FontId::monospace(CHART_TEXT_SIZE),
                    colors.fg_muted,
                );
            }
        });
}

/// Run `content` inside the body's scroll area, or directly into `ui` when the
/// content scrolls itself.
#[cfg(feature = "egui")]
fn scrolled(
    ui: &mut egui::Ui,
    scroll: bool,
    id_salt: impl std::hash::Hash,
    content: impl FnOnce(&mut egui::Ui),
) {
    if scroll {
        egui::ScrollArea::both().id_salt(id_salt).show(ui, content);
    } else {
        content(ui);
    }
}

/// Reconstruct the page's rows as a JSON array of objects (column name → cell),
/// typing each value from its column's hint so numbers/booleans render as JSON
/// scalars rather than quoted strings in the JSON/Raw views.
#[cfg(feature = "egui")]
fn records_value(page: &crate::dataset::DatasetPage) -> serde_json::Value {
    let records: Vec<serde_json::Value> = page
        .rows
        .iter()
        .map(|row| {
            let mut obj = serde_json::Map::new();
            for (c, col) in page.columns.iter().enumerate() {
                obj.insert(
                    col.name.clone(),
                    typed_cell(row.get(c).map(String::as_str).unwrap_or(""), &col.type_hint),
                );
            }
            serde_json::Value::Object(obj)
        })
        .collect();
    serde_json::Value::Array(records)
}

/// Reconstruct a single cell as a typed JSON value from its column's SQL type
/// hint. Rows reach the host as strings (published that way), so numeric/boolean
/// columns are parsed back; anything that doesn't parse stays a string, and an
/// empty cell stays an empty string.
#[cfg(feature = "egui")]
fn typed_cell(cell: &str, type_hint: &str) -> serde_json::Value {
    use crate::components::ColumnType;
    use serde_json::Value;
    if cell.is_empty() {
        return Value::String(String::new());
    }
    match ColumnType::from_sql(type_hint) {
        // A nested column's cell is Arrow's JSON rendering of the value, so it
        // parses back into the structure the chip counts. Anything that does
        // not parse stays the text it was.
        ColumnType::Json => {
            serde_json::from_str(cell).unwrap_or_else(|_| Value::String(cell.to_string()))
        }
        ColumnType::Integer => cell
            .parse::<i64>()
            .map(Value::from)
            .unwrap_or_else(|_| Value::String(cell.to_string())),
        ColumnType::Float => cell
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(cell.to_string())),
        ColumnType::Boolean => match cell.to_ascii_lowercase().as_str() {
            "true" | "t" | "1" => Value::Bool(true),
            "false" | "f" | "0" => Value::Bool(false),
            _ => Value::String(cell.to_string()),
        },
        _ => Value::String(cell.to_string()),
    }
}

/// Serialize the page's rows to a JSON string (pretty when `pretty`).
#[cfg(feature = "egui")]
fn records_json(page: &crate::dataset::DatasetPage, pretty: bool) -> String {
    let value = records_value(page);
    if pretty {
        serde_json::to_string_pretty(&value)
    } else {
        serde_json::to_string(&value)
    }
    .unwrap_or_default()
}

#[cfg(all(test, feature = "egui"))]
mod tests {
    use super::*;

    #[test]
    fn default_view_is_carried_on_the_node() {
        let dv = DataView::builder().handle("h").default_view("json").build();
        assert_eq!(dv.default_view.as_deref(), Some("json"));

        // Absent means the built-in fallback (Table) applies.
        let plain = DataView::builder().handle("h").build();
        assert!(plain.default_view.is_none());
    }

    #[test]
    fn default_view_survives_serialization() {
        // The node crosses the plugin boundary as JSON, so the field has to
        // round-trip or a producer's chosen view would be silently dropped.
        let dv = DataView::builder().handle("h").default_view("raw").build();
        let wire = serde_json::to_string(&dv).unwrap();
        let back: DataView = serde_json::from_str(&wire).unwrap();
        assert_eq!(back.default_view.as_deref(), Some("raw"));

        // An older node without the field still deserializes.
        let legacy: DataView = serde_json::from_str(r#"{"handle":"h"}"#).unwrap();
        assert!(legacy.default_view.is_none());
    }

    #[test]
    fn a_document_with_one_table_still_names_it() {
        // The picker is hidden only when there is nothing to name — one
        // collection is drawn (disabled), none is drawn not at all.
        let none = DataView::builder().handle("h").build();
        assert!(none.tables.is_empty());

        let one = DataView::builder()
            .handle("h")
            .tables(vec![
                DataTable::builder().value("users").label("users").build(),
            ])
            .selected_table("users")
            .build();
        assert_eq!(one.tables.len(), 1);
        assert_eq!(one.selected_table.as_deref(), Some("users"));
    }

    #[test]
    fn the_table_picker_survives_serialization() {
        // The node crosses the plugin boundary as JSON, so a producer's tables
        // have to round-trip or the picker silently empties.
        let dv = DataView::builder()
            .handle("h")
            .tables(vec![
                DataTable::builder()
                    .value("users")
                    .label("users")
                    .detail("4,812")
                    .build(),
                DataTable::builder().value("logs").label("logs").build(),
            ])
            .selected_table("logs")
            .build();

        let back: DataView = serde_json::from_str(&serde_json::to_string(&dv).unwrap()).unwrap();
        assert_eq!(back.tables.len(), 2);
        assert_eq!(back.tables[0].detail.as_deref(), Some("4,812"));
        assert!(back.tables[1].detail.is_none());
        assert_eq!(back.selected_table.as_deref(), Some("logs"));

        // An older node without either field still deserializes.
        let legacy: DataView = serde_json::from_str(r#"{"handle":"h"}"#).unwrap();
        assert!(legacy.tables.is_empty());
        assert!(legacy.selected_table.is_none());
    }

    fn page(cols: &[(&str, &str)], rows: Vec<Vec<&str>>) -> crate::dataset::DatasetPage {
        crate::dataset::DatasetPage {
            columns: cols
                .iter()
                .map(|(n, t)| crate::dataset::DatasetColumn {
                    name: (*n).to_string(),
                    type_hint: (*t).to_string(),
                })
                .collect(),
            total: rows.len() as u64,
            rows: rows
                .into_iter()
                .map(|r| r.into_iter().map(str::to_string).collect())
                .collect(),
            nulls: Vec::new(),
        }
    }

    #[test]
    fn a_grouped_result_charts_by_its_name_and_its_number() {
        let grouped = page(
            &[("service", "text"), ("n", "integer")],
            vec![
                vec!["billing", "1204"],
                vec!["auth-svc", "842"],
                vec!["search-svc", "511"],
            ],
        );
        assert_eq!(chart_columns(&grouped), Some((0, 1)));
    }

    #[test]
    fn ungrouped_rows_are_not_charted() {
        // One bar per record is not a chart of anything — the view says to
        // group instead of drawing a thousand bars nobody asked for.
        let raw = page(
            &[("service", "text"), ("ms", "double")],
            vec![
                vec!["billing", "12.5"],
                vec!["billing", "31.0"],
                vec!["auth-svc", "8.2"],
            ],
        );
        assert_eq!(chart_columns(&raw), None);

        // Nor is a result with no number in it, or no name.
        assert_eq!(
            chart_columns(&page(
                &[("service", "text"), ("region", "text")],
                vec![vec!["billing", "eu-west-1"]]
            )),
            None
        );
        assert_eq!(
            chart_columns(&page(&[("n", "integer")], vec![vec!["1"]])),
            None
        );
        // An empty result has nothing to draw either.
        assert_eq!(
            chart_columns(&page(&[("service", "text"), ("n", "integer")], vec![])),
            None
        );
    }

    #[test]
    fn a_repeat_beyond_the_drawn_slice_does_not_block_the_chart() {
        // Only the rows actually drawn are checked: a duplicate at row 500 of
        // a capped result says nothing about the top 40.
        let mut rows: Vec<Vec<&str>> = (0..CHART_ROWS).map(|_| vec!["x", "1"]).collect();
        for (i, row) in rows.iter_mut().enumerate() {
            row[0] = ["a", "b", "c", "d", "e", "f", "g", "h"][i % 8];
        }
        // The first 40 are not distinct here, so this must refuse…
        assert_eq!(
            chart_columns(&page(&[("k", "text"), ("n", "integer")], rows)),
            None
        );

        // …while 40 distinct rows followed by a repeat is fine.
        let labels: Vec<String> = (0..CHART_ROWS + 5).map(|i| format!("k{i}")).collect();
        let mut rows: Vec<Vec<&str>> = labels.iter().map(|l| vec![l.as_str(), "1"]).collect();
        rows.push(vec![labels[0].as_str(), "1"]);
        assert_eq!(
            chart_columns(&page(&[("k", "text"), ("n", "integer")], rows)),
            Some((0, 1))
        );
    }

    #[test]
    fn chart_is_a_view_the_default_can_name() {
        // `default_view` is only honoured when it matches an offered view, so
        // the two lists have to agree.
        let dv = DataView::builder()
            .handle("h")
            .default_view("chart")
            .build();
        assert_eq!(dv.default_view.as_deref(), Some("chart"));
        // …and it is not the grid, so the body scrolls it.
        assert!(!table_view("chart"));
        assert!(table_view("table"));
    }
}
