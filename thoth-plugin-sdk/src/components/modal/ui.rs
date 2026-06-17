use egui::Widget;

use crate::components::{IconButton, Separator};

use super::Modal;

impl Modal {
    /// Render the modal, drawing its [`body`](Modal::body) `RenderNode` (the
    /// DSL path).
    ///
    /// Returns `true` when the user requested to close it this frame (Escape,
    /// backdrop click, or the header close button). The caller owns visibility.
    /// For arbitrary live widgets that aren't expressible as a `RenderNode`,
    /// use [`Modal::show_with`].
    pub fn show(mut self, ui: &mut egui::Ui, events: &mut Vec<crate::render_node::UiEvent>) -> bool {
        let mut body = self.body.take();
        self.show_with(ui, |ui| {
            if let Some(node) = &mut body {
                node.show(ui, events);
            }
        })
    }

    /// Render the modal, drawing its content with the `body` closure (the
    /// flexible UI path — the closure may borrow local state).
    ///
    /// Returns `true` when the user requested to close it this frame — by
    /// pressing Escape, clicking the backdrop, or clicking the header's close
    /// button. The caller owns visibility: hide the modal when this returns
    /// `true`. When [`Modal::height`] is set the body is wrapped in a vertical
    /// scroll area.
    pub fn show_with<F>(self, ui: &mut egui::Ui, body: F) -> bool
    where
        F: FnOnce(&mut egui::Ui),
    {
        let ctx = ui.ctx().clone();
        let mut close_requested = false;

        // ── Escape key ───────────────────────────────────────────────────────
        if ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            close_requested = true;
        }

        // ── Backdrop ─────────────────────────────────────────────────────────
        let backdrop_id = egui::Id::new(("modal_backdrop", self.id.clone()));
        let screen = ctx.content_rect();

        let backdrop_resp = egui::Area::new(backdrop_id)
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .interactable(true)
            .show(&ctx, |ui| {
                ui.painter()
                    .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(140));
                ui.allocate_rect(screen, egui::Sense::click())
            })
            .inner;

        if backdrop_resp.clicked() {
            close_requested = true;
        }

        // ── Modal window ─────────────────────────────────────────────────────
        let modal_w = self.width;
        let modal_h = self.height;

        let win = egui::Window::new(format!("__modal_{}", self.id))
            .order(egui::Order::Foreground)
            .collapsible(false)
            .resizable(false)
            .movable(false)
            .title_bar(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]);

        let win = match (modal_w, modal_h) {
            // Both dimensions fixed.
            (Some(w), Some(h)) => win.fixed_size([w, h]),
            // Fixed width, auto height.
            (Some(w), None) => win.min_width(w).max_width(w),
            // Auto width (default 320–480), with or without a fixed height.
            (None, _) => win.min_width(320.0).max_width(480.0),
        };

        win.show(&ctx, |ui| {
            // ── Header ────────────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.heading(self.title);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let close_btn = IconButton::builder()
                        .icon(egui_phosphor::regular::X)
                        .frame(true)
                        .build()
                        .ui(ui);

                    if close_btn.clicked() {
                        close_requested = true;
                    }
                });
            });
            ui.add(Separator::with_margins(0.0, 4.0));

            // ── Body ──────────────────────────────────────────────────
            if let Some(h) = modal_h {
                // Approximate the header (title + separator) height so the body
                // scroll area fits within the fixed modal height. TODO: measure
                // the header rect instead of assuming ~40px.
                let header_overhead = 40.0;
                egui::ScrollArea::vertical()
                    .max_height(h - header_overhead)
                    .show(ui, |ui| body(ui));
            } else {
                body(ui);
            }
        });

        close_requested
    }
}
