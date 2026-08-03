//! Cards — a **data-renderer** plugin (#135). It presents a host-owned dataset
//! as a grid of cards (one per row: the first column is the title, the rest are
//! shown as key/value lines). The host reads its own copy of the rows, passes
//! them here, and draws the `RenderNode` tree this returns as an extra view
//! format in the DataView. The plugin only *describes* UI — it never renders
//! directly or touches the data bus.

#[rustfmt::skip]
mod bindings;

use serde::Deserialize;

use thoth_plugin_sdk::components::{Card, Column, Split, Typography, TypographyVariant};
use thoth_plugin_sdk::render_node::RenderNode;
use thoth_plugin_sdk::PluginMeta;

use bindings::exports::thoth::plugin::{
    data_renderer::{Guest as RendererGuest, PluginError},
    plugin_lifecycle::Guest as LifecycleGuest,
    plugin_settings::{Guest as SettingsGuest, SettingsOutput},
};

/// Cards per grid row.
const COLUMNS: usize = 2;

#[derive(PluginMeta)]
#[plugin(
    id = "com.thoth.card-view",
    name = "Cards",
    version = "0.1.0",
    description = "Present a dataset as a grid of cards",
    capabilities = [Renderer],
    author = "Thoth contributors",
)]
struct CardViewPlugin;

/// The host-owned dataset shape passed to `render` (see the `data-renderer` WIT
/// doc): column order preserved, every cell a string.
#[derive(Deserialize)]
struct Records {
    #[serde(default)]
    columns: Vec<String>,
    #[serde(default)]
    rows: Vec<Vec<String>>,
}

fn muted(text: impl Into<String>) -> RenderNode {
    RenderNode::Text(
        Typography::builder()
            .text(text.into())
            .variant(TypographyVariant::BodyMuted)
            .build(),
    )
}

/// One row → a card: first column is the title, the rest are `name: value` lines.
fn card_node(columns: &[String], row: &[String]) -> RenderNode {
    let title = row.first().cloned().unwrap_or_default();
    let lines: Vec<RenderNode> = columns
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, col)| {
            muted(format!(
                "{col}: {}",
                row.get(i).cloned().unwrap_or_default()
            ))
        })
        .collect();
    let body = RenderNode::Column(Column::builder().gap(2.0).children(lines).build());
    RenderNode::Card(Box::new(Card::builder().title(title).body(body).build()))
}

impl RendererGuest for CardViewPlugin {
    fn name() -> String {
        "Cards".to_string()
    }

    fn render(records_json: String) -> Result<String, PluginError> {
        let recs: Records = serde_json::from_str(&records_json).map_err(|e| PluginError {
            code: 1,
            message: format!("invalid records-json: {e}"),
        })?;

        if recs.rows.is_empty() {
            let empty = muted("No records to show.");
            return serde_json::to_string(&empty).map_err(|e| PluginError {
                code: 2,
                message: e.to_string(),
            });
        }

        let cards: Vec<RenderNode> = recs
            .rows
            .iter()
            .map(|row| card_node(&recs.columns, row))
            .collect();

        // Lay the cards out COLUMNS-per-row via equal-width splits.
        let grid: Vec<RenderNode> = cards
            .chunks(COLUMNS)
            .map(|chunk| {
                if chunk.len() == 1 {
                    chunk[0].clone()
                } else {
                    RenderNode::Split(
                        Split::builder()
                            .gap(8.0)
                            .widths(vec![1.0; chunk.len()])
                            .children(chunk.to_vec())
                            .build(),
                    )
                }
            })
            .collect();

        let root = RenderNode::Column(Column::builder().gap(8.0).children(grid).build());
        serde_json::to_string(&root).map_err(|e| PluginError {
            code: 2,
            message: e.to_string(),
        })
    }
}

impl LifecycleGuest for CardViewPlugin {
    fn on_load(_settings: String) {}
    fn on_close() {}
    fn on_setting_change(_settings: String) {}
}

impl SettingsGuest for CardViewPlugin {
    fn render_settings() -> Result<SettingsOutput, PluginError> {
        Ok(SettingsOutput {
            node_json: serde_json::to_string(&muted("Cards has no settings.")).unwrap_or_default(),
            height_hint: 0,
        })
    }
}

bindings::export!(CardViewPlugin with_types_in bindings);
