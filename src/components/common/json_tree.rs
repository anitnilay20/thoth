use eframe::egui::{self, Color32};
use serde_json::Value;

use crate::components::common::traits::StatelessComponent;
use crate::components::icon_button::{IconButton, IconButtonProps};
use crate::theme::{ROW_HEIGHT, Theme, ThemeColors};

// =============================================================================
// Public types
// =============================================================================

pub struct JsonTree;

pub struct JsonTreeProps<'a> {
    pub value: &'a Value,
    pub id: egui::Id,
}

pub struct JsonTreeOutput;

// =============================================================================
// Flat row list — rebuilt each frame from expansion state
// =============================================================================

#[derive(Clone)]
enum RowKind {
    /// `{` / `[` header that can be toggled.
    Expandable { label: String, is_expanded: bool },
    /// `key: value` or array-index leaf.
    Leaf { label: String, color: Color32 },
    /// Closing `}` / `]`.
    Closing { bracket: &'static str },
}

#[derive(Clone)]
struct FlatRow {
    path: String,
    indent: usize,
    kind: RowKind,
}

// =============================================================================
// Expansion state in egui memory
// =============================================================================

#[derive(Clone, Default)]
struct ExpandedPaths(std::collections::HashSet<String>);

// =============================================================================
// Component impl
// =============================================================================

impl StatelessComponent for JsonTree {
    type Props<'a> = JsonTreeProps<'a>;
    type Output = JsonTreeOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| Theme::default().colors())
        });

        let mem_id = props.id.with("json_tree_expanded");
        let init_id = props.id.with("json_tree_init");

        let initialized: bool = ui.ctx().data(|d| d.get_temp(init_id).unwrap_or(false));

        let mut expanded: ExpandedPaths = if !initialized {
            let mut paths = std::collections::HashSet::new();
            collect_all_paths(props.value, "", &mut paths);
            ExpandedPaths(paths)
        } else {
            ui.ctx().data(|d| d.get_temp(mem_id).unwrap_or_default())
        };

        // Flatten tree into rows based on current expansion state.
        let mut rows: Vec<FlatRow> = Vec::new();
        flatten_value(props.value, "", 0, &expanded, &colors, &mut rows);

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
                        RowKind::Leaf { label, color } => {
                            leaf_row(ui, label, row.indent, *color);
                        }
                        RowKind::Closing { bracket } => {
                            closing_row(ui, bracket, row.indent, &colors);
                        }
                    }
                }
            });

        if let Some(path) = toggle_path {
            if !expanded.0.remove(&path) {
                expanded.0.insert(path);
            }
        }

        ui.ctx().data_mut(|d| {
            d.insert_temp(mem_id, expanded);
            d.insert_temp(init_id, true);
        });

        JsonTreeOutput
    }
}

// =============================================================================
// Tree flattener — produces the ordered list of visible rows
// =============================================================================

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
                    flatten_keyed(
                        &i.to_string(),
                        val,
                        &child,
                        indent + 1,
                        expanded,
                        colors,
                        out,
                    );
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

/// Renders a key + child value pair (object field or array element).
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

// =============================================================================
// Collect all expandable paths (for default-expand-all on first render)
// =============================================================================

fn collect_all_paths(value: &Value, path: &str, out: &mut std::collections::HashSet<String>) {
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

// =============================================================================
// Row renderers
// =============================================================================

const INDENT_PX: f32 = 16.0;
const FONT_SIZE: f32 = 12.5;

/// Expandable header row with caret. Returns true if the caret was clicked.
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
        if IconButton::render(
            ui,
            IconButtonProps {
                icon,
                frame: false,
                tooltip: None,
                badge_color: None,
                size: None,
                disabled: false,
                icon_size: None,
                selected: false,
            },
        )
        .clicked
        {
            clicked = true;
        }
        // Split "key: {" so the key is colored differently from the bracket.
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

/// Leaf row (key: value or bare value).
fn leaf_row(ui: &mut egui::Ui, label: &str, indent: usize, color: Color32) {
    ui.horizontal(|ui| {
        // Extra INDENT_PX to align with text after the (absent) caret icon.
        ui.add_space(indent as f32 * INDENT_PX + INDENT_PX);
        ui.add(egui::Label::new(
            egui::RichText::new(label)
                .size(FONT_SIZE)
                .color(color)
                .monospace(),
        ));
    });
}

/// Closing `}` / `]` row.
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

// =============================================================================
// Value display helpers
// =============================================================================

fn value_display(val: &Value, colors: &ThemeColors) -> (String, Color32) {
    match val {
        Value::String(s) => (format!("\"{s}\""), colors.syntax_string),
        Value::Number(n) => (n.to_string(), colors.syntax_number),
        Value::Bool(b) => (b.to_string(), colors.accent),
        Value::Null => ("null".to_string(), colors.fg_muted),
        _ => (val.to_string(), colors.fg),
    }
}
