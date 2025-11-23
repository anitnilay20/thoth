use crate::components::data_row::{DataRow, DataRowProps};
use crate::components::icon_button::{IconButton, IconButtonProps};
use crate::components::traits::StatelessComponent;
use crate::file::lazy_loader::LazyJsonFile;
use crate::helpers::{
    LruCache, format_simple_kv, get_object_string, preview_value, scroll_to_selection,
    split_root_rel,
};
use crate::theme::{ROW_HEIGHT, TextToken, row_fill, selected_row_bg};
use eframe::egui::{self, Ui};
use serde_json::Value;
use std::collections::HashSet;

use super::context_menu::{
    ContextMenuConfig, ContextMenuHandler, execute_context_menu_action, render_context_menu,
};
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
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

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
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

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
                                TextToken::from(val)
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
                    text_token: (TextToken::from(value), None),
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
        loader: &mut LazyJsonFile,
        should_scroll_to_selection: &mut bool,
    ) -> bool {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        let row_count = self.rows.len();
        let row_height = ROW_HEIGHT;

        let mut toggles: Vec<String> = Vec::new();
        let mut new_selected: Option<String> = None;
        let mut copy_clipboard: Option<String> = None;

        let scroll_area = egui::ScrollArea::both()
            .auto_shrink([false, false])
            .id_salt("json_tree_scroll");

        scroll_area.show_rows(ui, row_height, row_count, |ui, row_range| {
            if let Some(selected_path) = selected.as_ref() {
                if let Some(row_idx) = self.rows.iter().position(|r| r.path == *selected_path) {
                    scroll_to_selection(
                        ui,
                        &row_range,
                        row_idx,
                        row_height,
                        should_scroll_to_selection,
                    );
                }
            }

            // Get indent guide color from theme
            let guide_color = ui.ctx().memory(|mem| {
                mem.data
                    .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
                    .map(|colors| colors.indent_guide)
                    .unwrap_or_else(|| egui::Color32::from_rgb(100, 100, 100))
            });

            for row_index in row_range {
                if let Some(row) = self.rows.get(row_index) {
                    let path = &row.path;
                    let display = &row.display_text;
                    let display2_parts: Vec<&str> = display.splitn(2, ':').collect();
                    let is_key_display = display2_parts.len() == 2 && row.text_token.1.is_some();
                    let display2 = if is_key_display {
                        display2_parts.get(1).unwrap_or(&"")
                    } else {
                        ""
                    };

                    // Selected background with alternating colors
                    let bg = if selected.as_deref() == Some(path.as_str()) {
                        selected_row_bg(ui)
                    } else {
                        row_fill(row_index, ui)
                    };

                    // Draw indent guide lines before rendering row content
                    if row.indent > 0 {
                        let painter = ui.painter();
                        let rect = ui.available_rect_before_wrap();
                        let row_y_min = rect.min.y;
                        let row_y_max = row_y_min + row_height;

                        // Draw a vertical line for each indent level
                        for level in 0..row.indent {
                            let x = rect.min.x + (level as f32 * 16.0) + 8.0;
                            painter.line_segment(
                                [egui::pos2(x, row_y_min), egui::pos2(x, row_y_max)],
                                egui::Stroke::new(1.0, guide_color),
                            );
                        }
                    }

                    // Render the row with toggle button (if expandable) and content
                    let mut toggle_clicked = false;

                    ui.horizontal(|ui| {
                        // Indentation spacing
                        ui.add_space(row.indent as f32 * 16.0);

                        // Toggle button for expandable rows (or spacer for non-expandable)
                        if row.is_expandable {
                            let toggle_icon = if row.is_expanded {
                                egui_phosphor::regular::CARET_DOWN
                            } else {
                                egui_phosphor::regular::CARET_RIGHT
                            };
                            let tooltip_text = if row.is_expanded {
                                "Collapse (Space/Enter)"
                            } else {
                                "Expand (Space/Enter)"
                            };
                            if IconButton::render(
                                ui,
                                IconButtonProps {
                                    icon: toggle_icon,
                                    frame: false,
                                    tooltip: Some(tooltip_text),
                                    badge_color: None,
                                    size: None,
                                },
                            )
                            .clicked
                            {
                                toggle_clicked = true;
                            }
                        } else {
                            // Add invisible button to maintain consistent spacing
                            ui.add_enabled_ui(false, |ui| {
                                ui.visuals_mut().widgets.inactive.bg_fill =
                                    egui::Color32::TRANSPARENT;
                                ui.visuals_mut().widgets.inactive.weak_bg_fill =
                                    egui::Color32::TRANSPARENT;
                                IconButton::render(
                                    ui,
                                    IconButtonProps {
                                        icon: " ",
                                        frame: false,
                                        tooltip: None,
                                        badge_color: None,
                                        size: None,
                                    },
                                );
                            });
                        }

                        // Use DataRow component for the content
                        let output = DataRow::render(
                            ui,
                            DataRowProps {
                                display_text: display,
                                text_tokens: row.text_token,
                                background: bg,
                                row_id: path,
                            },
                        );

                        if toggle_clicked {
                            toggles.push(path.clone());
                        } else if output.clicked || output.right_clicked {
                            new_selected = Some(path.clone());
                        }

                        // Context menu using the response from DataRow
                        output.response.context_menu(|ui| {
                            let config = ContextMenuConfig::from_display(is_key_display, display2);
                            render_context_menu(ui, &config, |action| {
                                if let Some(text) = execute_context_menu_action(
                                    action,
                                    self,
                                    &Some(path.clone()),
                                    cache,
                                    loader,
                                ) {
                                    copy_clipboard = Some(text);
                                }
                            });
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

        // Reset scroll flag after rendering
        *should_scroll_to_selection = false;

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

// Implement ContextMenuHandler trait for JsonTreeViewer
impl ContextMenuHandler for JsonTreeViewer {
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
        should_scroll_to_selection: &mut bool,
    ) -> bool {
        self.render(ui, selected, cache, loader, should_scroll_to_selection)
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
                } else {
                    // Already at first item, stay there
                    return Some(current_path.clone());
                }
            }
            // Current selection not found in rows (perhaps view was rebuilt)
            // Start from last item
            return Some(self.rows.last()?.path.clone());
        }

        // No selection, select last item
        Some(self.rows.last()?.path.clone())
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
                } else {
                    // Already at last item, stay there
                    return Some(current_path.clone());
                }
            }
            // Current selection not found in rows (perhaps view was rebuilt)
            // Start from first item
            return Some(self.rows.first()?.path.clone());
        }

        // No selection, select first item
        Some(self.rows.first()?.path.clone())
    }

    // ========================================================================
    // Clipboard Operations
    // ========================================================================
    // Note: Clipboard operations are now handled by the ContextMenuHandler trait
    // These methods delegate to that implementation

    fn copy_selected_key(&self, selected: &Option<String>) -> Option<String> {
        ContextMenuHandler::copy_selected_key(self, selected)
    }

    fn copy_selected_value(
        &self,
        selected: &Option<String>,
        cache: &mut LruCache<usize, Value>,
        loader: &mut LazyJsonFile,
    ) -> Option<String> {
        ContextMenuHandler::copy_selected_value(self, selected, cache, loader)
    }

    fn copy_selected_object(
        &self,
        selected: &Option<String>,
        cache: &mut LruCache<usize, Value>,
        loader: &mut LazyJsonFile,
    ) -> Option<String> {
        ContextMenuHandler::copy_selected_object(self, selected, cache, loader)
    }

    fn copy_selected_path(&self, selected: &Option<String>) -> Option<String> {
        ContextMenuHandler::copy_selected_path(self, selected)
    }
}
