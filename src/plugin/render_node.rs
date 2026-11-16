//! egui renderer for plugin UI trees.
//!
//! The node/event/output types live in `thoth-core`; this module re-exports
//! them and adds the one function that needs an `egui::Ui`.

pub use thoth_core::plugin::render_node::{RenderNode, UiEvent, UiNode, UiOutput};

use eframe::egui;

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
