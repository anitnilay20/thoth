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
            RenderNode::Split {
                children,
                gap,
                widths,
                separator,
            } => {
                let n = children.len();
                if n == 0 {
                    return;
                }
                let total_gap = *gap * (n.saturating_sub(1)) as f32;
                let avail = (ui.available_width() - total_gap).max(0.0);
                // Resolve per-column widths from relative weights (equal if absent).
                let weights: Vec<f32> = if widths.len() == n {
                    widths.clone()
                } else {
                    vec![1.0; n]
                };
                let sum: f32 = weights.iter().sum::<f32>().max(1.0);
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = *gap;
                    for (i, child) in children.iter_mut().enumerate() {
                        let w = avail * (weights[i] / sum);
                        ui.allocate_ui_with_layout(
                            egui::vec2(w, ui.available_height()),
                            egui::Layout::top_down(egui::Align::Min),
                            |ui| child.show(ui),
                        );
                        if *separator && i + 1 < n {
                            ui.separator();
                        }
                    }
                });
            }
            RenderNode::Group { label, children } => {
                egui::CollapsingHeader::new(label.as_str())
                    .default_open(true)
                    .show(ui, |ui| {
                        for child in children {
                            child.show(ui);
                        }
                    });
            }
            RenderNode::Collapsible { label, children } => {
                egui::CollapsingHeader::new(label.as_str())
                    .default_open(false)
                    .show(ui, |ui| {
                        for child in children {
                            child.show(ui);
                        }
                    });
            }
            RenderNode::Footer {
                children,
                gap,
                padding,
            } => {
                egui::Frame::new()
                    .inner_margin(egui::Margin::same(*padding as i8))
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = *gap;
                        for child in children {
                            child.show(ui);
                        }
                    });
            }
            RenderNode::KeyValue { key, value } => {
                ui.horizontal(|ui| {
                    let muted = ui.visuals().weak_text_color();
                    ui.label(egui::RichText::new(format!("{key}: ")).color(muted));
                    value.show(ui);
                });
            }
            RenderNode::Colored { color, child } => {
                let resolved = crate::theme::parse_hex_color(color);
                ui.scope(|ui| {
                    if let Some(c) = resolved {
                        ui.visuals_mut().override_text_color = Some(c);
                    }
                    child.show(ui);
                });
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
            RenderNode::Modal(m) => {
                m.as_ref().clone().show(ui);
            }
            RenderNode::Checkbox(c) => {
                c.show(ui);
            }
            RenderNode::Slider(s) => {
                s.show(ui);
            }
            RenderNode::NumberInput(n) => {
                n.show(ui);
            }
            RenderNode::Radio(r) => {
                r.show(ui);
            }
            RenderNode::MultiSelect(m) => {
                m.show(ui);
            }
            RenderNode::KeyValueList(k) => {
                k.show(ui);
            }
            RenderNode::Code(c) => {
                ui.add(c.clone());
            }
            RenderNode::Markdown(m) => {
                m.show(ui);
            }
            RenderNode::CodeEditor(c) => {
                c.show(ui);
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
