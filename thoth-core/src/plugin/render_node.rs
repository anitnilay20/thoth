//! Host-side plugin UI rendering.
//!
//! Plugins describe their UI as a [`thoth_plugin_sdk::render_node::RenderNode`]
//! tree, serialized to JSON across the WIT boundary. The host deserializes that
//! JSON into [`RenderNode`] and renders it via the SDK's renderer, collecting
//! interaction events to route back to the plugin's `handle-event`.
//!
//! This module is a thin adapter: the DSL, components, and rendering all live
//! in the `thoth-plugin-sdk` crate (single source of truth for both plugin
//! authors and the host).

use eframe::egui;

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

/// Render a plugin [`RenderNode`] tree, collecting interaction events.
///
/// SDK events (`id`/`kind`/`value`) are mapped onto host [`UiEvent`]s
/// (`widget_id`/`kind`/`value`) for the existing event-dispatch path.
pub fn render_ui_node(ui: &mut egui::Ui, node: &mut RenderNode, events: &mut Vec<UiEvent>) {
    let mut sdk_events = Vec::new();
    node.show(ui, &mut sdk_events);
    events.extend(sdk_events.into_iter().map(|e| UiEvent {
        widget_id: e.id,
        kind: e.kind,
        value: e.value,
    }));
}
