use crate::file::lazy_loader::LazyJsonFile;
use crate::helpers::{
    LruCache, format_simple_kv, get_object_string, preview_value, split_root_rel,
};
use crate::theme::{TextPalette, TextToken, row_fill, selected_row_bg};
use eframe::egui::{self, RichText, Ui};
use serde_json::Value;
use std::collections::HashSet;

use super::viewer_trait::FileFormatViewer;

/// JSON-specific tree viewer that handles expansion and rendering
///
/// Implements `FileFormatViewer` trait to integrate with the FileViewer architecture.
pub struct JsonTreeViewer {
    /// Tree expansion state (paths like "0", "0.user", "0.items[2]")
    expanded: HashSet<String>,

    /// Flattened render list
    rows: Vec<JsonRow>,
}

#[derive(Clone)]
struct JsonRow {
    path: String,
    indent: usize,
    is_expandable: bool,
    is_expanded: bool,
    display_text: String,
    text_token: (TextToken, Option<TextToken>),
}

impl Default for JsonTreeViewer {
    fn default() -> Self {
        Self::new()
    }
}

impl JsonTreeViewer {
    pub fn new() -> Self {
        Self {
            expanded: HashSet::new(),
            rows: Vec::new(),
        }
    }

    /// Rebuild rows based on visible roots and cache
    pub fn rebuild_rows(
        &mut self,
        visible_roots: &Option<Vec<usize>>,
        cache: &mut LruCache<usize, Value>,
        loader: &mut LazyJsonFile,
        total_len: usize,
    ) {
        self.rows.clear();

        // Determine which root indices to render
        let indices: Vec<usize> = if let Some(list) = visible_roots.as_ref() {
            list.clone()
        } else {
            (0..total_len).collect()
        };

        for i in indices {
            let path = i.to_string();
            let is_expanded = self.expanded.contains(&path);

            let display_text = if is_expanded {
                format!("[{}]: {{", i)
            } else {
                format!("[{}]: (…) ", i)
            };

            self.rows.push(JsonRow {
                path: path.clone(),
                indent: 0,
                is_expandable: true,
                is_expanded,
                display_text,
                text_token: (TextToken::Key, Some(TextToken::Bracket)),
            });

            if is_expanded {
                // Try to get from cache, or load from file
                let value = if let Some(v) = cache.get(&i) {
                    v.clone()
                } else {
                    match loader.get(i) {
                        Ok(v) => {
                            cache.put(i, v.clone());
                            v
                        }
                        Err(_) => continue,
                    }
                };

                self.build_rows_from_value(&value, &path, 1);

                // Closing brace
                self.rows.push(JsonRow {
                    path: format!("{}/_close", path),
                    indent: 0,
                    is_expandable: false,
                    is_expanded: false,
                    display_text: "}".to_string(),
                    text_token: (TextToken::Bracket, None),
                });
            }
        }
    }

    /// Build rows from a JSON value recursively
    fn build_rows_from_value(&mut self, value: &Value, path: &str, indent: usize) {
        match value {
            Value::Object(map) => {
                for (key, val) in map.iter() {
                    let new_path = format!("{}.{}", path, key);
                    let is_expandable = matches!(val, Value::Object(_) | Value::Array(_));
                    let is_expanded = self.expanded.contains(&new_path);

                    let display_text = if is_expandable {
                        format!("\"{}\": {}", key, if is_expanded { "{" } else { "{}" })
                    } else {
                        format_simple_kv(key, val)
                    };

                    self.rows.push(JsonRow {
                        path: new_path.clone(),
                        indent,
                        is_expandable,
                        is_expanded,
                        display_text,
                        text_token: (
                            TextToken::Key,
                            Some(if is_expandable {
                                TextToken::Bracket
                            } else {
                                TextToken::from(&mut val.clone())
                            }),
                        ),
                    });

                    if is_expanded {
                        self.build_rows_from_value(val, &new_path, indent + 1);
                        self.rows.push(JsonRow {
                            path: format!("{}/_close", new_path),
                            indent,
                            is_expandable: false,
                            is_expanded: false,
                            display_text: "}".to_string(),
                            text_token: (TextToken::Bracket, None),
                        });
                    }
                }
            }
            Value::Array(arr) => {
                for (idx, val) in arr.iter().enumerate() {
                    let new_path = format!("{}[{}]", path, idx);
                    let is_expandable = matches!(val, Value::Object(_) | Value::Array(_));
                    let is_expanded = self.expanded.contains(&new_path);

                    let display_text = if is_expandable {
                        format!("[{}]: {}", idx, if is_expanded { "[" } else { "[]" })
                    } else {
                        format!("[{}]: {}", idx, preview_value(val))
                    };

                    self.rows.push(JsonRow {
                        path: new_path.clone(),
                        indent,
                        is_expandable,
                        is_expanded,
                        display_text,
                        text_token: (TextToken::Key, Some(TextToken::Bracket)),
                    });

                    if is_expanded {
                        self.build_rows_from_value(val, &new_path, indent + 1);
                        self.rows.push(JsonRow {
                            path: format!("{}/_close", new_path),
                            indent,
                            is_expandable: false,
                            is_expanded: false,
                            display_text: "]".to_string(),
                            text_token: (TextToken::Bracket, None),
                        });
                    }
                }
            }
            _ => {
                // Primitives
                self.rows.push(JsonRow {
                    path: path.to_string(),
                    indent,
                    is_expandable: false,
                    is_expanded: false,
                    display_text: preview_value(value).to_string(),
                    text_token: (TextToken::from(&mut value.clone()), None),
                });
            }
        }
    }

