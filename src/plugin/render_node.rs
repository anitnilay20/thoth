use eframe::egui;
use serde::{Deserialize, Serialize};

use crate::components::table_view::{TableCell, TableView, TableViewProps};
use crate::components::traits::StatelessComponent;

// pub enum NodeType {
//     Text,
//     Table,
//     JSONTree,
//     Button,
//     Label,
//     Frame,
// }

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum RenderNode {
    Text {
        value: String,
    },
    Bold {
        child: Box<RenderNode>,
    },
    Italic {
        child: Box<RenderNode>,
    },
    Colored {
        color: String,
        child: Box<RenderNode>,
    },
    Badge {
        label: String,
        color: String,
    },
    Link {
        label: String,
        url: String,
    },
    Row {
        children: Vec<RenderNode>,
    },
    Column {
        children: Vec<RenderNode>,
    },
    KeyValue {
        key: String,
        value: Box<RenderNode>,
    },
    Collapsible {
        label: String,
        children: Vec<RenderNode>,
    },
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<RenderNode>>,
    },
    JSONTree {
        value: serde_json::Value,
    },
}

pub fn render_node(ui: &mut egui::Ui, node: &RenderNode) {
    match node {
        RenderNode::Text { value } => {
            ui.label(value);
        }
        RenderNode::Bold { child } => {
            ui.label(egui::RichText::new(collect_text(child)).strong());
        }
        RenderNode::Italic { child } => {
            ui.label(egui::RichText::new(collect_text(child)).italics());
        }
        RenderNode::Colored { color, child } => {
            if let Some(c) = parse_hex_color(color) {
                ui.colored_label(c, collect_text(child));
            } else {
                render_node(ui, child);
            }
        }
        RenderNode::Badge { label, color } => {
            let c = parse_hex_color(color).unwrap_or(egui::Color32::GRAY);
            egui::Frame::new()
                .fill(c)
                .corner_radius(3.0)
                .inner_margin(egui::Margin::symmetric(4, 2))
                .show(ui, |ui| {
                    ui.label(label);
                });
        }
        RenderNode::Link { label, url } => {
            ui.hyperlink_to(label, url);
        }
        RenderNode::Row { children } => {
            ui.horizontal(|ui| {
                for child in children {
                    render_node(ui, child);
                }
            });
        }
        RenderNode::Column { children } => {
            ui.vertical(|ui| {
                for child in children {
                    render_node(ui, child);
                }
            });
        }
        RenderNode::KeyValue { key, value } => {
            ui.horizontal(|ui| {
                ui.strong(key);
                ui.label(":");
                render_node(ui, value);
            });
        }
        RenderNode::Collapsible { label, children } => {
            egui::CollapsingHeader::new(label).show(ui, |ui| {
                for child in children {
                    render_node(ui, child);
                }
            });
        }
        RenderNode::Table { headers, rows } => {
            let row_count = rows.len();
            TableView::render(
                ui,
                TableViewProps {
                    headers,
                    row_count,
                    min_col_width: None,
                    build_row: Box::new(move |i| {
                        rows.get(i)
                            .map(|row| {
                                row.iter()
                                    .map(|cell| TableCell::custom(move |ui| render_node(ui, cell)))
                                    .collect()
                            })
                            .unwrap_or_default()
                    }),
                },
            );
        }
        RenderNode::JSONTree { value: _ } => {
            // TODO: render inline JSON tree viewer
        }
    }
}

fn collect_text(node: &RenderNode) -> String {
    match node {
        RenderNode::Text { value } => value.clone(),
        RenderNode::Bold { child }
        | RenderNode::Italic { child }
        | RenderNode::Colored { child, .. } => collect_text(child),
        RenderNode::Badge { label, .. } => label.clone(),
        RenderNode::Link { label, .. } => label.clone(),
        RenderNode::KeyValue { key, value } => format!("{}: {}", key, collect_text(value)),
        RenderNode::Row { children }
        | RenderNode::Column { children }
        | RenderNode::Collapsible { children, .. } => children
            .iter()
            .map(collect_text)
            .collect::<Vec<_>>()
            .join(" "),
        RenderNode::Table { headers, rows } => {
            let mut text = headers.join(" ") + "\n";
            for row in rows {
                text += &row.iter().map(collect_text).collect::<Vec<_>>().join(" ");
                text += "\n";
            }
            text
        }
        RenderNode::JSONTree { value } => value.to_string(),
    }
}

fn parse_hex_color(s: &str) -> Option<egui::Color32> {
    let s = s.strip_prefix('#')?;
    if s.len() == 6 {
        let r = u8::from_str_radix(&s[0..2], 16).ok()?;
        let g = u8::from_str_radix(&s[2..4], 16).ok()?;
        let b = u8::from_str_radix(&s[4..6], 16).ok()?;
        Some(egui::Color32::from_rgb(r, g, b))
    } else {
        None
    }
}
