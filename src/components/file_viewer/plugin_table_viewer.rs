use std::collections::HashMap;

use eframe::egui;
use serde_json::Value;

use crate::components::file_viewer::viewer_trait::FileFormatViewer;
use crate::file::loaders::FileType;
use crate::helpers::LruCache;
use crate::plugin::wasm_file_viewer_loader::DisplayMode;
use thoth_plugin_sdk::components::TableView;
use thoth_plugin_sdk::render_node::RenderNode;

pub struct PluginTableViewer {
    headers: Vec<String>,
    visible_indices: Vec<usize>,
    display_mode: DisplayMode,
    render_cache: HashMap<usize, String>,
}

impl PluginTableViewer {
    pub fn new() -> Self {
        Self {
            headers: Vec::new(),
            visible_indices: Vec::new(),
            display_mode: DisplayMode::Table,
            render_cache: HashMap::new(),
        }
    }
}

impl Default for PluginTableViewer {
    fn default() -> Self {
        Self::new()
    }
}

impl FileFormatViewer for PluginTableViewer {
    fn reset(&mut self) {
        self.headers.clear();
        self.visible_indices.clear();
        self.display_mode = DisplayMode::Table;
        self.render_cache.clear();
    }

    fn rebuild_view(
        &mut self,
        visible_roots: &Option<Vec<usize>>,
        _cache: &mut LruCache<usize, Value>,
        loader: &mut FileType,
        total_len: usize,
    ) {
        if self.headers.is_empty() {
            self.display_mode = loader.preferred_display();
            if let Some(h) = loader.column_headers() {
                self.headers = h;
            } else if total_len > 0 {
                // Plugin didn't provide headers — derive them from the keys of
                // the first record so the table has something to render.
                if let Ok(first) = loader.get(0)
                    && let Some(obj) = first.as_object()
                {
                    let mut keys: Vec<String> = obj.keys().cloned().collect();
                    keys.sort(); // deterministic column order
                    self.headers = keys;
                }
            }
        }

        let new_indices: Vec<usize> = match visible_roots {
            Some(roots) => roots.clone(),
            None => (0..total_len).collect(),
        };

        if new_indices != self.visible_indices {
            self.render_cache.clear();
            self.visible_indices = new_indices;
        }
    }

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        _selected: &mut Option<String>,
        cache: &mut LruCache<usize, Value>,
        loader: &mut FileType,
        _should_scroll_to_selection: &mut bool,
        _is_search_navigation: bool,
        _syntax_highlighting: bool,
    ) -> bool {
        let display_mode = self.display_mode;
        let headers = self.headers.clone();
        let headers_for_closure = headers.clone();
        let indices = self.visible_indices.clone();
        let num_rows = indices.len();
        let render_cache = &mut self.render_cache;

        TableView::show_rows(ui, &headers, num_rows, None, &mut Vec::new(), move |i| {
            let idx = indices[i];

            match display_mode {
                DisplayMode::Table => {
                    let cached = cache.get(&idx).cloned();
                    let record = match cached {
                        Some(v) => Some(v),
                        None => loader.get(idx).ok().inspect(|v| {
                            cache.put(idx, v.clone());
                        }),
                    };
                    headers_for_closure
                        .iter()
                        .map(|h| match record.as_ref().and_then(|v| v.get(h)) {
                            // Colour each cell by its JSON type, like the tree.
                            Some(v) => RenderNode::json_cell(v),
                            None => RenderNode::text(""),
                        })
                        .collect()
                }

                DisplayMode::Custom => {
                    if let std::collections::hash_map::Entry::Vacant(e) = render_cache.entry(idx) {
                        let cached = cache.get(&idx).cloned();
                        let record = match cached {
                            Some(v) => Some(v),
                            None => loader.get(idx).ok().inspect(|v| {
                                cache.put(idx, v.clone());
                            }),
                        };
                        if let Some(r) = record {
                            let json = serde_json::to_string(&r).unwrap_or_default();
                            if let Some(node_json) = loader.render_record(&json) {
                                e.insert(node_json);
                            }
                        }
                    }

                    if let Some(node_json) = render_cache.get(&idx) {
                        match serde_json::from_str::<RenderNode>(node_json) {
                            Ok(RenderNode::Row(row)) => row.children,
                            Ok(other) => vec![other],
                            Err(_) => vec![RenderNode::text("—")],
                        }
                    } else {
                        headers_for_closure
                            .iter()
                            .map(|_| RenderNode::text("—"))
                            .collect()
                    }
                }
            }
        });

        false
    }
}
