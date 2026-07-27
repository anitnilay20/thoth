//! A **data-bound** render node: the plugin publishes a dataset to the host
//! (via `dataset-bus.publish`) and embeds the returned `handle` here. The host
//! draws the data itself — a table or a JSON tree, user-switchable — reading
//! the rows from its single-owned registry through the installed resolver
//! ([`crate::dataset`]). The plugin delegates *display* and never holds the
//! rows in its own UI.

use bon::Builder;
use serde::{Deserialize, Serialize};

/// Renders a host-owned dataset (referenced by `handle`) as a table / JSON tree.
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct DataView {
    /// Stable id for this node (persists the table⇄json toggle across frames).
    #[builder(default)]
    pub id: String,
    /// Registry handle returned by `dataset-bus.publish`.
    pub handle: String,
}

#[cfg(feature = "egui")]
impl DataView {
    /// Max rows the host draws (the registry also caps a single read).
    const LIMIT: u32 = 1000;

    /// Draw the referenced dataset (table or JSON tree) from the host registry.
    pub fn show(&self, ui: &mut egui::Ui) {
        use crate::components::{
            ColumnType, IconButton, JsonTree, TableView, Typography, TypographyVariant,
        };
        use crate::dataset::resolve_dataset;
        use crate::render_node::RenderNode;

        let Some(page) = resolve_dataset(&self.handle, Self::LIMIT) else {
            ui.add(
                Typography::builder()
                    .text("This dataset is no longer available.")
                    .variant(TypographyVariant::BodyMuted)
                    .build(),
            );
            return;
        };

        // Per-node table/json-tree toggle, remembered across frames.
        let mem_id = ui.make_persistent_id((self.id.as_str(), "data_view_json"));
        let mut json: bool = ui.data(|d| d.get_temp(mem_id).unwrap_or(false));

        ui.horizontal(|ui| {
            ui.add(
                Typography::builder()
                    .text(format!("{} rows", page.total))
                    .variant(TypographyVariant::BodyMuted)
                    .build(),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui
                    .add(
                        IconButton::builder()
                            .icon(egui_phosphor::regular::TREE_STRUCTURE)
                            .frame(true)
                            .selected(json)
                            .tooltip("JSON tree")
                            .build(),
                    )
                    .clicked()
                {
                    json = true;
                }
                if ui
                    .add(
                        IconButton::builder()
                            .icon(egui_phosphor::regular::TABLE)
                            .frame(true)
                            .selected(!json)
                            .tooltip("Table")
                            .build(),
                    )
                    .clicked()
                {
                    json = false;
                }
            });
        });
        ui.data_mut(|d| d.insert_temp(mem_id, json));

        egui::ScrollArea::both()
            .id_salt((self.id.as_str(), "data_view_scroll"))
            .show(ui, |ui| {
                if json {
                    let records: Vec<serde_json::Value> = page
                        .rows
                        .iter()
                        .map(|row| {
                            let mut obj = serde_json::Map::new();
                            for (c, col) in page.columns.iter().enumerate() {
                                obj.insert(
                                    col.name.clone(),
                                    serde_json::Value::String(
                                        row.get(c).cloned().unwrap_or_default(),
                                    ),
                                );
                            }
                            serde_json::Value::Object(obj)
                        })
                        .collect();
                    JsonTree::builder()
                        .id(format!("{}_tree", self.id))
                        .value(serde_json::Value::Array(records))
                        .build()
                        .show(ui);
                } else {
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
                    let mut events = Vec::new();
                    TableView::builder()
                        .headers(headers)
                        .rows(rows)
                        .column_types(column_types)
                        .build()
                        .show(ui, &mut events);
                }
            });
    }
}
