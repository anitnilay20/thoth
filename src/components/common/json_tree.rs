use eframe::egui::{self, Color32};
use serde_json::Value;

use crate::components::common::traits::StatelessComponent;
use crate::components::icon_button::{IconButton, IconButtonProps};
use crate::theme::{Theme, ThemeColors};

// =============================================================================
// Public types
// =============================================================================

pub struct JsonTree;

pub struct JsonTreeProps<'a> {
    /// The JSON value to render.
    pub value: &'a Value,
    /// Stable ID used to namespace expansion state in egui memory.
    /// Multiple `JsonTree` instances on the same frame must have different ids.
    pub id: egui::Id,
}

/// Nothing to report upward for now.
pub struct JsonTreeOutput;

// =============================================================================
// Expansion state stored in egui memory
// =============================================================================

/// Flat set of "path strings" that are currently expanded.
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
            // First render — expand every expandable path by default.
            let mut paths = std::collections::HashSet::new();
            collect_all_paths(props.value, "", &mut paths);
            ExpandedPaths(paths)
        } else {
            ui.ctx().data(|d| d.get_temp(mem_id).unwrap_or_default())
        };

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                render_value(ui, props.value, "", 0, &mut expanded, &colors);
            });

        ui.ctx().data_mut(|d| {
            d.insert_temp(mem_id, expanded);
            d.insert_temp(init_id, true);
        });

        JsonTreeOutput
    }
}

// =============================================================================
// Collect all expandable paths (for default-expand-all)
// =============================================================================

fn collect_all_paths(value: &Value, path: &str, out: &mut std::collections::HashSet<String>) {
    match value {
        Value::Object(map) => {
            out.insert(path.to_string());
            for (key, val) in map {
                let child = format!("{path}/{key}");
                collect_all_paths(val, &child, out);
            }
        }
        Value::Array(arr) => {
            out.insert(path.to_string());
            for (i, val) in arr.iter().enumerate() {
                let child = format!("{path}/{i}");
                collect_all_paths(val, &child, out);
            }
        }
        _ => {}
    }
}

// =============================================================================
// Recursive renderer
// =============================================================================

fn render_value(
    ui: &mut egui::Ui,
    value: &Value,
    path: &str,
    indent: usize,
    expanded: &mut ExpandedPaths,
    colors: &ThemeColors,
) {
    match value {
        Value::Object(map) => {
            let is_expanded = expanded.0.contains(path);
            let label = if is_expanded { "{" } else { "{…}" };
            if collapsible_row(ui, label, indent, is_expanded, colors) {
                if is_expanded {
                    expanded.0.remove(path);
                } else {
                    expanded.0.insert(path.to_string());
                }
            }
            if is_expanded {
                for (key, val) in map {
                    let child_path = format!("{path}/{key}");
                    match val {
                        Value::Object(_) | Value::Array(_) => {
                            let key_is_expanded = expanded.0.contains(&child_path);
                            let suffix = match val {
                                Value::Object(_) => {
                                    if key_is_expanded {
                                        "{"
                                    } else {
                                        "{…}"
                                    }
                                }
                                Value::Array(_) => {
                                    if key_is_expanded {
                                        "["
                                    } else {
                                        "[…]"
                                    }
                                }
                                _ => unreachable!(),
                            };
                            let key_label = format!("{key}: {suffix}");
                            if collapsible_key_row(
                                ui,
                                &key_label,
                                indent + 1,
                                key_is_expanded,
                                colors,
                            ) {
                                if key_is_expanded {
                                    expanded.0.remove(&child_path);
                                } else {
                                    expanded.0.insert(child_path.clone());
                                }
                            }
                            if key_is_expanded {
                                render_children(ui, val, &child_path, indent + 1, expanded, colors);
                                closing_bracket(ui, val, indent + 1, colors);
                            }
                        }
                        _ => kv_row(ui, key, val, indent + 1, colors),
                    }
                }
                closing_bracket(ui, value, indent, colors);
            }
        }
        Value::Array(arr) => {
            let is_expanded = expanded.0.contains(path);
            let label = if is_expanded { "[" } else { "[…]" };
            if collapsible_row(ui, label, indent, is_expanded, colors) {
                if is_expanded {
                    expanded.0.remove(path);
                } else {
                    expanded.0.insert(path.to_string());
                }
            }
            if is_expanded {
                for (i, val) in arr.iter().enumerate() {
                    let child_path = format!("{path}/{i}");
                    match val {
                        Value::Object(_) | Value::Array(_) => {
                            let key_is_expanded = expanded.0.contains(&child_path);
                            let suffix = match val {
                                Value::Object(_) => {
                                    if key_is_expanded {
                                        "{"
                                    } else {
                                        "{…}"
                                    }
                                }
                                Value::Array(_) => {
                                    if key_is_expanded {
                                        "["
                                    } else {
                                        "[…]"
                                    }
                                }
                                _ => unreachable!(),
                            };
                            let key_label = format!("{i}: {suffix}");
                            if collapsible_key_row(
                                ui,
                                &key_label,
                                indent + 1,
                                key_is_expanded,
                                colors,
                            ) {
                                if key_is_expanded {
                                    expanded.0.remove(&child_path);
                                } else {
                                    expanded.0.insert(child_path.clone());
                                }
                            }
                            if key_is_expanded {
                                render_children(ui, val, &child_path, indent + 1, expanded, colors);
                                closing_bracket(ui, val, indent + 1, colors);
                            }
                        }
                        _ => primitive_row(ui, &i.to_string(), val, indent + 1, colors),
                    }
                }
                closing_bracket(ui, value, indent, colors);
            }
        }
        _ => primitive_row(ui, "", value, indent, colors),
    }
}

