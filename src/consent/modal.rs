use std::cell::Cell;

use eframe::egui::{self, Align2, CornerRadius, Frame, Layout, Margin, Stroke, StrokeKind};

use crate::{
    components::traits::ContextComponent,
    theme::{
        RADIUS_CONTROL, RADIUS_PANEL, ThemeColors, edge_stroke, get_contrast_text_color,
        phosphor_font_id,
    },
};
use thoth_plugin_sdk::{
    components::{
        Badge, Button, ButtonColor, Checkbox, Icon, Modal, Separator, Size, Typography,
        TypographyVariant,
    },
    theme::{FONT_CAPTION, FONT_CONTROL, RADIUS_PILL, color_to_hex, with_alpha},
};

use super::manager::{ConsentRequest, PermissionEntry};

/// `.pill{font-size:9.5px}` — a rung below the caption scale, so it is pinned
/// here rather than reached through a size preset.
const PILL_FONT: f32 = 9.5;

// ── Props ─────────────────────────────────────────────────────────────────────

pub struct ConsentModalProps<'a> {
    /// The active consent request, or `None` when nothing is pending.
    pub request: Option<ConsentRequest>,
    /// Called when the user clicks Allow.
    /// The `bool` argument is `true` when "Remember this choice" is checked.
    pub on_accept: &'a dyn Fn(bool),
    /// Called when the user clicks Cancel.
    pub on_cancel: &'a dyn Fn(),
}

// ── Component ─────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct ConsentModal {
    remember: bool,
}

impl ContextComponent for ConsentModal {
    type Props<'a> = ConsentModalProps<'a>;
    type Output = ();

    fn render(&mut self, ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let Some(request) = props.request else {
            self.remember = false;
            return;
        };

        let colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| crate::theme::Theme::default().colors())
        });

        // The footer closure borrows these, so they must be passed by reference —
        // `Cell::clone` would hand the footer detached copies and silently drop
        // every click.
        let remember = Cell::new(self.remember);
        let accepted = Cell::new(false);
        let cancelled = Cell::new(false);

        // "Remember" only persists for domain-scoped (network) consents; hide it
        // for one-off consents like dataset export, where it would be a no-op.
        let show_remember = request.domain.is_some();

        // Design `.card.w480` — the base modal supplies the head, body and footer
        // chrome, including the `.avatar` tile (the header's glyph-tile slot).
        // Not dismissible: this dialog absorbs Escape and backdrop clicks and
        // shows no ✕, so the user must make an explicit Allow/Cancel choice.
        let modal = Modal::builder()
            .id("consent_modal")
            .title(request.title.as_str())
            // `.grouplabel.warn` — the header's eyebrow, above the title.
            .eyebrow("PERMISSION REQUESTED")
            .eyebrow_color("warning")
            .glyph(egui_phosphor::regular::PUZZLE_PIECE)
            .glyph_color("accent")
            .glyph_tile(true)
            .dismissible(false)
            .open(true)
            .width(480.0)
            .build();

        let _never_closes = modal.show_with_footer(
            ui,
            |ui| render_body(ui, &request, &colors),
            |ui| render_footer(ui, show_remember, &remember, &accepted, &cancelled),
        );

        self.remember = remember.get();
        if accepted.get() {
            (props.on_accept)(self.remember);
            self.remember = false;
        } else if cancelled.get() {
            (props.on_cancel)();
            self.remember = false;
        }
    }
}

// ── Section renderers ─────────────────────────────────────────────────────────

fn render_body(ui: &mut egui::Ui, request: &ConsentRequest, colors: &ThemeColors) {
    // Every gap in this body is a design margin, so drive them all explicitly.
    ui.spacing_mut().item_spacing = egui::Vec2::ZERO;

    render_status_badge(ui, colors);

    // Design `.sep` between head and body. The body inset already matches the
    // rule's own `margin:0 20px`, so a full-width hairline lands exactly right.
    ui.add(Separator::rule(color_to_hex(with_alpha(
        colors.surface_raised,
        77, // surface1@30%
    ))));

    // This dialog overrides the shared body padding with `padding-top:14px`.
    ui.add_space(14.0);

    if !request.message.is_empty() {
        // `.m-body{font-size:13px;color:var(--subtext0)}`
        ui.add(
            Typography::builder()
                .text(&request.message)
                .variant(TypographyVariant::BodyLarge)
                .color(color_to_hex(colors.fg_subtle()))
                .build(),
        );
    }

    // `.grouplabel{margin:14px 0 7px}`
    ui.add_space(14.0);
    ui.add(
        Typography::builder()
            .text("THIS WILL ALLOW THE PLUGIN TO")
            .variant(TypographyVariant::GroupLabel)
            .size(10.0)
            .build(),
    );
    ui.add_space(7.0);

    // `.permbox` — the crust-filled list container.
    Frame::new()
        .fill(colors.bg_sunken)
        .stroke(edge_stroke(colors))
        .corner_radius(RADIUS_PANEL)
        .inner_margin(Margin::same(5))
        .show(ui, |ui| {
            // `.permbox` is a block element: it spans the body's full width rather
            // than shrinking to the longest permission label.
            ui.set_min_width(ui.available_width());
            for (i, entry) in request.permissions.iter().enumerate() {
                // `.perm+.perm` carries an inset top hairline.
                render_permission_row(ui, entry, colors, i > 0);
            }
        });
}

