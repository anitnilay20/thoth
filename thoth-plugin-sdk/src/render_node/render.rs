//! egui rendering for [`RenderNode`] trees.
//!
//! [`RenderNode::show`] walks the tree, delegating each leaf to the wrapped
//! component's own `Widget`/`show` and recursing through containers. Stateful
//! leaves (input, select) are rendered through `&mut`, so editing mutates the
//! node's own data in place — the host can keep the tree in state and read
//! values back out.
//!
//! Interaction results (clicks, changes) are currently dropped; event
//! propagation will be threaded through a host sink in a later step.

use super::{CustomWidget, RenderNode, UiEvent};

impl RenderNode {
    /// Render this node and its descendants into `ui`, collecting interaction
    /// events into `events` (which the host forwards to the plugin).
    pub fn show(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) {
        match self {
            // ── Containers (delegate to the layout components) ───────────────
            RenderNode::Row(r) => r.show(ui, events),
            RenderNode::Column(c) => c.show(ui, events),
            RenderNode::Scroll(s) => s.show(ui, events),
            RenderNode::Spacer(s) => s.show(ui),
            RenderNode::Split(s) => s.show(ui, events),
            RenderNode::Group(g) => g.show(ui, events),
            RenderNode::Collapsible(c) => c.show(ui, events),
            RenderNode::Footer(f) => f.show(ui, events),
            RenderNode::KeyValue(kv) => kv.show(ui, events),
            RenderNode::Colored(c) => c.show(ui, events),

            // ── Action widgets (emit "click") ────────────────────────────────
            RenderNode::Button(b) => {
                if ui.add(b.clone()).clicked() {
                    emit(events, &b.id, "click", String::new());
                }
            }
            RenderNode::IconButton(b) => {
                if ui.add(b.clone()).clicked() {
                    emit(events, &b.id, "click", String::new());
                }
            }
            RenderNode::Toggle(t) => {
                if ui.add(t.clone()).clicked() {
                    emit(events, &t.id, "change", (!t.enabled).to_string());
                }
            }

            // ── Display-only leaves ──────────────────────────────────────────
            RenderNode::Text(t) => {
                ui.add(t.clone());
            }
            RenderNode::Separator(s) => {
                ui.add(*s);
            }
            RenderNode::Badge(b) => {
                ui.add(b.clone());
            }
            RenderNode::Icon(i) => {
                ui.add(i.clone());
            }
            RenderNode::Link(l) => {
                ui.add(l.clone());
            }
            RenderNode::Progress(p) => {
                ui.add(*p);
            }
            RenderNode::Spinner(s) => {
                ui.add(*s);
            }
            RenderNode::Code(c) => {
                ui.add(c.clone());
            }
            RenderNode::Markdown(m) => {
                m.show(ui);
            }
            RenderNode::JsonTree(j) => {
                j.show(ui);
            }
            RenderNode::Table(t) => {
                t.show(ui, events);
            }
            RenderNode::Breadcrumbs(b) => {
                b.clone().show(ui);
            }
            RenderNode::SidebarHeader(h) => {
                h.show(ui);
            }

            // ── Input widgets (emit "change") ────────────────────────────────
            RenderNode::Input(i) => {
                if i.show(ui).inner {
                    emit(events, &i.id, "change", i.value.clone());
                }
            }
            RenderNode::Select(s) => {
                if let Some(v) = s.show(ui).inner {
                    emit(events, &s.id, "change", v);
                }
            }
            RenderNode::Checkbox(c) => {
                if c.show(ui).changed() {
                    emit(events, &c.id, "change", c.checked.to_string());
                }
            }
            RenderNode::Slider(s) => {
                if s.show(ui).changed() {
                    emit(events, &s.id, "change", s.value.to_string());
                }
            }
            RenderNode::NumberInput(n) => {
                if n.show(ui).changed() {
                    emit(events, &n.id, "change", n.value.to_string());
                }
            }
            RenderNode::Radio(r) => {
                if let Some(v) = r.show(ui) {
                    emit(events, &r.id, "change", v);
                }
            }
            RenderNode::MultiSelect(m) => {
                if m.show(ui) {
                    let value = serde_json::to_string(&m.value).unwrap_or_default();
                    emit(events, &m.id, "change", value);
                }
            }
            RenderNode::KeyValueList(k) => {
                if k.show(ui) {
                    let value = serde_json::to_string(&k.entries).unwrap_or_default();
                    emit(events, &k.id, "change", value);
                }
            }
            RenderNode::ButtonGroup(g) => {
                if let Some(i) = g.show(ui).inner {
                    emit(events, &g.id, "change", i.to_string());
                }
            }
            RenderNode::DataRow(d) => {
                let out = d.show(ui);
                if out.caret_clicked {
                    emit(events, &d.row_id, "toggle", String::new());
                } else if out.clicked {
                    emit(events, &d.row_id, "click", String::new());
                }
            }
            RenderNode::CodeEditor(c) => {
                if c.show(ui) {
                    emit(events, &c.id, "change", c.value.clone());
                }
            }
            RenderNode::List(l) => {
                if let Some(ev) = l.show(ui) {
                    match ev {
                        crate::components::ListEvent::ItemClicked(i) => {
                            emit(events, &l.id, "click", i.to_string());
                        }
                        crate::components::ListEvent::ActionClicked { item, action } => {
                            let value = serde_json::json!({ "item": item, "action": action })
                                .to_string();
                            emit(events, &l.id, "action", value);
                        }
                    }
                }
            }

            // ── Containers that recurse ──────────────────────────────────────
            RenderNode::Modal(m) => {
                if m.open {
                    let close_id = m.close_id.clone().unwrap_or_else(|| m.id.clone());
                    if m.show(ui, events) {
                        emit(events, &close_id, "click", String::new());
                    }
                }
            }
            RenderNode::Tabs(t) => {
                t.show(ui, events);
            }
            RenderNode::Card(c) => {
                c.show(ui, events);
            }

            // ── Escape hatch ─────────────────────────────────────────────────
            RenderNode::Custom(c) => c.show(ui),
        }
    }
}

/// Push an event onto the sink (skipping widgets that have no id assigned).
fn emit(events: &mut Vec<UiEvent>, id: &str, kind: &str, value: String) {
    if id.is_empty() {
        return;
    }
    events.push(UiEvent {
        id: id.to_string(),
        kind: kind.to_string(),
        value,
    });
}

impl RenderNode {
    /// Build a [`RenderNode::Custom`] from a draw closure — the UI-path escape
    /// hatch for arbitrary host widgets inside an otherwise-serializable tree.
    ///
    /// The closure must be `Send + 'static` (the tree is retainable and
    /// `Send + Sync`), so it can't borrow local state — capture by move or via
    /// shared handles.
    pub fn custom(draw: impl FnMut(&mut egui::Ui) + Send + 'static) -> Self {
        RenderNode::Custom(CustomWidget::new(draw))
    }
}
