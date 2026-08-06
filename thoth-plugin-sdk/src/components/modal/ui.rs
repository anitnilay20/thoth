use egui::Widget;

use crate::components::{IconButton, Typography, TypographyVariant};
use crate::render_node::UiEvent;
use crate::theme::ThemeColors;

use super::Modal;

/// A modal's action bar. Boxed so [`Modal::frame`] can take it as one optional
/// argument regardless of what the caller closes over.
type Footer<'a> = Box<dyn FnOnce(&mut egui::Ui) + 'a>;

/// Header eyebrow type size — design `.grouplabel{font-size:10px}`.
const EYEBROW_FONT: f32 = 10.0;

/// The header and sizing configuration lifted off a [`Modal`], so `frame` takes
/// one argument instead of eight. Borrowed field-wise, which lets `show` hand it
/// the chrome while still mutably borrowing `children`.
struct Chrome<'a> {
    id: &'a str,
    title: &'a str,
    eyebrow: Option<&'a str>,
    eyebrow_color: Option<&'a str>,
    subtitle: Option<&'a str>,
    glyph: Option<&'a str>,
    glyph_color: Option<&'a str>,
    glyph_tile: bool,
    dismissible: bool,
    width: Option<f32>,
    width_pct: Option<f32>,
    height_pct: Option<f32>,
}

