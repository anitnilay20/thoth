use egui::Widget;

use crate::components::{IconButton, Separator};
use crate::render_node::UiEvent;

use super::Modal;

impl Modal {
    /// Render the modal overlay, drawing its [`children`](Modal::children) (the
    /// DSL path) and collecting their events.
    ///
    /// Returns `true` when the user requested to close it this frame (Escape,
    /// backdrop click, or the header close button). The caller (or the
    /// `RenderNode` renderer) turns that into the close event.
    pub fn show(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) -> bool {
        let children = &mut self.children;
        Self::frame(&self.id, &self.title, self.width_pct, self.height_pct, ui, |ui| {
            for child in children {
                child.show(ui, events);
            }
        })
    }

    /// Render the modal overlay, drawing its content with the `body` closure
    /// (the flexible UI path — the closure may borrow local state).
    ///
    /// Returns `true` when the user requested to close it this frame.
    pub fn show_with<F>(&self, ui: &mut egui::Ui, body: F) -> bool
    where
        F: FnOnce(&mut egui::Ui),
    {
        Self::frame(&self.id, &self.title, self.width_pct, self.height_pct, ui, body)
    }

    /// Draw the backdrop + centered window chrome and run `body` for content.
    fn frame<F: FnOnce(&mut egui::Ui)>(
        id: &str,
        title: &str,
        width_pct: Option<f32>,
        height_pct: Option<f32>,
        ui: &mut egui::Ui,
        body: F,
    ) -> bool {
        let ctx = ui.ctx().clone();
        let mut close_requested = ctx.input(|i| i.key_pressed(egui::Key::Escape));

        let screen = ctx.content_rect();

        // ── Backdrop ─────────────────────────────────────────────────────────
        let backdrop = egui::Area::new(egui::Id::new(("modal_backdrop", id)))
            .order(egui::Order::Middle)
            .fixed_pos(screen.min)
            .interactable(true)
            .show(&ctx, |ui| {
                ui.painter()
                    .rect_filled(screen, 0.0, egui::Color32::from_black_alpha(140));
                ui.allocate_rect(screen, egui::Sense::click())
            })
            .inner;
        if backdrop.clicked() {
            close_requested = true;
        }

        // ── Window (sized as a fraction of the viewport) ─────────────────────
        let win = egui::Window::new(format!("__modal_{id}"))
            .order(egui::Order::Foreground)
            .collapsible(false)
            .resizable(false)
            .movable(false)
            .title_bar(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]);

        let w = width_pct.map(|p| screen.width() * p.clamp(0.0, 1.0));
        let h = height_pct.map(|p| screen.height() * p.clamp(0.0, 1.0));
        let win = match (w, h) {
            (Some(w), Some(h)) => win.fixed_size([w, h]),
            (Some(w), None) => win.min_width(w).max_width(w),
            (None, _) => win.min_width(320.0).max_width(480.0),
        };

        win.show(&ctx, |ui| {
            ui.horizontal(|ui| {
                ui.heading(title);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if IconButton::builder()
                        .icon(egui_phosphor::regular::X)
                        .frame(true)
                        .build()
                        .ui(ui)
                        .clicked()
                    {
                        close_requested = true;
                    }
                });
            });
            ui.add(Separator::with_margins(0.0, 4.0));

            if let Some(h) = h {
                let header_overhead = 40.0;
                egui::ScrollArea::vertical()
                    .max_height((h - header_overhead).max(0.0))
                    .show(ui, body);
            } else {
                body(ui);
            }
        });

        close_requested
    }
}
