//! Sidebar panel for the host **datasets** registry (#113): lists published
//! datasets and previews the selected one's first rows in a `TableView`.
//!
//! Read-only browse/preview surface — it reads the registry in-process (no WASM
//! crossing). The full paged `read` path is what consumer plugins use (#114).

use eframe::egui;
use thoth_plugin_sdk::components::{ColumnType, TableView};
use thoth_plugin_sdk::render_node::RenderNode;

/// Rows fetched for the preview grid.
const PREVIEW_ROWS: u32 = 100;

#[derive(Default)]
pub struct DatasetsPanel {
    /// Currently-previewed dataset id.
    selected: Option<String>,
}

impl DatasetsPanel {
    pub fn render(&mut self, ui: &mut egui::Ui) {
        let datasets = crate::plugin::datasets::list();

        // Forget a selection whose dataset was released/evicted.
        if let Some(sel) = &self.selected
            && !datasets.iter().any(|m| &m.id == sel)
        {
            self.selected = None;
        }

        ui.add_space(4.0);
        ui.label(egui::RichText::new("DATASETS").small().strong());
        ui.separator();

        if datasets.is_empty() {
            ui.label(
                egui::RichText::new("No datasets published yet.")
                    .weak()
                    .italics(),
            );
            return;
        }

        // Dataset list — name, row count, source plugin (short).
        for meta in &datasets {
            let short = meta
                .source_plugin
                .rsplit('.')
                .next()
                .unwrap_or(meta.source_plugin.as_str());
            let selected = self.selected.as_deref() == Some(meta.id.as_str());
            let label = format!("{}  ·  {} rows  ·  {short}", meta.name, meta.row_count);
            if ui.selectable_label(selected, label).clicked() {
                self.selected = Some(meta.id.clone());
            }
        }

        ui.separator();

        // Preview of the selected dataset.
        let Some(id) = self.selected.clone() else {
            ui.label(egui::RichText::new("Select a dataset to preview.").weak());
            return;
        };
        let Some(page) = crate::plugin::datasets::read(&id, 0, PREVIEW_ROWS) else {
            return;
        };

        let headers: Vec<String> = page
            .columns
            .iter()
            .map(|c| {
                if c.type_hint.is_empty() {
                    c.name.clone()
                } else {
                    format!("{}  ·  {}", c.name, c.type_hint)
                }
            })
            .collect();
        let column_types: Vec<ColumnType> = page
            .columns
            .iter()
            .map(|c| ColumnType::from_sql(&c.type_hint))
            .collect();
        let rows: Vec<Vec<RenderNode>> = page
            .rows
            .iter()
            .map(|r| r.iter().map(RenderNode::text).collect())
            .collect();

        let mut table = TableView::builder()
            .headers(headers)
            .rows(rows)
            .column_types(column_types)
            .build();
        let mut events = Vec::new();
        table.show(ui, &mut events);

        if page.total > page.rows.len() as u64 {
            ui.add_space(2.0);
            ui.label(
                egui::RichText::new(format!(
                    "Showing {} of {} rows",
                    page.rows.len(),
                    page.total
                ))
                .weak(),
            );
        }
    }
}