fn render_footer(
    ui: &mut egui::Ui,
    show_remember: bool,
    remember: &Cell<bool>,
    accepted: &Cell<bool>,
    cancelled: &Cell<bool>,
) {
    // The footer lays out right-to-left, so the primary action goes first.
    if ui
        .add(
            Button::builder()
                .label("Allow")
                .color(ButtonColor::Primary)
                .build(),
        )
        .clicked()
    {
        accepted.set(true);
    }

    if ui
        .add(
            Button::builder()
                .label("Cancel")
                .color(ButtonColor::Default)
                .build(),
        )
        .clicked()
    {
        cancelled.set(true);
    }

    if show_remember {
        // `.m-foot.split` — the remember toggle takes the remaining width and sits
        // against the left edge, opposite the actions.
        ui.with_layout(Layout::left_to_right(egui::Align::Center), |ui| {
            let mut checkbox = Checkbox::builder()
                .label("Remember this choice")
                .checked(remember.get())
                .build();
            if checkbox.show(ui).changed() {
                remember.set(checkbox.checked);
            }
        });
    }
}

/// The `.avatar .badge` status dot, painted over the bottom-right corner of the
/// header's avatar tile.
///
/// The shared header owns the tile itself and hands back no geometry, so the
/// corner is derived from the body: the body starts flush under the head, whose
/// `padding-bottom` and matching 20px left inset fix where the 44px tile sits.
fn render_status_badge(ui: &mut egui::Ui, colors: &ThemeColors) {
    /// `.avatar{width:44px;height:44px}`
    const TILE: f32 = 44.0;
    /// `.m-head{padding:18px 20px 14px}` — the gap under the tile.
    const HEAD_BOTTOM: f32 = 14.0;

    let tile_corner = egui::pos2(
        ui.max_rect().left() + TILE,
        ui.max_rect().top() - HEAD_BOTTOM,
    );
    // `.badge{right:-4px;bottom:-4px;width:20px;border-radius:50%}` — a round
    // warning dot hanging off the corner, ringed in the card fill so it detaches.
    // `RADIUS_PILL` saturates to `u8::MAX`; the tessellator clamps it to a circle.
    let badge = egui::Rect::from_min_size(
        tile_corner - egui::vec2(16.0, 16.0),
        egui::Vec2::splat(20.0),
    );
    let round = CornerRadius::same(RADIUS_PILL as u8);
    ui.painter().rect_filled(badge, round, colors.warning);
    ui.painter().rect_stroke(
        badge,
        round,
        Stroke::new(2.0, colors.bg),
        StrokeKind::Outside,
    );
    ui.painter().text(
        badge.center(),
        Align2::CENTER_CENTER,
        egui_phosphor::regular::SHIELD_WARNING,
        phosphor_font_id(12.0),
        get_contrast_text_color(colors.warning),
    );
}

/// One `.perm` row: glyph, stacked label + scope, and a `.pill` for sensitive
/// permissions. `divided` draws the inset hairline rows after the first carry.
fn render_permission_row(
    ui: &mut egui::Ui,
    entry: &PermissionEntry,
    colors: &ThemeColors,
    divided: bool,
) {
    // `.perm{padding:8px 9px;border-radius:8px}` — the row itself is unfilled, so
    // the radius only matters if it ever gains a hover wash.
    let row = Frame::new()
        .corner_radius(RADIUS_CONTROL)
        .inner_margin(Margin {
            left: 9,
            right: 9,
            top: 8,
            bottom: 8,
        })
        .show(ui, |ui| {
            // Full-width too, so the inter-row hairline spans the box and a future
            // hover wash would cover the whole row.
            ui.set_min_width(ui.available_width());
            ui.spacing_mut().item_spacing.x = 10.0; // `.perm{gap:10px}`
            ui.horizontal(|ui| {
                // `.perm i` — warn-tinted for a sensitive permission (`.sens`).
                ui.add(
                    Icon::builder()
                        .glyph(entry.icon)
                        .size(17.0)
                        .color(color_to_hex(if entry.sensitive {
                            colors.warning
                        } else {
                            colors.accent
                        }))
                        .build(),
                );
                ui.vertical(|ui| {
                    // The two lines are adjacent blocks in the design; a hair of
                    // leading stands in for its 1.5 line-height.
                    ui.spacing_mut().item_spacing.y = 2.0;
                    // `.pl` — 12.5px in the primary foreground (the variant default).
                    ui.add(
                        Typography::builder()
                            .text(&entry.label)
                            .variant(TypographyVariant::Body)
                            .size(FONT_CONTROL)
                            .build(),
                    );
                    // `.ps` — monospace scope hint.
                    ui.add(
                        Typography::builder()
                            .text(&entry.scope)
                            .variant(TypographyVariant::Mono)
                            .size(FONT_CAPTION)
                            .color(color_to_hex(colors.fg_muted))
                            .build(),
                    );
                });
                if entry.sensitive {
                    // `.pill{margin-left:auto}`
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        render_sensitive_pill(ui, colors);
                    });
                }
            });
        });

    if divided {
        // `.perm+.perm{box-shadow:inset 0 1px 0 surface1@24%}` — painted rather
        // than added, so it rides the row's own top edge instead of pushing it.
        Separator::rule(color_to_hex(with_alpha(colors.surface_raised, 61))).paint_at(
            ui,
            row.response.rect.x_range(),
            row.response.rect.top() + 0.5,
        );
    }
}

/// The `.pill` tag on a sensitive permission — a fully round warning wash behind
/// 9.5px bold proportional text, i.e. the shared soft [`Badge`] at a pill radius.
fn render_sensitive_pill(ui: &mut egui::Ui, colors: &ThemeColors) {
    ui.add(
        Badge::builder()
            .label("SENSITIVE")
            .color(color_to_hex(colors.warning))
            .soft(true)
            .pill(true)
            // `.pill{font-weight:700}` in the body family, not a code token.
            .mono(false)
            .bold(true)
            // `.pill{padding:2px 7px}` — the medium preset's horizontal padding.
            .size(Size::Medium)
            .font_size(PILL_FONT)
            .build(),
    );
}