fn render_children(
    ui: &mut egui::Ui,
    value: &Value,
    path: &str,
    indent: usize,
    expanded: &mut ExpandedPaths,
    colors: &ThemeColors,
) {
    match value {
        Value::Object(map) => {
            for (key, val) in map {
                let child_path = format!("{path}/{key}");
                match val {
                    Value::Object(_) | Value::Array(_) => {
                        let key_is_expanded = expanded.0.contains(&child_path);
                        let suffix = match val {
                            Value::Object(_) => {
                                if key_is_expanded {
                                    "{"
                                } else {
                                    "{…}"
                                }
                            }
                            Value::Array(_) => {
                                if key_is_expanded {
                                    "["
                                } else {
                                    "[…]"
                                }
                            }
                            _ => unreachable!(),
                        };
                        let key_label = format!("{key}: {suffix}");
                        if collapsible_key_row(ui, &key_label, indent + 1, key_is_expanded, colors)
                        {
                            if key_is_expanded {
                                expanded.0.remove(&child_path);
                            } else {
                                expanded.0.insert(child_path.clone());
                            }
                        }
                        if key_is_expanded {
                            render_children(ui, val, &child_path, indent + 1, expanded, colors);
                            closing_bracket(ui, val, indent + 1, colors);
                        }
                    }
                    _ => kv_row(ui, key, val, indent + 1, colors),
                }
            }
        }
        Value::Array(arr) => {
            for (i, val) in arr.iter().enumerate() {
                let child_path = format!("{path}/{i}");
                match val {
                    Value::Object(_) | Value::Array(_) => {
                        let key_is_expanded = expanded.0.contains(&child_path);
                        let suffix = match val {
                            Value::Object(_) => {
                                if key_is_expanded {
                                    "{"
                                } else {
                                    "{…}"
                                }
                            }
                            Value::Array(_) => {
                                if key_is_expanded {
                                    "["
                                } else {
                                    "[…]"
                                }
                            }
                            _ => unreachable!(),
                        };
                        let key_label = format!("{i}: {suffix}");
                        if collapsible_key_row(ui, &key_label, indent + 1, key_is_expanded, colors)
                        {
                            if key_is_expanded {
                                expanded.0.remove(&child_path);
                            } else {
                                expanded.0.insert(child_path.clone());
                            }
                        }
                        if key_is_expanded {
                            render_children(ui, val, &child_path, indent + 1, expanded, colors);
                            closing_bracket(ui, val, indent + 1, colors);
                        }
                    }
                    _ => primitive_row(ui, &i.to_string(), val, indent + 1, colors),
                }
            }
        }
        _ => {}
    }
}

// =============================================================================
// Row helpers
// =============================================================================

const INDENT_PX: f32 = 16.0;
const FONT_SIZE: f32 = 12.5;

/// Caret IconButton. Returns true if clicked.
fn caret_button(ui: &mut egui::Ui, is_expanded: bool) -> bool {
    let icon = if is_expanded {
        egui_phosphor::regular::CARET_DOWN
    } else {
        egui_phosphor::regular::CARET_RIGHT
    };
    IconButton::render(
        ui,
        IconButtonProps {
            icon,
            frame: false,
            tooltip: None,
            badge_color: None,
            size: None,
            disabled: false,
        },
    )
    .clicked
}