    /// Render the JSON tree and return whether rows need to be rebuilt
    pub fn render(
        &mut self,
        ui: &mut Ui,
        selected: &mut Option<String>,
        cache: &mut LruCache<usize, Value>,
        _loader: &mut LazyJsonFile,
    ) -> bool {
        let row_count = self.rows.len();
        let row_height = 20.0;

        let mut toggles: Vec<String> = Vec::new();
        let mut new_selected: Option<String> = None;
        let mut copy_clipboard: Option<String> = None;

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show_rows(ui, row_height, row_count, |ui, row_range| {
                let visuals = ui.visuals();
                let palette = TextPalette::for_visuals(visuals);

                let rows = self.rows.clone();

                for row_index in row_range.clone() {
                    if let Some(row) = rows.get(row_index) {
                        let indent = row.indent;
                        let is_expandable = row.is_expandable;
                        let path = row.path.clone();
                        let display = row.display_text.clone();
                        let mut parts = display.splitn(2, ':');
                        let display1 = parts.next().unwrap_or("");
                        let display2 = parts.next().unwrap_or("");
                        let is_key_display = !display2.is_empty() && row.text_token.1.is_some();
                        let mut clicked = false;
                        let mut right_clicked = false;

                        // Selected background
                        let is_selected = selected.as_deref() == Some(path.as_str());
                        let bg = if is_selected {
                            selected_row_bg(ui)
                        } else {
                            row_fill(row_index, ui)
                        };

                        egui::Frame::new().fill(bg).show(ui, |ui| {
                            let rect = ui.max_rect();
                            let id = ui.id().with(&path);
                            let resp = ui.interact(rect, id, egui::Sense::click());
                            clicked = resp.clicked();
                            right_clicked |= resp.clicked_by(egui::PointerButton::Secondary);

                            ui.set_min_width(ui.available_width());
                            ui.horizontal(|ui| {
                                ui.add_space(indent as f32 * 12.0);

                                if is_expandable {
                                    let toggle_icon = if row.is_expanded { "-" } else { "+" };
                                    if ui
                                        .selectable_label(
                                            false,
                                            RichText::new(toggle_icon).monospace(),
                                        )
                                        .clicked()
                                    {
                                        toggles.push(path.clone());
                                    }
                                } else {
                                    ui.add_space(23.0);
                                }

                                let label_resp = ui.add(egui::Label::new(
                                    RichText::new(format!(
                                        "{}{}",
                                        display1,
                                        if is_key_display { ":" } else { "" }
                                    ))
                                    .monospace()
                                    .color(palette.color(row.text_token.0)),
                                ));

                                clicked |= label_resp.clicked();
                                right_clicked |=
                                    label_resp.clicked_by(egui::PointerButton::Secondary);

                                if is_key_display {
                                    let key_resp = ui.add(egui::Label::new(
                                        RichText::new(display2)
                                            .monospace()
                                            .color(palette.color(row.text_token.1.unwrap())),
                                    ));
                                    clicked |= key_resp.clicked();
                                    right_clicked |=
                                        key_resp.clicked_by(egui::PointerButton::Secondary);
                                }
                            });

                            if clicked || right_clicked {
                                new_selected = Some(path.clone());
                            }

                            resp.context_menu(|ui| {
                                if ui.button("Copy key").clicked() {
                                    let path_clone = path.clone();
                                    let split =
                                        path_clone.split_inclusive('.').next_back().unwrap_or("");
                                    copy_clipboard = Some(split.to_string());
                                    ui.close();
                                }

                                let show_value_menu = is_key_display
                                    && !display2.starts_with(" [")
                                    && !display2.starts_with(" {")
                                    && !display2.starts_with(" (");

                                if show_value_menu && ui.button("Copy Value").clicked() {
                                    copy_clipboard = Some(display2.trim().to_string());
                                    ui.close();
                                }

                                if ui.button("Copy Object").clicked() {
                                    if let Some((root_idx, rel)) = split_root_rel(&path) {
                                        if let Some(value) = cache.get(&root_idx).cloned() {
                                            copy_clipboard = get_object_string(value, rel);
                                        }
                                    }
                                    ui.close();
                                }

                                if ui.button("Copy path").clicked() {
                                    copy_clipboard = Some(path.clone());
                                    ui.close();
                                }
                            });
                        });
                    }
                }
            });

        if let Some(sel) = new_selected {
            *selected = Some(sel);
        }

        if let Some(text) = copy_clipboard {
            ui.output_mut(|o| o.commands.push(egui::OutputCommand::CopyText(text)));
        }

        // Handle toggles
        let needs_rebuild = !toggles.is_empty();
        if needs_rebuild {
            for path in toggles {
                if !self.expanded.insert(path.clone()) {
                    self.expanded.remove(&path);
                }
            }
        }

        needs_rebuild
    }
}

