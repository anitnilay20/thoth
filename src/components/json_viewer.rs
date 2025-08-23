use crate::components::theme::{TextPalette, TextToken, row_fill};
use crate::helpers::LruCache;
use crate::{
    file::lazy_loader::{FileType, LazyJsonFile, load_file_auto},
    helpers::{format_simple_kv, preview_value},
};
use eframe::egui;
use egui::Ui;
use serde_json::Value;
use std::collections::HashSet;

#[derive(Default)]
pub struct JsonViewer {
    // Data source
    loader: Option<LazyJsonFile>,
    visible_roots: Option<Vec<usize>>, // filter for root indices (e.g. search results)

    // UI state
    expanded: HashSet<String>, // paths like "0", "0.user", "0.items[2]"
    rows: Vec<JsonRow>,        // flattened render list

    // (Optional) tiny cache to avoid re-parsing when toggling the same item
    cache: LruCache<usize, Value>, // cache top-level items only; nested gets are cheap to rewalk
}

#[derive(Clone)]
struct JsonRow {
    path: String, // stable key
    indent: usize,
    is_expandable: bool,
    is_expanded: bool,
    display_text: String,
    text_token: (TextToken, Option<TextToken>),
}

impl JsonViewer {
    pub fn new() -> Self {
        Self {
            loader: None,
            expanded: HashSet::new(),
            rows: Vec::new(),
            cache: LruCache::new(32),
            visible_roots: None,
        }
    }

    /// Open a file lazily. This does NOT parse the whole file.
    pub fn open(&mut self, path: &std::path::Path, file_type: &mut FileType) -> anyhow::Result<()> {
        let resp = load_file_auto(path)?;
        self.loader = Some(resp.1);
        *file_type = resp.0.into();

        self.expanded.clear();
        self.rows.clear();
        self.rebuild_root_rows();
        Ok(())
    }

    fn rebuild_root_rows(&mut self) {
        self.rows.clear();

        let Some(loader) = self.loader.as_ref() else {
            return;
        };

        // Which root indices to render: filtered (search) or all
        let total = loader.len();
        let indices: Vec<usize> = if let Some(list) = self.visible_roots.as_ref() {
            list.clone()
        } else {
            (0..total).collect()
        };

        for i in indices {
            let path = i.to_string(); // "0", "1", ...
            let is_expanded = self.expanded.contains(&path);

            // Lightweight stub until expanded
            let display_text = if is_expanded {
                // We'll append children using build_rows_from_value and then a closing brace
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
                // Load root value (from cache or loader), then expand children
                if let Some(mut value) = self.cache.get(&i).cloned() {
                    self.build_rows_from_value(&mut value, &path, 1);
                } else if let Some(loader_mut) = self.loader.as_mut() {
                    if let Ok(v) = loader_mut.get(i) {
                        let mut v_owned = v;
                        self.cache.put(i, v_owned.clone());
                        self.build_rows_from_value(&mut v_owned, &path, 1);
                    }
                }

                // Closing row (visual balance for "{")
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

    /// Depth-first row builder; only called for expanded nodes.
    fn build_rows_from_value(&mut self, value: &mut Value, path: &str, indent: usize) {
        match value {
            Value::Object(map) => {
                for (key, val) in map.iter_mut() {
                    let new_path = format!("{}.{key}", path);
                    let is_expandable = matches!(val, Value::Object(_) | Value::Array(_));
                    let is_expanded = self.expanded.contains(&new_path);

                    let display_text = if is_expandable {
                        format!("\"{key}\": {}", if is_expanded { "{" } else { "{}" })
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
                for (idx, val) in arr.iter_mut().enumerate() {
                    let new_path = format!("{path}[{idx}]");
                    let is_expandable = matches!(val, Value::Object(_) | Value::Array(_));
                    let is_expanded = self.expanded.contains(&new_path);

                    let display_text = if is_expandable {
                        format!("[{idx}]: {}", if is_expanded { "[" } else { "[]" })
                    } else {
                        format!("[{idx}]: {}", preview_value(val))
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
            // Primitives are shown only when the parent is expanded
            _ => {
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

    pub fn ui(&mut self, ui: &mut Ui) {
        let row_count = self.rows.len();
        let row_height = 20.0;

        // collect actions, don't mutate `self` inside the paint closure
        let mut toggles: Vec<String> = Vec::new();

        egui::ScrollArea::both()
            .auto_shrink([false, false])
            .show_rows(ui, row_height, row_count, |ui, row_range| {
                let visuals = ui.visuals();
                let palette = TextPalette::for_visuals(visuals);

                for row_index in row_range.clone() {
                    if let Some(row) = self.rows.get(row_index) {
                        // Copy small bits we need (avoids borrowing self later)
                        let indent = row.indent;
                        let is_expandable = row.is_expandable;
                        let is_expanded = row.is_expanded;
                        let path = row.path.clone();
                        let display = row.display_text.clone();
                        let mut parts = display.splitn(2, ':');
                        let display1 = parts.next().unwrap_or("");
                        let display2 = parts.next().unwrap_or("");
                        let is_key_display = !display2.is_empty() && row.text_token.1.is_some();

                        egui::Frame::new()
                            .fill(row_fill(row_index, ui))
                            .show(ui, |ui| {
                                ui.set_min_width(ui.available_width());
                                ui.horizontal(|ui| {
                                    ui.add_space(indent as f32 * 12.0);

                                    if is_expandable {
                                        let toggle_icon = if is_expanded { " - " } else { "+" };
                                        if ui.selectable_label(false, toggle_icon).clicked() {
                                            toggles.push(path.clone());
                                        }
                                    } else {
                                        ui.add_space(23.0);
                                    }

                                    ui.add(egui::Label::new(
                                        egui::RichText::new(format!(
                                            "{}{}",
                                            display1,
                                            if is_key_display { ":" } else { "" }
                                        ))
                                        .monospace()
                                        .color(palette.color(row.text_token.0)),
                                    ));

                                    if is_key_display {
                                        ui.add(egui::Label::new(
                                            egui::RichText::new(display2)
                                                .monospace()
                                                .color(palette.color(row.text_token.1.unwrap())),
                                        ));
                                    }
                                });
                            });
                    }
                }
            });

        // Now mutate state once, outside the paint closure
        if !toggles.is_empty() {
            for path in toggles {
                if !self.expanded.insert(path.clone()) {
                    // was already present; remove to toggle closed
                    self.expanded.remove(&path);
                }
            }
            self.rebuild_root_rows();
        }
    }

    /// Limit the viewer to a subset of root indices (search hits). Pass None to clear.
    pub fn set_root_filter(&mut self, filter: Option<Vec<usize>>) {
        self.visible_roots = filter.map(|mut v| {
            v.sort_unstable();
            v.dedup();
            v
        });
        self.rebuild_root_rows();
    }

    /// For UI badges etc.
    pub fn current_filter_len(&self) -> Option<usize> {
        self.visible_roots.as_ref().map(|v| v.len())
    }
}
