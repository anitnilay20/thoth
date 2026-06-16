use eframe::egui;

use thoth_plugin_sdk::components::Separator;

use crate::components::{
    icon_button::{IconButton, IconButtonProps},
    traits::StatelessComponent,
};

// ── Props / Output ────────────────────────────────────────────────────────────

pub struct ModalProps<'a> {
    /// Unique stable id for this modal (used as egui Id salt).
    pub id: &'a str,
    /// Title shown in the header bar.
    pub title: &'a str,
    /// Pre-rendered body content as an egui closure result — pass via
    /// `Modal::show(ui, props, |ui| { … })` using the free function below.
    pub body_response: Option<egui::Response>,
}

pub struct ModalOutput {
    /// True if the user requested the modal be closed
    /// (backdrop click, × button, or Escape key).
    pub close_requested: bool,
}

/// Size hint for a modal: fraction of the screen in each axis.
#[derive(Clone, Copy)]
pub struct ModalSize {
    pub width_pct: f32,
    pub height_pct: f32,
}

// ── Component ────────────────────────────────────────────────────────────────

pub struct Modal;

impl Modal {
    /// Convenience entry-point: shows the modal and renders `body` inside it.
    /// Returns `ModalOutput` with `close_requested` set when the user wants to
    /// dismiss the dialog.
    pub fn show<F>(ui: &mut egui::Ui, id: &str, title: &str, body: F) -> ModalOutput
    where
        F: FnOnce(&mut egui::Ui),
    {
        Self::show_sized(ui, id, title, None, body)
    }

    /// Like `show` but accepts an optional `ModalSize` to control the window dimensions
    /// as a fraction of the screen.
    pub fn show_sized<F>(
        ui: &mut egui::Ui,
        id: &str,
        title: &str,
        size: Option<ModalSize>,
        body: F,
    ) -> ModalOutput
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
        let backdrop_id = egui::Id::new(("modal_backdrop", id));
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
        let modal_w = size.map(|s| screen.width() * s.width_pct);
        let modal_h = size.map(|s| screen.height() * s.height_pct);

        let win = egui::Window::new(format!("__modal_{}", id))
            .order(egui::Order::Foreground)
            .collapsible(false)
            .resizable(false)
            .movable(false)
            .title_bar(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]);

        let win = if let Some(w) = modal_w {
            win.min_width(w)
                .max_width(w)
                .fixed_size([w, modal_h.unwrap_or(0.0)])
        } else {
            win.min_width(320.0).max_width(480.0)
        };

        win.show(&ctx, |ui| {
            // ── Header ────────────────────────────────────────────────
            ui.horizontal(|ui| {
                ui.heading(title);
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let close_btn = IconButton::render(
                        ui,
                        IconButtonProps {
                            icon: egui_phosphor::regular::X,
                            frame: true,
                            tooltip: None,
                            badge_color: None,
                            size: None,
                            disabled: false,
                            icon_size: None,
                            selected: false,
                        },
                    );

                    if close_btn.clicked {
                        close_requested = true;
                    }
                });
            });
            ui.add(Separator::with_margins(0.0, 4.0));

            // ── Body ──────────────────────────────────────────────────
            if let Some(h) = modal_h {
                let header_overhead = 40.0;
                egui::ScrollArea::vertical()
                    .max_height(h - header_overhead)
                    .show(ui, |ui| body(ui));
            } else {
                body(ui);
            }
        });

        ModalOutput { close_requested }
    }
}

// Implement the trait for cases where a caller wants the trait-based API.
// The Props type carries the body as a boxed closure.
pub struct ModalPropsBoxed<'a> {
    pub id: &'a str,
    pub title: &'a str,
    pub body: Box<dyn FnOnce(&mut egui::Ui) + 'a>,
    pub size: Option<ModalSize>,
}

impl StatelessComponent for Modal {
    type Props<'a> = ModalPropsBoxed<'a>;
    type Output = ModalOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        Modal::show_sized(ui, props.id, props.title, props.size, props.body)
    }
}