// Implement FileFormatViewer trait for JsonTreeViewer
impl FileFormatViewer for JsonTreeViewer {
    fn reset(&mut self) {
        self.expanded.clear();
        self.rows.clear();
    }

    fn rebuild_view(
        &mut self,
        visible_roots: &Option<Vec<usize>>,
        cache: &mut LruCache<usize, Value>,
        loader: &mut LazyJsonFile,
        total_len: usize,
    ) {
        self.rebuild_rows(visible_roots, cache, loader, total_len);
    }

    fn render(
        &mut self,
        ui: &mut Ui,
        selected: &mut Option<String>,
        cache: &mut LruCache<usize, Value>,
        loader: &mut LazyJsonFile,
    ) -> bool {
        self.render(ui, selected, cache, loader)
    }

    // ========================================================================
    // Navigation & Tree Operations
    // ========================================================================

    fn expand_selected(&mut self, selected: &Option<String>) -> bool {
        if let Some(path) = selected {
            // Insert returns false if already present
            if self.expanded.insert(path.clone()) {
                return true; // Need rebuild
            }
        }
        false
    }

    fn collapse_selected(&mut self, selected: &Option<String>) -> bool {
        if let Some(path) = selected {
            // Remove returns true if was present
            if self.expanded.remove(path) {
                return true; // Need rebuild
            }
        }
        false
    }

    fn expand_all(&mut self) -> bool {
        // Expand all expandable rows
        let paths_to_expand: Vec<String> = self
            .rows
            .iter()
            .filter(|row| row.is_expandable && !row.is_expanded)
            .map(|row| row.path.clone())
            .collect();

        if !paths_to_expand.is_empty() {
            for path in paths_to_expand {
                self.expanded.insert(path);
            }
            return true; // Need rebuild
        }
        false
    }

    fn collapse_all(&mut self) -> bool {
        if !self.expanded.is_empty() {
            self.expanded.clear();
            return true; // Need rebuild
        }
        false
    }

    fn move_selection_up(&self, current: &Option<String>) -> Option<String> {
        if self.rows.is_empty() {
            return None;
        }

        if let Some(current_path) = current {
            // Find current index
            if let Some(idx) = self.rows.iter().position(|r| r.path == *current_path) {
                if idx > 0 {
                    // Move to previous row
                    return Some(self.rows[idx - 1].path.clone());
                }
            }
        } else {
            // No selection, select last item
            return Some(self.rows.last()?.path.clone());
        }
        None
    }

    fn move_selection_down(&self, current: &Option<String>) -> Option<String> {
        if self.rows.is_empty() {
            return None;
        }

        if let Some(current_path) = current {
            // Find current index
            if let Some(idx) = self.rows.iter().position(|r| r.path == *current_path) {
                if idx < self.rows.len() - 1 {
                    // Move to next row
                    return Some(self.rows[idx + 1].path.clone());
                }
            }
        } else {
            // No selection, select first item
            return Some(self.rows.first()?.path.clone());
        }
        None
    }

    // ========================================================================
    // Clipboard Operations
    // ========================================================================

    fn copy_selected_key(&self, selected: &Option<String>) -> Option<String> {
        if let Some(path) = selected {
            // Extract the key from the path (last segment)
            let split = path.split_inclusive('.').next_back()?;
            return Some(split.to_string());
        }
        None
    }

    fn copy_selected_value(
        &self,
        selected: &Option<String>,
        cache: &mut LruCache<usize, Value>,
        loader: &mut LazyJsonFile,
    ) -> Option<String> {
        if let Some(path) = selected {
            // Find the row to get display text
            if let Some(row) = self.rows.iter().find(|r| r.path == *path) {
                // Parse display text to extract value part
                let parts: Vec<&str> = row.display_text.splitn(2, ':').collect();
                if parts.len() == 2 {
                    return Some(parts[1].trim().to_string());
                }
            }
        }
        let _ = (cache, loader); // Suppress unused warnings for now
        None
    }

    fn copy_selected_object(
        &self,
        selected: &Option<String>,
        cache: &mut LruCache<usize, Value>,
        loader: &mut LazyJsonFile,
    ) -> Option<String> {
        if let Some(path) = selected {
            if let Some((root_idx, rel)) = split_root_rel(path) {
                // Try to get from cache first
                let value = if let Some(v) = cache.get(&root_idx) {
                    v.clone()
                } else {
                    // Load from file
                    match loader.get(root_idx) {
                        Ok(v) => {
                            cache.put(root_idx, v.clone());
                            v
                        }
                        Err(_) => return None,
                    }
                };

                return get_object_string(value, rel);
            }
        }
        None
    }

    fn copy_selected_path(&self, selected: &Option<String>) -> Option<String> {
        selected.clone()
    }
}
