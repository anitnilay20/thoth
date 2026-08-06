//! A **data-bound** render node: the plugin publishes a dataset to the host
//! (via `dataset-bus.publish`) and embeds the returned `handle` here. The host
//! draws the data itself — as a **table**, a **JSON** tree, or **raw**
//! (pretty-printed JSON text), user-switchable — reading the rows from its
//! single-owned registry through the installed resolver ([`crate::dataset`]).
//! The plugin delegates *display* and never holds the rows in its own UI.

use bon::Builder;
use serde::{Deserialize, Serialize};

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

    /// Draw the referenced dataset (table, JSON tree, or raw JSON) from the
    /// host registry.
    ///
    /// The header's "Charts" shortcut emits a click on the reserved
    /// [`OPEN_IN_CHARTS`](crate::actions::OPEN_IN_CHARTS) id into `events`; the
    /// host intercepts it and opens Chart Studio bound to the emitting plugin's
    /// tab. All other controls (view toggle, Copy) are handled in-widget.
    pub fn show(&self, ui: &mut egui::Ui, events: &mut Vec<crate::render_node::UiEvent>) {
        use crate::components::{SelectOption, Typography, TypographyVariant};
        use crate::dataset::{renderers, resolve_dataset};
        use crate::theme::{ThemeColors, with_alpha};

        let Some(page) = resolve_dataset(&self.handle, Self::LIMIT) else {
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
        let mut view_options: Vec<SelectOption> = ["table", "json", "raw"]
            .iter()
            .map(|v| {
                SelectOption::builder()
                    .value(*v)
                    .label(match *v {
                        "json" => "JSON",
                        "raw" => "Raw",
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
        let mut view: String = ui
            .data(|d| d.get_temp::<String>(mem_id))
            .filter(|v| view_options.iter().any(|o| &o.value == v))
            .unwrap_or_else(|| "table".to_string());

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
                    self.header(ui, events, &page, node_id, view_options, &mut view);
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
        ui.data_mut(|d| d.insert_temp(mem_id, view.clone()));

        self.body(ui, events, &page, node_id, &view);
    }

    /// Draw the header strip's controls: the view selector, the row-count label,
    /// then the trailing actions (Copy · Export · Charts) pushed to the right
    /// edge by a flexible spacer.
    fn header(
        &self,
        ui: &mut egui::Ui,
        events: &mut Vec<crate::render_node::UiEvent>,
        page: &crate::dataset::DatasetPage,
        node_id: &str,
        view_options: Vec<crate::components::SelectOption>,
        view: &mut String,
    ) {
        use crate::components::{
            Button, ButtonColor, ButtonType, Select, SelectOption, Size, Typography,
            TypographyVariant,
        };
        use crate::render_node::UiEvent;

        // Design `.viewsel` is a 28px-tall select trigger at 12.5px — exactly
        // `Size::Medium`'s field metrics, so the shared `Select` is used as-is —
        // and it leads with a glyph for the current view.
        let view_glyph = match view.as_str() {
            "json" => egui_phosphor::regular::BRACKETS_CURLY,
            "raw" => egui_phosphor::regular::CODE,
            _ => egui_phosphor::regular::TABLE,
        };
        if let Some(v) = Select::builder()
            .id(format!("{node_id}_views"))
            .value(view.clone())
            .options(view_options)
            .icon(view_glyph)
            .size(Size::Medium)
            .width(136.0)
            .build()
            .show(ui)
            .inner
            .selected
        {
            *view = v;
        }
        // The resolver caps a read at LIMIT, so the fallback count reflects
        // the rows actually drawn rather than over-reporting a large dataset.
        let count = self.caption.clone().unwrap_or_else(|| {
            let shown = page.rows.len() as u64;
            if page.total > shown {
                format!("{shown} of {} rows", page.total)
            } else {
                format!("{} rows", page.total)
            }
        });
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
        // rightmost-first to read Copy · Export · Charts left-to-right on screen.
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui
                .add(
                    Button::builder()
                        .label("Charts")
                        .icon(egui_phosphor::regular::CHART_LINE)
                        .button_type(ButtonType::Text)
                        .color(ButtonColor::Primary)
                        .hover_text("Open in Chart Studio")
                        .build(),
                )
                .clicked()
            {
                events.push(UiEvent {
                    id: crate::actions::OPEN_IN_CHARTS.to_string(),
                    kind: "click".to_string(),
                    value: String::new(),
                });
            }

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

            // `copy` is handled in-widget (no plugin round-trip).
            ui.add(
                Button::builder()
                    .label("Copy")
                    .icon(egui_phosphor::regular::COPY)
                    .button_type(ButtonType::Text)
                    .hover_text("Copy as JSON")
                    .copy(records_json(page, true))
                    .build(),
            );
        });
    }

    /// Draw the body: the selected view's content, flush against the container
    /// with no padding of its own (design `.dvbody{padding:0}`).
    fn body(
        &self,
        ui: &mut egui::Ui,
        events: &mut Vec<crate::render_node::UiEvent>,
        page: &crate::dataset::DatasetPage,
        node_id: &str,
        view: &str,
    ) {
        use crate::components::{
            Code, ColumnType, JsonTree, TableView, Typography, TypographyVariant,
        };
        use crate::dataset::{PluginRenderResult, render_with_plugin};
        use crate::render_node::RenderNode;

        egui::ScrollArea::both()
            .id_salt((node_id, "data_view_scroll"))
            .show(ui, |ui| match view {
                "json" => {
                    JsonTree::builder()
                        .id(format!("{node_id}_tree"))
                        .value(records_value(page))
                        // DataView already owns the rounded panel and hairline
                        // edge, so the tree must not draw a second one inside it
                        // (same reason `TableView` is unframed here).
                        .framed(false)
                        .build()
                        .show(ui);
                }
                "raw" => {
                    ui.add(
                        Code::builder()
                            .value(records_json(page, true))
                            .language("json")
                            .build(),
                    );
                }
                // A renderer plugin: the host reads the rows, gates consent, runs
                // the plugin, and hands back a RenderNode tree we draw here.
                // Renderer output is **display-only** — the producing plugin is
                // stateless (push model) and can't handle events, so any
                // interactions are discarded rather than routed to the producer.
                plugin if plugin.starts_with("plugin:") => {
                    match render_with_plugin(&plugin["plugin:".len()..], &self.handle) {
                        PluginRenderResult::Rendered(mut node) => {
                            node.show(ui, &mut Vec::new())
                        }
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
                        .map(|row| {
                            row.iter()
                                .zip(&page.columns)
                                .map(|(cell, col)| {
                                    RenderNode::typed_cell(
                                        &typed_cell(cell, &col.type_hint),
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
            });
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