/// Top-level collapsible row (no key label). Returns true if toggled.
fn collapsible_row(
    ui: &mut egui::Ui,
    label: &str,
    indent: usize,
    is_expanded: bool,
    colors: &ThemeColors,
) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.add_space(indent as f32 * INDENT_PX);
        if caret_button(ui, is_expanded) {
            clicked = true;
        }
        ui.add(egui::Label::new(
            egui::RichText::new(label)
                .size(FONT_SIZE)
                .color(colors.overlay1)
                .monospace(),
        ));
    });
    clicked
}

/// Key row where the value is a nested object/array. Returns true if toggled.
fn collapsible_key_row(
    ui: &mut egui::Ui,
    label: &str,
    indent: usize,
    is_expanded: bool,
    colors: &ThemeColors,
) -> bool {
    let mut clicked = false;
    ui.horizontal(|ui| {
        ui.add_space(indent as f32 * INDENT_PX);
        if caret_button(ui, is_expanded) {
            clicked = true;
        }
        if let Some(colon) = label.find(": ") {
            let key = &label[..colon];
            let rest = &label[colon..];
            ui.add(egui::Label::new(
                egui::RichText::new(key)
                    .size(FONT_SIZE)
                    .color(colors.key)
                    .monospace(),
            ));
            ui.add(egui::Label::new(
                egui::RichText::new(rest)
                    .size(FONT_SIZE)
                    .color(colors.overlay1)
                    .monospace(),
            ));
        } else {
            ui.add(egui::Label::new(
                egui::RichText::new(label)
                    .size(FONT_SIZE)
                    .color(colors.key)
                    .monospace(),
            ));
        }
    });
    clicked
}

/// key: primitive-value row (leaf).
fn kv_row(ui: &mut egui::Ui, key: &str, val: &Value, indent: usize, colors: &ThemeColors) {
    ui.horizontal(|ui| {
        // Extra INDENT_PX to align with text after caret icon
        ui.add_space(indent as f32 * INDENT_PX + INDENT_PX);
        ui.add(egui::Label::new(
            egui::RichText::new(format!("{key}: "))
                .size(FONT_SIZE)
                .color(colors.key)
                .monospace(),
        ));
        let (text, color) = value_display(val, colors);
        ui.add(egui::Label::new(
            egui::RichText::new(text)
                .size(FONT_SIZE)
                .color(color)
                .monospace(),
        ));
    });
}

/// Array index: primitive-value row.
fn primitive_row(
    ui: &mut egui::Ui,
    prefix: &str,
    val: &Value,
    indent: usize,
    colors: &ThemeColors,
) {
    ui.horizontal(|ui| {
        ui.add_space(indent as f32 * INDENT_PX + INDENT_PX);
        if !prefix.is_empty() {
            ui.add(egui::Label::new(
                egui::RichText::new(format!("{prefix}: "))
                    .size(FONT_SIZE)
                    .color(colors.overlay1)
                    .monospace(),
            ));
        }
        let (text, color) = value_display(val, colors);
        ui.add(egui::Label::new(
            egui::RichText::new(text)
                .size(FONT_SIZE)
                .color(color)
                .monospace(),
        ));
    });
}

/// Closing `}` or `]` bracket.
fn closing_bracket(ui: &mut egui::Ui, value: &Value, indent: usize, colors: &ThemeColors) {
    let bracket = match value {
        Value::Object(_) => "}",
        Value::Array(_) => "]",
        _ => return,
    };
    ui.horizontal(|ui| {
        ui.add_space(indent as f32 * INDENT_PX + INDENT_PX);
        ui.add(egui::Label::new(
            egui::RichText::new(bracket)
                .size(FONT_SIZE)
                .color(colors.overlay1)
                .monospace(),
        ));
    });
}

/// `(display_string, color)` for a primitive JSON value.
fn value_display(val: &Value, colors: &ThemeColors) -> (String, Color32) {
    match val {
        Value::String(s) => (format!("\"{s}\""), colors.string),
        Value::Number(n) => (n.to_string(), colors.number),
        Value::Bool(b) => (b.to_string(), colors.primary),
        Value::Null => ("null".to_string(), colors.overlay1),
        _ => (val.to_string(), colors.text),
    }
}
