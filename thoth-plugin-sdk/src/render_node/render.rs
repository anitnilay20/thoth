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

use super::{CustomWidget, RenderNode};

impl RenderNode {
    /// Render this node and its descendants into `ui`.
    pub fn show(&mut self, ui: &mut egui::Ui) {
        match self {
            // ── Containers ───────────────────────────────────────────────────
            RenderNode::Row { children, gap } => {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = *gap;
                    for child in children {
                        child.show(ui);
                    }
                });
            }
            RenderNode::Column { children, gap } => {
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = *gap;
                    for child in children {
                        child.show(ui);
                    }
                });
            }
            RenderNode::Scroll { child, max_height } => {
                let mut area = egui::ScrollArea::vertical();
                if let Some(h) = max_height {
                    area = area.max_height(*h);
                }
                area.show(ui, |ui| child.show(ui));
            }
            RenderNode::Spacer { size } => {
                ui.add_space(*size);
            }

            // ── Leaf widgets ─────────────────────────────────────────────────
            // Widget-based leaves are cheap owned clones (the node retains its
            // data; `ui.add` consumes the clone).
            RenderNode::Button(b) => {
                ui.add(b.clone());
            }
            RenderNode::Text(t) => {
                ui.add(t.clone());
            }
            RenderNode::IconButton(b) => {
                ui.add(b.clone());
            }
            RenderNode::Toggle(t) => {
                ui.add(t.clone());
            }
            RenderNode::Separator(s) => {
                ui.add(*s);
            }
            RenderNode::Breadcrumbs(b) => {
                b.clone().show(ui);
            }
            RenderNode::ButtonGroup(g) => {
                g.show(ui);
            }
            // Stateful leaves render through `&mut`, mutating the node's data.
            RenderNode::Input(i) => {
                i.show(ui);
            }
            RenderNode::Select(s) => {
                s.show(ui);
            }
            RenderNode::DataRow(d) => {
                d.show(ui);
            }
            RenderNode::Table(t) => {
                t.show(ui);
            }
            RenderNode::JsonTree(j) => {
                j.show(ui);
            }
            RenderNode::SidebarHeader(h) => {
                h.show(ui);
            }

            // ── Escape hatch ─────────────────────────────────────────────────
            RenderNode::Custom(c) => c.show(ui),
        }
    }
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