impl Modal {
    /// Borrow this modal's header/sizing config for [`Modal::frame`].
    fn chrome(&self) -> Chrome<'_> {
        Chrome {
            id: &self.id,
            title: &self.title,
            eyebrow: self.eyebrow.as_deref(),
            eyebrow_color: self.eyebrow_color.as_deref(),
            subtitle: self.subtitle.as_deref(),
            glyph: self.glyph.as_deref(),
            glyph_color: self.glyph_color.as_deref(),
            glyph_tile: self.glyph_tile,
            dismissible: self.dismissible,
            width: self.width,
            width_pct: self.width_pct,
            height_pct: self.height_pct,
        }
    }
    /// Render the modal overlay, drawing its [`children`](Modal::children) (the
    /// DSL path) and collecting their events.
    ///
    /// Returns `true` when the user requested to close it this frame (Escape,
    /// backdrop click, or the header close button). The caller (or the
    /// `RenderNode` renderer) turns that into the close event.
    pub fn show(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) -> bool {
        // Field-wise borrows: the chrome reads the header fields while `children`
        // is borrowed mutably for the body.
        let Modal {
            id,
            title,
            eyebrow,
            eyebrow_color,
            subtitle,
            glyph,
            glyph_color,
            glyph_tile,
            dismissible,
            width,
            width_pct,
            height_pct,
            children,
            footer,
            ..
        } = self;
        let chrome = Chrome {
            id,
            title,
            eyebrow: eyebrow.as_deref(),
            eyebrow_color: eyebrow_color.as_deref(),
            subtitle: subtitle.as_deref(),
            glyph: glyph.as_deref(),
            glyph_color: glyph_color.as_deref(),
            glyph_tile: *glyph_tile,
            dismissible: *dismissible,
            width: *width,
            width_pct: *width_pct,
            height_pct: *height_pct,
        };
        // `events` is borrowed by both closures, so the footer collects into its
        // own buffer and they're merged after the frame returns.
        let mut footer_events = Vec::new();
        let footer_bar: Option<Footer<'_>> = if footer.is_empty() {
            None
        } else {
            let sink = &mut footer_events;
            Some(Box::new(move |ui: &mut egui::Ui| {
                // Right-to-left, so declaration order reads rightmost-first —
                // the same contract as `show_with_footer`.
                for node in footer {
                    node.show(ui, sink);
                }
            }))
        };
        let closed = Self::frame(
            chrome,
            ui,
            |ui| {
                for child in children {
                    child.show(ui, events);
                }
            },
            footer_bar,
        );
        events.append(&mut footer_events);
        closed
    }

    /// Render the modal overlay, drawing its content with the `body` closure
    /// (the flexible UI path — the closure may borrow local state).
    ///
    /// Returns `true` when the user requested to close it this frame.
    pub fn show_with<F>(&self, ui: &mut egui::Ui, body: F) -> bool
    where
        F: FnOnce(&mut egui::Ui),
    {
        Self::frame(self.chrome(), ui, body, None)
    }

    /// As [`show_with`](Modal::show_with), plus an action bar pinned to the bottom
    /// of the card — design `.m-foot`.
    ///
    /// The footer is a *sibling* of the body, not part of it: it spans the card's
    /// full width with its own panel fill and a hairline along its top edge, so it
    /// cannot be drawn from inside `body` (which is inset by the body padding).
    /// Lay the actions out right-to-left; they render right-aligned as the design
    /// specifies.
    /// The footer only has to live for the duration of this call, so it may borrow
    /// locals — no need to route clicks through a cell.
    pub fn show_with_footer<'f, F, G>(&self, ui: &mut egui::Ui, body: F, footer: G) -> bool
    where
        F: FnOnce(&mut egui::Ui),
        G: FnOnce(&mut egui::Ui) + 'f,
    {
        Self::frame(self.chrome(), ui, body, Some(Box::new(footer)))
    }

    /// Draw the backdrop + centered window chrome and run `body` for content,
    /// optionally followed by a full-bleed action bar.
    fn frame<F: FnOnce(&mut egui::Ui)>(
        chrome: Chrome<'_>,
        ui: &mut egui::Ui,
        body: F,
        footer: Option<Footer<'_>>,
    ) -> bool {
        let Chrome {
            id,
            title,
            eyebrow,
            eyebrow_color,
            subtitle,
            glyph,
            glyph_color,
            glyph_tile,
            dismissible,
            width,
            width_pct,
            height_pct,
        } = chrome;
        let ctx = ui.ctx().clone();
        let colors = ThemeColors::from_ctx(&ctx);
        // A non-dismissible modal must be answered: Escape and backdrop clicks do
        // nothing and no ✕ is drawn.
        let mut close_requested = dismissible && ctx.input(|i| i.key_pressed(egui::Key::Escape));

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
        if backdrop.clicked() && dismissible {
            close_requested = true;
        }

        // ── Window (fixed px width, or a fraction of the viewport) ───────────
        // Design `.card`: canvas fill (*not* the panel tint), hairline edge, panel
        // corners and the dialog shadow. No inner margin — the head and body own
        // their own padding, which is asymmetric (18/20/14 vs 0/20/16).
        let modal_frame = egui::Frame::new()
            .fill(colors.bg)
            .stroke(crate::theme::edge_stroke(&colors))
            .corner_radius(crate::theme::RADIUS_PANEL)
            .shadow(crate::theme::dialog_shadow(ui.visuals().dark_mode));
        let win = egui::Window::new(format!("__modal_{id}"))
            .order(egui::Order::Foreground)
            .collapsible(false)
            .resizable(false)
            .movable(false)
            .title_bar(false)
            .frame(modal_frame)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0]);

        let w = width.or_else(|| width_pct.map(|p| screen.width() * p.clamp(0.0, 1.0)));
        let h = height_pct.map(|p| screen.height() * p.clamp(0.0, 1.0));
        let win = match (w, h) {
            (Some(w), Some(h)) => win.fixed_size([w, h]),
            (Some(w), None) => win.min_width(w).max_width(w),
            (None, Some(h)) => win
                .min_width(320.0)
                .max_width(480.0)
                .min_height(h)
                .max_height(h),
            (None, None) => win.min_width(320.0).max_width(480.0),
        };

        win.show(&ctx, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;

            // ── Header — design `.m-head{padding:18px 20px 14px;gap:13px}` ────
            egui::Frame::NONE
                .inner_margin(egui::Margin {
                    left: 20,
                    right: 20,
                    top: 18,
                    bottom: 14,
                })
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.x = 13.0;
                    ui.horizontal_top(|ui| {
                        // Leading status glyph — design `.glyph` (a bare 44px cell)
                        // or `.avatar` (the same box filled, with a hairline edge).
                        if let Some(g) = glyph.filter(|g| !g.is_empty()) {
                            let tint = glyph_color
                                .and_then(|t| crate::theme::resolve_color(t, &colors))
                                .unwrap_or(colors.accent);
                            let (rect, _) = ui
                                .allocate_exact_size(egui::vec2(44.0, 44.0), egui::Sense::hover());
                            if glyph_tile {
                                ui.painter().rect_filled(
                                    rect,
                                    crate::theme::RADIUS_PANEL,
                                    colors.surface,
                                );
                                ui.painter().rect_stroke(
                                    rect,
                                    crate::theme::RADIUS_PANEL,
                                    crate::theme::edge_stroke(&colors),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            ui.painter().text(
                                if glyph_tile {
                                    rect.center()
                                } else {
                                    // Design `.glyph` is left-aligned in its cell.
                                    egui::pos2(rect.left(), rect.center().y)
                                },
                                if glyph_tile {
                                    egui::Align2::CENTER_CENTER
                                } else {
                                    egui::Align2::LEFT_CENTER
                                },
                                g,
                                crate::theme::phosphor_font_id(if glyph_tile {
                                    24.0
                                } else {
                                    30.0
                                }),
                                tint,
                            );
                        }
                        ui.vertical(|ui| {
                            ui.spacing_mut().item_spacing.y = 3.0;
                            // Design `.grouplabel`: the eyebrow above the title,
                            // 10px bold, muted unless the caller tints it.
                            if let Some(label) = eyebrow.filter(|e| !e.is_empty()) {
                                ui.add(
                                    Typography::builder()
                                        .text(label)
                                        .variant(TypographyVariant::GroupLabel)
                                        .size(EYEBROW_FONT)
                                        .maybe_color(eyebrow_color)
                                        .build(),
                                );
                            }
                            ui.add(
                                Typography::builder()
                                    .text(title)
                                    .variant(TypographyVariant::Heading)
                                    .size(15.0)
                                    .build(),
                            );
                            if let Some(sub) = subtitle.filter(|s| !s.is_empty()) {
                                ui.label(
                                    egui::RichText::new(sub).size(12.0).color(colors.fg_muted),
                                );
                            }
                        });
                        // Design `.m-head .x` is a *ghost* 24px close affordance —
                        // no fill or edge until hovered. Omitted entirely when the
                        // modal is not dismissible.
                        if dismissible {
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                                if IconButton::builder()
                                    .icon(egui_phosphor::regular::X)
                                    .frame(false)
                                    .size_px(24.0)
                                    .icon_size(15.0)
                                    .tooltip("Close")
                                    .build()
                                    .ui(ui)
                                    .clicked()
                                {
                                    close_requested = true;
                                }
                            });
                        }
                    });
                });

            // ── Body — design `.m-body{padding:0 20px 16px}` ──────────────────
            egui::Frame::NONE
                .inner_margin(egui::Margin {
                    left: 20,
                    right: 20,
                    top: 0,
                    bottom: 16,
                })
                .show(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = 8.0;
                    // Body copy is 13px in the softer `subtext0`; children that set
                    // their own size or colour still win.
                    ui.visuals_mut().widgets.noninteractive.fg_stroke.color = colors.fg_subtle();
                    if let Some(h) = h {
                        // Head padding + title + body padding, so a height-capped
                        // modal scrolls its body rather than overflowing the card.
                        let header_overhead = 78.0;
                        egui::ScrollArea::vertical()
                            .max_height((h - header_overhead).max(0.0))
                            .show(ui, body);
                    } else {
                        body(ui);
                    }
                });

            // ── Footer — design `.m-foot`: full-bleed panel strip with a hairline
            // along its top edge, actions right-aligned. Drawn edge to edge, so it
            // uses the card's own width rather than the body's inset.
            if let Some(footer) = footer {
                let strip = egui::Frame::NONE
                    .fill(colors.bg_panel)
                    .inner_margin(egui::Margin {
                        left: 20,
                        right: 20,
                        top: 12,
                        bottom: 16,
                    })
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.x = 8.0;
                        // `horizontal` bounds the row to its content height first.
                        // Laying the actions out right-to-left directly on this
                        // vertical ui would let a nested layout (e.g. a `.m-foot
                        // .split` left-hand control) claim the window's whole
                        // remaining height, inflating the strip.
                        ui.horizontal(|ui| {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                footer,
                            );
                        });
                    });
                ui.painter().hline(
                    strip.response.rect.x_range(),
                    strip.response.rect.top() + 0.5,
                    egui::Stroke::new(
                        1.0,
                        crate::theme::with_alpha(colors.surface_raised, 66), // 26%
                    ),
                );
            }
        });

        close_requested
    }
}
