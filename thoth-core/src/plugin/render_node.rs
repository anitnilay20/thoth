//! Host-side plugin UI node types.
//!
//! Plugins describe their UI as a [`thoth_plugin_sdk::render_node::RenderNode`]
//! tree, serialized to JSON across the WIT boundary. The types here are the
//! display-independent half; the egui renderer lives in `thoth-ui`.

pub use thoth_plugin_sdk::render_node::RenderNode;

/// Alias kept for existing call sites — the plugin UI node type.
pub type UiNode = RenderNode;

/// An interaction event emitted by a plugin widget.
///
/// Field naming matches the WIT `ui-event` record (`widget-id`); it is the
/// host-facing counterpart to the SDK's [`thoth_plugin_sdk::render_node::UiEvent`]
/// (which names the field `id`).
#[derive(Debug, Clone)]
pub struct UiEvent {
    /// Id of the widget that emitted the event.
    pub widget_id: String,
    /// Interaction class — `"click"`, `"change"`, `"toggle"`, `"action"`.
    pub kind: String,
    /// String payload (new value, clicked index, JSON, …).
    pub value: String,
}

/// A plugin's UI render output (WIT `ui-output`).
#[derive(Debug, Clone)]
pub struct UiOutput {
    /// JSON-encoded [`RenderNode`] tree.
    pub node_json: String,
    /// Height hint in points; `0` means auto-size.
    pub height_hint: u32,
}
