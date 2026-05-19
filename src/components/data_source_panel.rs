use eframe::egui;

use crate::{
    components::traits::StatefulComponent,
    plugin::{
        render_node::{TextSize, UiNode, UiOutput, render_ui_node},
        wasm_data_source::{ConsentRequest, WasmDataSourceLoader},
    },
};

pub struct DataSourcePanel {
    loader: Option<WasmDataSourceLoader>,
    cached_output: Option<UiOutput>,
    /// Tracks the current URL value from the plugin's "url" widget so we can
    /// populate `QueryResult::display_url` when "send" is clicked.
    last_url: String,
}

pub enum DataSourcePanelEvent {
    QueryResult { json: String, display_url: String },
    ConsentNeeded(ConsentRequest),
    Error(String),
}

pub struct DataSourcePanelProps {}

impl DataSourcePanel {
    pub fn new() -> Self {
        Self {
            loader: None,
            cached_output: None,
            last_url: String::new(),
        }
    }

    pub fn set_loader(&mut self, loader: WasmDataSourceLoader) {
        self.loader = Some(loader);
        self.cached_output = None; // force re-render on next frame
        self.last_url = String::new();
    }

    pub fn has_loader(&self) -> bool {
        self.loader.is_some()
    }
}

impl Default for DataSourcePanel {
    fn default() -> Self {
        Self::new()
    }
}

impl StatefulComponent for DataSourcePanel {
    type Props<'a> = DataSourcePanelProps;
    type Output = Vec<DataSourcePanelEvent>;

    fn render(
        &mut self,
        ui: &mut egui::Ui,
        _props: DataSourcePanelProps,
    ) -> Vec<DataSourcePanelEvent> {
        let mut panel_events: Vec<DataSourcePanelEvent> = Vec::new();

        if self.loader.is_none() {
            ui.centered_and_justified(|ui| {
                ui.label("No data source loaded.");
            });
            return panel_events;
        }

        // Populate cache on first render (borrow ends before we access cached_output below).
        if self.cached_output.is_none() {
            let result = self.loader.as_mut().unwrap().render_ui();
            self.cached_output = Some(result.unwrap_or_else(|e| error_output(&e.to_string())));
        }

        // Deserialise the cached node tree for rendering.
        let node: UiNode = serde_json::from_str(&self.cached_output.as_ref().unwrap().node_json)
            .unwrap_or(UiNode::Text {
                value: "UI parse error".into(),
                size: TextSize::Md,
                muted: false,
            });

        // Render the tree and collect widget events for this frame.
        let mut ui_events = Vec::new();
        render_ui_node(ui, &node, &mut ui_events);

        for evt in ui_events {
            // Track URL changes so QueryResult has a readable display_url.
            if evt.widget_id == "url" && evt.kind == "change"
                && let Ok(url) = serde_json::from_str::<String>(&evt.value) {
                    self.last_url = url;
                }

            // Forward event to plugin; update cached tree on success.
            match self.loader.as_mut().unwrap().handle_event(evt.clone()) {
                Ok(new_output) => {
                    *self.cached_output.as_mut().unwrap() = new_output;
                }
                Err(e) => panel_events.push(DataSourcePanelEvent::Error(e.to_string())),
            }

            // After "send": the HTTP request is in-flight asynchronously; drain any
            // consent requests and wait for the plugin's http-response event to deliver
            // the QueryResult — do not call query() here as the response is not yet cached.
            if evt.widget_id == "send" && evt.kind == "click" {
                for cr in self.loader.as_mut().unwrap().drain_consent_requests() {
                    panel_events.push(DataSourcePanelEvent::ConsentNeeded(cr));
                }
            }
        }

        panel_events
    }
}

/// Returns a minimal `UiOutput` displaying an error message.
/// Used as a fallback when `render_ui()` or JSON parsing fails.
fn error_output(message: &str) -> UiOutput {
    let node = serde_json::json!({
        "type": "text",
        "value": format!("Plugin UI error: {message}"),
        "muted": true
    });
    UiOutput {
        node_json: node.to_string(),
        height_hint: 0,
    }
}
