use std::collections::HashSet;

use egui::Color32;
use serde_json::Value;

use crate::components::IconButton;
use crate::theme::{ROW_HEIGHT, ThemeColors};

use super::JsonTree;

const INDENT_PX: f32 = 16.0;
const FONT_SIZE: f32 = 12.5;

#[derive(Clone)]
enum RowKind {
    Expandable { label: String, is_expanded: bool },
    Leaf { label: String, color: Color32 },
    Closing { bracket: &'static str },
}

#[derive(Clone)]
struct FlatRow {
    path: String,
    indent: usize,
    kind: RowKind,
}

#[derive(Clone, Default)]
struct ExpandedPaths(HashSet<String>);

impl JsonTree {
    /// Render the tree into the available area.
    pub fn show(&self, ui: &mut egui::Ui) {
        let colors = ThemeColors::from_ctx(ui.ctx());
        let base_id = egui::Id::new(&self.id);
        let mem_id = base_id.with("json_tree_expanded");
        let init_id = base_id.with("json_tree_init");

        let initialized: bool = ui.ctx().data(|d| d.get_temp(init_id).unwrap_or(false));
        let mut expanded: ExpandedPaths = if !initialized {
            let mut paths = HashSet::new();
            collect_all_paths(&self.value, "", &mut paths);
            ExpandedPaths(paths)
        } else {
            ui.ctx().data(|d| d.get_temp(mem_id).unwrap_or_default())
        };

        let mut rows: Vec<FlatRow> = Vec::new();
        flatten_value(&self.value, "", 0, &expanded, &colors, &mut rows);
        let row_count = rows.len();
        let mut toggle_path: Option<String> = None;

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show_rows(ui, ROW_HEIGHT, row_count, |ui, range| {
                for idx in range {
                    let row = &rows[idx];
                    match &row.kind {
                        RowKind::Expandable { label, is_expanded } => {
                            if expandable_row(ui, label, row.indent, *is_expanded, &colors) {
                                toggle_path = Some(row.path.clone());
                            }
                        }
                        RowKind::Leaf { label, color } => leaf_row(ui, label, row.indent, *color),
                        RowKind::Closing { bracket } => {
                            closing_row(ui, bracket, row.indent, &colors)
                        }
                    }
                }
            });

        if let Some(path) = toggle_path
            && !expanded.0.remove(&path)
        {
            expanded.0.insert(path);
        }

        ui.ctx().data_mut(|d| {
            d.insert_temp(mem_id, expanded);
            d.insert_temp(init_id, true);
        });

    }
}

fn flatten_value(
    value: &Value,
    path: &str,
    indent: usize,
    expanded: &ExpandedPaths,
    colors: &ThemeColors,
    out: &mut Vec<FlatRow>,
) {
    match value {
        Value::Object(map) => {
            let is_expanded = expanded.0.contains(path);
            out.push(FlatRow {
                path: path.to_string(),
                indent,
                kind: RowKind::Expandable {
                    label: if is_expanded {
                        "{".to_string()
                    } else {
                        format!("{{…}} ({} keys)", map.len())
                    },
                    is_expanded,
                },
            });
            if is_expanded {
                for (key, val) in map {
                    let child = format!("{path}/{key}");
                    flatten_keyed(key, val, &child, indent + 1, expanded, colors, out);
                }
                out.push(FlatRow {
                    path: format!("{path}/_close"),
                    indent,
                    kind: RowKind::Closing { bracket: "}" },
                });
            }
        }
        Value::Array(arr) => {
            let is_expanded = expanded.0.contains(path);
            out.push(FlatRow {
                path: path.to_string(),
                indent,
                kind: RowKind::Expandable {
                    label: if is_expanded {
                        "[".to_string()
                    } else {
                        format!("[…] ({} items)", arr.len())
                    },
                    is_expanded,
                },
            });
            if is_expanded {
                for (i, val) in arr.iter().enumerate() {
                    let child = format!("{path}/{i}");
                    flatten_keyed(&i.to_string(), val, &child, indent + 1, expanded, colors, out);
                }
                out.push(FlatRow {
                    path: format!("{path}/_close"),
                    indent,
                    kind: RowKind::Closing { bracket: "]" },
                });
            }
        }
        _ => {
            let (text, color) = value_display(value, colors);
            out.push(FlatRow {
                path: path.to_string(),
                indent,
                kind: RowKind::Leaf { label: text, color },
            });
        }
    }
}

fn flatten_keyed(
    key: &str,
    val: &Value,
    path: &str,
    indent: usize,
    expanded: &ExpandedPaths,
    colors: &ThemeColors,
    out: &mut Vec<FlatRow>,
) {
    match val {
        Value::Object(map) => {
            let is_expanded = expanded.0.contains(path);
            let suffix = if is_expanded {
                "{".to_string()
            } else {
                format!("{{…}} ({} keys)", map.len())
            };
            out.push(FlatRow {
                path: path.to_string(),
                indent,
                kind: RowKind::Expandable {
                    label: format!("{key}: {suffix}"),
                    is_expanded,
                },
            });
            if is_expanded {
                for (k, v) in map {
                    let child = format!("{path}/{k}");
                    flatten_keyed(k, v, &child, indent + 1, expanded, colors, out);
                }
                out.push(FlatRow {
                    path: format!("{path}/_close"),
                    indent,
                    kind: RowKind::Closing { bracket: "}" },
                });
            }
        }
        Value::Array(arr) => {
            let is_expanded = expanded.0.contains(path);
            let suffix = if is_expanded {
                "[".to_string()
            } else {
                format!("[…] ({} items)", arr.len())
            };
            out.push(FlatRow {
                path: path.to_string(),
                indent,
                kind: RowKind::Expandable {
                    label: format!("{key}: {suffix}"),
                    is_expanded,
                },
            });
            if is_expanded {
                for (i, v) in arr.iter().enumerate() {
                    let child = format!("{path}/{i}");
                    flatten_keyed(&i.to_string(), v, &child, indent + 1, expanded, colors, out);
                }
                out.push(FlatRow {
                    path: format!("{path}/_close"),
                    indent,
                    kind: RowKind::Closing { bracket: "]" },
                });
            }
        }
        _ => {
            let (value_text, color) = value_display(val, colors);
            out.push(FlatRow {
                path: path.to_string(),
                indent,
                kind: RowKind::Leaf {
                    label: format!("{key}: {value_text}"),
                    color,
                },
            });
        }
    }
}

fn collect_all_paths(value: &Value, path: &str, out: &mut HashSet<String>) {
    match value {
        Value::Object(map) => {
            out.insert(path.to_string());
            for (key, val) in map {
                collect_all_paths(val, &format!("{path}/{key}"), out);
            }
        }
        Value::Array(arr) => {
            out.insert(path.to_string());
            for (i, val) in arr.iter().enumerate() {
                collect_all_paths(val, &format!("{path}/{i}"), out);
            }
        }
        _ => {}
    }
}

fn expandable_row(
    ui: &mut egui::Ui,
    label: &str,
    indent: usize,
    is_expanded: bool,
    colors: &ThemeColors,
) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.add_space(indent as f32 * INDENT_PX);
        let icon = if is_expanded {
            egui_phosphor::regular::CARET_DOWN
        } else {
            egui_phosphor::regular::CARET_RIGHT
        };
        if ui.add(IconButton::builder().icon(icon).build()).clicked() {
            clicked = true;
        }
        if let Some(colon) = label.find(": ") {
            let key = &label[..colon];
            let rest = &label[colon..];
            ui.add(egui::Label::new(
                egui::RichText::new(key)
                    .size(FONT_SIZE)
                    .color(colors.syntax_key)
                    .monospace(),
            ));
            ui.add(egui::Label::new(
                egui::RichText::new(rest)
                    .size(FONT_SIZE)
                    .color(colors.fg_muted)
                    .monospace(),
            ));
        } else {
            ui.add(egui::Label::new(
                egui::RichText::new(label)
                    .size(FONT_SIZE)
                    .color(colors.fg_muted)
                    .monospace(),
            ));
        }
    });
    clicked
}

fn leaf_row(ui: &mut egui::Ui, label: &str, indent: usize, color: Color32) {
    ui.horizontal(|ui| {
        ui.add_space(indent as f32 * INDENT_PX + INDENT_PX);
        ui.add(egui::Label::new(
            egui::RichText::new(label)
                .size(FONT_SIZE)
                .color(color)
                .monospace(),
        ));
    });
}

fn closing_row(ui: &mut egui::Ui, bracket: &str, indent: usize, colors: &ThemeColors) {
    ui.horizontal(|ui| {
        ui.add_space(indent as f32 * INDENT_PX + INDENT_PX);
        ui.add(egui::Label::new(
            egui::RichText::new(bracket)
                .size(FONT_SIZE)
                .color(colors.fg_muted)
                .monospace(),
        ));
    });
}

fn value_display(val: &Value, colors: &ThemeColors) -> (String, Color32) {
    match val {
        Value::String(s) => (format!("\"{s}\""), colors.syntax_string),
        Value::Number(n) => (n.to_string(), colors.syntax_number),
        Value::Bool(b) => (b.to_string(), colors.accent),
        Value::Null => ("null".to_string(), colors.fg_muted),
        _ => (val.to_string(), colors.fg),
    }
}
