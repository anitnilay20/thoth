use std::collections::HashSet;

use serde_json::Value;

use crate::components::DataRow;
use crate::theme::{ROW_HEIGHT, TextToken, color_to_hex};

use super::JsonTree;

/// A render-ready row, mapped directly onto [`DataRow`] fields.
struct TreeRow {
    /// Tree path — identity for expansion state and the row's interaction id.
    path: String,
    indent: usize,
    /// `key`, `key: value`, an opening `{`/`[` (possibly with a key), or a
    /// closing `}`/`]`.
    text: String,
    key_token: TextToken,
    value_token: Option<TextToken>,
    /// `Some(expanded)` for expandable container rows; `None` for leaves/closers.
    caret: Option<bool>,
}

#[derive(Clone, Default)]
struct ExpandedPaths(HashSet<String>);

impl JsonTree {
    /// Render the tree into the available area.
    pub fn show(&self, ui: &mut egui::Ui) {
        let id = if self.id.is_empty() {
            "json-tree"
        } else {
            self.id.as_str()
        };
        let base_id = egui::Id::new(id);
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

        let mut rows: Vec<TreeRow> = Vec::new();
        flatten_value(&self.value, "", 0, &expanded, &mut rows);
        let row_count = rows.len();
        let mut toggle_path: Option<String> = None;

        // Zebra striping: faint fill on every other displayed row.
        let stripe = color_to_hex(ui.visuals().faint_bg_color);

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show_rows(ui, ROW_HEIGHT, row_count, |ui, range| {
                for idx in range {
                    let row = &rows[idx];
                    let background = (idx % 2 == 1).then(|| stripe.clone());
                    let out = DataRow::builder()
                        .display_text(row.text.clone())
                        .row_id(row.path.clone())
                        .key_token(row.key_token)
                        .maybe_value_token(row.value_token)
                        .maybe_caret(row.caret)
                        .maybe_background(background)
                        .syntax_highlighting(true)
                        .indent(row.indent)
                        .build()
                        .show(ui);

                    // A caret click toggles this container; clicking the row
                    // body also toggles, so expandable rows feel responsive.
                    if row.caret.is_some() && (out.caret_clicked || out.clicked) {
                        toggle_path = Some(row.path.clone());
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
    out: &mut Vec<TreeRow>,
) {
    match value {
        Value::Object(map) => {
            let is_expanded = expanded.0.contains(path);
            out.push(TreeRow {
                path: path.to_string(),
                indent,
                text: if is_expanded {
                    "{".to_string()
                } else {
                    format!("{{…}} ({} keys)", map.len())
                },
                key_token: TextToken::Bracket,
                value_token: None,
                caret: Some(is_expanded),
            });
            if is_expanded {
                for (key, val) in map {
                    flatten_keyed(
                        key,
                        val,
                        &format!("{path}/{key}"),
                        indent + 1,
                        expanded,
                        out,
                    );
                }
                out.push(closing(path, indent, "}"));
            }
        }
        Value::Array(arr) => {
            let is_expanded = expanded.0.contains(path);
            out.push(TreeRow {
                path: path.to_string(),
                indent,
                text: if is_expanded {
                    "[".to_string()
                } else {
                    format!("[…] ({} items)", arr.len())
                },
                key_token: TextToken::Bracket,
                value_token: None,
                caret: Some(is_expanded),
            });
            if is_expanded {
                for (i, val) in arr.iter().enumerate() {
                    flatten_keyed(
                        &i.to_string(),
                        val,
                        &format!("{path}/{i}"),
                        indent + 1,
                        expanded,
                        out,
                    );
                }
                out.push(closing(path, indent, "]"));
            }
        }
        _ => {
            let (text, token) = scalar_display(value);
            out.push(TreeRow {
                path: path.to_string(),
                indent,
                text,
                key_token: token,
                value_token: None,
                caret: None,
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
    out: &mut Vec<TreeRow>,
) {
    match val {
        Value::Object(map) => {
            let is_expanded = expanded.0.contains(path);
            let suffix = if is_expanded {
                "{".to_string()
            } else {
                format!("{{…}} ({} keys)", map.len())
            };
            out.push(TreeRow {
                path: path.to_string(),
                indent,
                text: format!("{key}: {suffix}"),
                key_token: TextToken::Key,
                value_token: Some(TextToken::Bracket),
                caret: Some(is_expanded),
            });
            if is_expanded {
                for (k, v) in map {
                    flatten_keyed(k, v, &format!("{path}/{k}"), indent + 1, expanded, out);
                }
                out.push(closing(path, indent, "}"));
            }
        }
        Value::Array(arr) => {
            let is_expanded = expanded.0.contains(path);
            let suffix = if is_expanded {
                "[".to_string()
            } else {
                format!("[…] ({} items)", arr.len())
            };
            out.push(TreeRow {
                path: path.to_string(),
                indent,
                text: format!("{key}: {suffix}"),
                key_token: TextToken::Key,
                value_token: Some(TextToken::Bracket),
                caret: Some(is_expanded),
            });
            if is_expanded {
                for (i, v) in arr.iter().enumerate() {
                    flatten_keyed(
                        &i.to_string(),
                        v,
                        &format!("{path}/{i}"),
                        indent + 1,
                        expanded,
                        out,
                    );
                }
                out.push(closing(path, indent, "]"));
            }
        }
        _ => {
            let (value_text, token) = scalar_display(val);
            out.push(TreeRow {
                path: path.to_string(),
                indent,
                text: format!("{key}: {value_text}"),
                key_token: TextToken::Key,
                value_token: Some(token),
                caret: None,
            });
        }
    }
}

/// A closing-bracket row (`}` / `]`), painted as punctuation with no caret.
fn closing(path: &str, indent: usize, bracket: &str) -> TreeRow {
    TreeRow {
        path: format!("{path}/_close"),
        indent,
        text: bracket.to_string(),
        key_token: TextToken::Bracket,
        value_token: None,
        caret: None,
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

/// Format a scalar value's display text and its syntax token.
fn scalar_display(val: &Value) -> (String, TextToken) {
    match val {
        Value::String(s) => (format!("\"{s}\""), TextToken::Str),
        Value::Number(n) => (n.to_string(), TextToken::Number),
        Value::Bool(b) => (b.to_string(), TextToken::Boolean),
        Value::Null => ("null".to_string(), TextToken::Boolean),
        _ => (val.to_string(), TextToken::Bracket),
    }
}
