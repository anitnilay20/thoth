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

/// Which representation the host is currently drawing for a [`DataView`].
#[cfg(feature = "egui")]
#[derive(Clone, Copy, PartialEq, Eq)]
enum ViewMode {
    Table,
    Json,
    Raw,
}

#[cfg(feature = "egui")]
impl ViewMode {
    fn from_u8(v: u8) -> Self {
        match v {
            1 => Self::Json,
            2 => Self::Raw,
            _ => Self::Table,
        }
    }

    fn as_u8(self) -> u8 {
        match self {
            Self::Table => 0,
            Self::Json => 1,
            Self::Raw => 2,
        }
    }
}

#[cfg(feature = "egui")]
impl DataView {
    /// Max rows the host draws (the registry also caps a single read).
    const LIMIT: u32 = 1000;

    /// Draw the referenced dataset (table, JSON tree, or raw JSON) from the
    /// host registry.
    ///
    /// The header's "Charts" shortcut emits a click on the reserved
    /// [`OPEN_IN_CHARTS`](crate::actions::OPEN_IN_CHARTS) id into `events`; the
    /// host intercepts it and opens Chart Studio bound to the emitting plugin's
    /// tab. All other controls (view toggle, Copy) are handled in-widget.
    pub fn show(&self, ui: &mut egui::Ui, events: &mut Vec<crate::render_node::UiEvent>) {
        use crate::components::{
            Button, ButtonColor, ButtonGroupItem, ButtonGroups, ButtonType, Code, ColumnType,
            JsonTree, Select, SelectOption, Size, TableView, Typography, TypographyVariant,
        };
        use crate::dataset::resolve_dataset;
        use crate::render_node::{RenderNode, UiEvent};

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

        // Per-node table/json/raw toggle, remembered across frames.
        let mem_id = ui.make_persistent_id((node_id, "data_view_mode"));
        let mut mode = ViewMode::from_u8(ui.data(|d| d.get_temp(mem_id).unwrap_or(0u8)));

        // Header row: [Table|JSON|Raw] segmented tabs · count · <spacer> ·
        // Copy · Charts — matching the DataViewer design. A fixed row height
        // keeps the tabs, count, and buttons vertically centred.
        ui.horizontal(|ui| {
            ui.set_min_height(28.0);
            let active = match mode {
                ViewMode::Table => "table",
                ViewMode::Json => "json",
                ViewMode::Raw => "raw",
            };
            if let Some(v) = ButtonGroups::builder()
                .id(format!("{node_id}_views"))
                .items(vec![
                    ButtonGroupItem::builder()
                        .value("table")
                        .label("Table")
                        .build(),
                    ButtonGroupItem::builder()
                        .value("json")
                        .label("JSON")
                        .build(),
                    ButtonGroupItem::builder().value("raw").label("Raw").build(),
                ])
                .active(active)
                .build()
                .show(ui)
                .inner
            {
                mode = match v.as_str() {
                    "json" => ViewMode::Json,
                    "raw" => ViewMode::Raw,
                    _ => ViewMode::Table,
                };
            }
            ui.add_space(8.0);
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
            ui.add(
                Typography::builder()
                    .text(count)
                    .variant(TypographyVariant::BodyMuted)
                    .build(),
            );

            // Actions hang off the right edge, so add rightmost-first to read
            // Copy · Export · Charts left-to-right on screen.
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        Button::builder()
                            .label("Charts")
                            .icon(egui_phosphor::regular::CHART_LINE)
                            .button_type(ButtonType::Text)
                            .color(ButtonColor::Secondary)
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
                        .size(Size::Small)
                        .width(92.0)
                        .build()
                        .show(ui)
                        .inner
                        .selected;
                    if let Some(exporter) = selected {
                        events.push(UiEvent {
                            id: crate::actions::EXPORT_DATASET.to_string(),
                            kind: "click".to_string(),
                            value:
                                serde_json::json!({ "handle": self.handle, "exporter": exporter })
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
                        .copy(records_json(&page, true))
                        .build(),
                );
            });
        });
        ui.data_mut(|d| d.insert_temp(mem_id, mode.as_u8()));

        egui::ScrollArea::both()
            .id_salt((node_id, "data_view_scroll"))
            .show(ui, |ui| match mode {
                ViewMode::Json => {
                    JsonTree::builder()
                        .id(format!("{node_id}_tree"))
                        .value(records_value(&page))
                        .build()
                        .show(ui);
                }
                ViewMode::Raw => {
                    ui.add(
                        Code::builder()
                            .value(records_json(&page, true))
                            .language("json")
                            .build(),
                    );
                }
                ViewMode::Table => {
                    let headers: Vec<String> =
                        page.columns.iter().map(|c| c.name.clone()).collect();
                    let column_types: Vec<ColumnType> = page
                        .columns
                        .iter()
                        .map(|c| ColumnType::from_sql(&c.type_hint))
                        .collect();
                    let rows: Vec<Vec<RenderNode>> = page
                        .rows
                        .iter()
                        .map(|row| {
                            row.iter()
                                .map(|cell| {
                                    RenderNode::Text(Typography::builder().text(cell).build())
                                })
                                .collect()
                        })
                        .collect();
                    TableView::builder()
                        .headers(headers)
                        .rows(rows)
                        .column_types(column_types)
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
