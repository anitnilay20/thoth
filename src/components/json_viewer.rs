use eframe::egui;
use egui::{RichText, Ui};
use serde_json::Value;
use std::collections::HashSet;

#[derive(Default)]
pub struct JsonViewer {
    pub expanded: HashSet<String>,
    pub rows: Vec<JsonRow>,
    pub root: Option<Value>,
}

#[derive(Clone)]
pub struct JsonRow {
    path: String,
    indent: usize,
    is_expandable: bool,
    is_expanded: bool,
    display_text: String,
}

impl JsonViewer {
    pub fn load(&mut self, root: Value) {
        self.rows.clear();
        let root_clone = root.clone();
        self.root = Some(root);
        self.build_rows_from_value(&root_clone, "", 0);
    }

    fn build_rows_from_value(&mut self, value: &Value, path: &str, indent: usize) {
        match value {
            Value::Object(map) => {
                for (key, val) in map {
                    let new_path = format!("{}/{}", path, key);
                    let is_expandable = val.is_object() || val.is_array();
                    let is_expanded = self.expanded.contains(&new_path);
                    let display_text = if is_expandable {
                        format!(
                            "\"{}\": {}",
                            key,
                            self.open_bracket_symbol(val, is_expanded)
                        )
                    } else {
                        self.format_simple_value(key, val)
                    };

                    self.rows.push(JsonRow {
                        path: new_path.clone(),
                        indent,
                        is_expandable,
                        is_expanded,
                        display_text,
                    });

                    if is_expanded {
                        self.build_rows_from_value(val, &new_path, indent + 1);
                        self.rows.push(JsonRow {
                            path: format!("{}/_close", &new_path),
                            indent,
                            is_expandable: false,
                            is_expanded: false,
                            display_text: self.closing_bracket_symbol(val).to_string(),
                        });
                    }
                }
            }
            Value::Array(values) => {
                for (i, val) in values.iter().enumerate() {
                    let new_path = format!("{}[{}]", path, i);
                    let is_expandable = val.is_object() || val.is_array();
                    let is_expanded = self.expanded.contains(&new_path);
                    let display_text = if is_expandable {
                        format!("[{}]: {}", i, self.open_bracket_symbol(val, is_expanded))
                    } else {
                        format!("[{}]: {}", i, val)
                    };

                    self.rows.push(JsonRow {
                        path: new_path.clone(),
                        indent,
                        is_expandable,
                        is_expanded,
                        display_text,
                    });

                    if is_expanded {
                        self.build_rows_from_value(val, &new_path, indent + 1);
                        self.rows.push(JsonRow {
                            path: format!("{}/_close", &new_path),
                            indent,
                            is_expandable: false,
                            is_expanded: false,
                            display_text: self.closing_bracket_symbol(val).to_string(),
                        });
                    }
                }
            }
            _ => {
                self.rows.push(JsonRow {
                    path: path.to_string(),
                    indent,
                    is_expandable: false,
                    is_expanded: false,
                    display_text: value.to_string(),
                });
            }
        }
    }

    pub fn ui(&mut self, ui: &mut Ui) {
        let row_count = self.rows.len();
        let row_height = 20.0;
        let visible_rows = ui.available_height() / row_height;
        let scroll_y = ui.clip_rect().top();
        let start_row = (scroll_y / row_height).floor() as usize;
        let end_row = (start_row + visible_rows as usize + 5).min(row_count);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show_rows(ui, row_height, row_count, |ui, row_range| {
                for row_index in row_range.clone() {
                    if let Some(row) = self.rows.get(row_index).cloned() {
                        ui.horizontal(|ui| {
                            ui.add_space(row.indent as f32 * 12.0);

                            if row.is_expandable {
                                let toggle_icon = if row.is_expanded { "-" } else { "+" };
                                if ui.selectable_label(false, toggle_icon).clicked() {
                                    if row.is_expanded {
                                        self.expanded.remove(&row.path);
                                    } else {
                                        self.expanded.insert(row.path.clone());
                                    }
                                    // Rebuild the rows after toggling
                                    if let Some(root_clone) = self.root.clone() {
                                        self.rows.clear();
                                        self.build_rows_from_value(&root_clone, "", 0);
                                    }
                                }
                            } else {
                                ui.add_space(12.0);
                            }

                            ui.label(RichText::new(&row.display_text).monospace());
                        });
                    }
                }
            });
    }

    fn format_simple_value(&self, key: &str, val: &Value) -> String {
        match val {
            Value::String(s) => format!("\"{}\": \"{}\"", key, s),
            _ => format!("\"{}\": {}", key, val),
        }
    }

    fn closing_bracket_symbol(&self, value: &Value) -> &str {
        match value {
            Value::Object(_) => "}",
            Value::Array(_) => "]",
            _ => "",
        }
    }

    fn open_bracket_symbol(&self, value: &Value, is_expanded: bool) -> &str {
        if is_expanded {
            match value {
                Value::Object(_) => "{",
                Value::Array(_) => "[",
                _ => "",
            }
        } else {
            match value {
                Value::Object(_) => "{}",
                Value::Array(_) => "[]",
                _ => "",
            }
        }
    }
}
