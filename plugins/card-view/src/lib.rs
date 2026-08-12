//! Cards — a **data-renderer** plugin (#135). It presents a host-owned dataset
//! as a grid of cards (one per row: the first column is the title, the rest are
//! shown as key/value lines). The host reads its own copy of the rows, passes
//! them here, and draws the `RenderNode` tree this returns as an extra view
//! format in the DataView. The plugin only *describes* UI — it never renders
//! directly or touches the data bus.

#[rustfmt::skip]
mod bindings;

use serde::Deserialize;

use thoth_plugin_sdk::components::{
    Card, Column, Row, Spacer, Split, Typography, TypographyVariant,
};
use thoth_plugin_sdk::render_node::RenderNode;
use thoth_plugin_sdk::PluginMeta;

use bindings::exports::thoth::plugin::{
    data_renderer::{Guest as RendererGuest, PluginError},
    plugin_lifecycle::Guest as LifecycleGuest,
    plugin_settings::{Guest as SettingsGuest, SettingsOutput},
};

/// Cards per grid row.
const COLUMNS: usize = 2;
/// Max cards rendered (matches the host DataView row cap); extra rows are noted.
const MAX_CARDS: usize = 1000;

/// Gap between grid cells — design `.cardgrid{gap:8px}`, the 8px panel gutter.
const GRID_GAP: f32 = 8.0;
/// Inset of the grid from the view edges — design `.cardgrid{padding:12px}`. The
/// host draws a renderer's node flush (`.dvbody{padding:0}`), so the grid owns it.
const GRID_PAD: f32 = 12.0;
/// Space between a card's key/value lines — design `.kvl{line-height:1.75}`, i.e.
/// ~21px lines for 12px mono text.
const LINE_GAP: f32 = 6.0;
/// Space after a key's colon — design `.kvl b` is followed by `": "`.
const KEY_GAP: f32 = 4.0;

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

/// A mono run, optionally muted — the card body is monospaced throughout
/// (design `.kvl{font-family:var(--mono);font-size:12px}`), with the key label
/// carried a step brighter than its value.
fn mono(text: impl Into<String>, muted: bool) -> RenderNode {
    let run = Typography::builder()
        .text(text.into())
        .variant(TypographyVariant::Mono);
    // Design `.kvl` sits at `--subtext0` with the value a further step down, so the
    // brighter run is `fg-subtle` rather than full `fg`.
    RenderNode::Text(if muted {
        run.color("muted").build()
    } else {
        run.color("fg-subtle").build()
    })
}

/// One row → a card: first column is the title, the rest are `name: value` lines.
fn card_node(columns: &[String], row: &[String]) -> RenderNode {
    let title = row.first().cloned().unwrap_or_default();
    // `.kvl b` (the key) reads brighter than the value it labels, so each line is
    // a two-tone pair rather than one muted string.
    let lines: Vec<RenderNode> = columns
        .iter()
        .enumerate()
        .skip(1)
        .map(|(i, col)| {
            RenderNode::Row(
                Row::builder()
                    .gap(KEY_GAP)
                    .children(vec![
                        mono(format!("{col}:"), false),
                        mono(row.get(i).cloned().unwrap_or_default(), true),
                    ])
                    .build(),
            )
        })
        .collect();
    let body = RenderNode::Column(Column::builder().gap(LINE_GAP).children(lines).build());
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
            // Padded like the grid, so the hint isn't flush against the edge.
            let empty = RenderNode::Row(
                Row::builder()
                    .padding(GRID_PAD)
                    .children(vec![muted("No records to show.")])
                    .build(),
            );
            return serde_json::to_string(&empty).map_err(|e| PluginError {
                code: 2,
                message: e.to_string(),
            });
        }

        // Cap the number of cards drawn; note any overflow.
        let total = recs.rows.len();
        let shown = total.min(MAX_CARDS);
        let cards: Vec<RenderNode> = recs.rows[..shown]
            .iter()
            .map(|row| card_node(&recs.columns, row))
            .collect();

        // Lay the cards out COLUMNS-per-row via equal-width splits. The splits are
        // grid cells, not resizable regions, so they stay unframed (flush) — the
        // cards themselves carry the panel fill, edge and corners. A short last
        // chunk is padded with empty cells so its card keeps the grid's column
        // width instead of stretching across the row.
        let mut grid: Vec<RenderNode> = cards
            .chunks(COLUMNS)
            .map(|chunk| {
                let mut cells = chunk.to_vec();
                while cells.len() < COLUMNS {
                    cells.push(RenderNode::Spacer(Spacer::builder().size(0.0).build()));
                }
                RenderNode::Split(
                    Split::builder()
                        .gap(GRID_GAP)
                        .widths(vec![1.0; cells.len()])
                        .children(cells)
                        .build(),
                )
            })
            .collect();

        if total > shown {
            // Design: the overflow note is an italic caption under the grid.
            grid.push(RenderNode::Text(
                Typography::builder()
                    .text(format!("… {} more row(s) not shown.", total - shown))
                    .variant(TypographyVariant::Caption)
                    .italic(true)
                    .build(),
            ));
        }

        // The grid supplies its own inset from the DataView body, which is flush.
        let root = RenderNode::Row(
            Row::builder()
                .padding(GRID_PAD)
                .max_width(true)
                .children(vec![RenderNode::Column(
                    Column::builder().gap(GRID_GAP).children(grid).build(),
                )])
                .build(),
        );
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

#[cfg(test)]
mod tests {
    use super::{CardViewPlugin, RendererGuest};

    #[test]
    fn renders_a_card_per_row() {
        let recs = serde_json::json!({
            "columns": ["name", "dept"],
            "rows": [["Alice", "Eng"], ["Bob", "Sales"]],
        })
        .to_string();
        let out = CardViewPlugin::render(recs).unwrap();
        assert!(out.contains("Alice"));
        // Key and value are separate mono runs (two-tone `key: value` line).
        assert!(out.contains("dept:"));
        assert!(out.contains("Eng"));
        assert!(out.contains("Bob"));
    }

    #[test]
    fn caps_cards_and_notes_overflow() {
        let rows: Vec<Vec<String>> = (0..(super::MAX_CARDS + 5))
            .map(|i| vec![i.to_string()])
            .collect();
        let recs = serde_json::json!({ "columns": ["id"], "rows": rows }).to_string();
        let out = CardViewPlugin::render(recs).unwrap();
        assert!(out.contains("more row(s) not shown"));
    }

    #[test]
    fn empty_dataset_is_handled() {
        let recs = serde_json::json!({ "columns": ["id"], "rows": [] }).to_string();
        let out = CardViewPlugin::render(recs).unwrap();
        assert!(out.contains("No records"));
    }
}
