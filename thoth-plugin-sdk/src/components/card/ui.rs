use egui::{Align2, CornerRadius, Layout, RichText, Sense};

use crate::components::{
    Button, ButtonColor, ButtonType, ToggleSwitch, Typography, TypographyVariant,
};
use crate::theme::{
    FONT_BODY, FONT_CAPTION, RADIUS_CONTROL, RADIUS_PANEL, ThemeColors, edge_stroke,
    phosphor_font_id, with_alpha,
};

use super::{Card, CardIcon};

/// Inner padding — design `.card{padding:16px}`.
const PAD: i8 = 16;
/// Gap between the icon tile and the body — design `.card{gap:12px}`.
const ICON_GAP: f32 = 12.0;
/// Leading tile — design `.cicon{width:44px;height:44px;font-size:26px}`.
const TILE: f32 = 44.0;
/// Glyph size inside the tile — design `.cicon{font-size:26px}`.
const TILE_GLYPH: f32 = 26.0;
/// Tile tint — design `.cicon{background:accent@16%}`.
const TILE_TINT_ALPHA: u8 = 41; // 16% of 255
/// Tag chip text — design `.ctag{font-size:10px;font-family:mono}`.
const FONT_TAG: f32 = 10.0;
/// Tag chip corner — design `.ctag{border-radius:4px}`. Below the radius ladder's
/// smallest rung, so it stays a raw value like the badge's 3px chips.
const TAG_RADIUS: f32 = 4.0;
/// Gap between tag chips — design `.ctags{gap:5px}`.
const TAG_GAP: f32 = 5.0;
/// Meta line — design `.cmeta{color:fg-muted@80%}`.
const META_ALPHA: u8 = 204; // 80% of 255
/// Subtitle offset — design `.csub{margin-top:4px}`.
const GAP_SUB: f32 = 4.0;
/// Tag / meta block offset — design `.ctags`/`.cmeta{margin-top:9px}`.
const GAP_BLOCK: f32 = 9.0;
/// Action row offset — design `.cact{margin-top:12px}`.
const GAP_ACTIONS: f32 = 12.0;
/// Gap between action buttons — design `.cact{gap:8px}`.
const ACTION_GAP: f32 = 8.0;

/// What the user did in a [`Card`] this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CardEvent {
    /// The enable toggle was flipped to `bool`.
    Toggled(bool),
    /// Action `index` was clicked.
    ActionClicked(usize),
}

impl Card {
    /// Render the card, mutating the enable toggle in place and reporting the
    /// user's action this frame, if any. Body-node events are collected into
    /// `events`.
    pub fn show(
        &mut self,
        ui: &mut egui::Ui,
        events: &mut Vec<crate::render_node::UiEvent>,
    ) -> Option<CardEvent> {
        let colors = ThemeColors::from_ctx(ui.ctx());
        let mut event = None;

        // Design `.card`: a flat surface panel with a hairline edge — no drop
        // shadow (cards sit *in* the layout, they don't float above it).
        egui::Frame::new()
            .fill(colors.surface)
            .stroke(edge_stroke(&colors))
            .corner_radius(RADIUS_PANEL)
            .inner_margin(PAD)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // ── Leading icon ─────────────────────────────────────────
                    if let Some(icon) = &self.icon {
                        ui.spacing_mut().item_spacing.x = 0.0;
                        Self::icon_tile(ui, icon, &colors);
                        ui.add_space(ICON_GAP);
                    }

                    ui.vertical(|ui| {
                        // ── Title row (+ toggle) ─────────────────────────────
                        ui.horizontal(|ui| {
                            // `.ctitle` — 16px `fg` at weight 600; egui has no
                            // font weights, so this goes through Typography's
                            // faux-bold pass (`Heading` is 16px + bold + `fg`).
                            ui.add(
                                Typography::builder()
                                    .text(self.title.as_str())
                                    .variant(TypographyVariant::Heading)
                                    .build(),
                            );
                            if let Some(on) = self.enabled {
                                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                    if ui
                                        .add(ToggleSwitch::builder().enabled(on).build())
                                        .clicked()
                                    {
                                        self.enabled = Some(!on);
                                        event = Some(CardEvent::Toggled(!on));
                                    }
                                });
                            }
                        });

                        if let Some(subtitle) = &self.subtitle {
                            ui.add_space(GAP_SUB);
                            ui.label(
                                RichText::new(subtitle)
                                    .color(colors.fg_muted)
                                    .size(FONT_BODY),
                            );
                        }

                        if !self.tags.is_empty() {
                            ui.add_space(GAP_BLOCK);
                            ui.horizontal(|ui| {
                                ui.spacing_mut().item_spacing.x = TAG_GAP;
                                for tag in &self.tags {
                                    egui::Frame::new()
                                        .fill(colors.surface_raised)
                                        .corner_radius(TAG_RADIUS)
                                        .inner_margin(egui::Margin::symmetric(6, 2))
                                        .show(ui, |ui| {
                                            ui.label(
                                                RichText::new(tag)
                                                    .monospace()
                                                    .size(FONT_TAG)
                                                    .color(colors.syntax_key),
                                            );
                                        });
                                }
                            });
                        }

                        if let Some(meta) = &self.meta {
                            ui.add_space(GAP_BLOCK);
                            ui.label(
                                RichText::new(meta)
                                    .color(with_alpha(colors.fg_muted, META_ALPHA))
                                    .size(FONT_CAPTION),
                            );
                        }

                        if let Some(body) = &mut self.body {
                            ui.add_space(GAP_BLOCK);
                            body.show(ui, events);
                        }

                        if !self.actions.is_empty() {
                            ui.add_space(GAP_ACTIONS);
                            ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                                ui.spacing_mut().item_spacing.x = ACTION_GAP;
                                for (i, action) in self.actions.iter().enumerate().rev() {
                                    let color = if action.danger {
                                        ButtonColor::Danger
                                    } else {
                                        ButtonColor::Default
                                    };
                                    if ui
                                        .add(
                                            Button::builder()
                                                .label(action.label.as_str())
                                                .color(color)
                                                .button_type(ButtonType::Elevated)
                                                .build(),
                                        )
                                        .clicked()
                                    {
                                        event = Some(CardEvent::ActionClicked(i));
                                    }
                                }
                            });
                        }
                    });
                });
            });

        event
    }

    /// Paint the 44×44 leading tile — design `.cicon`: an accent-tinted square
    /// behind a centred accent glyph, or the icon image clipped to the same box.
    fn icon_tile(ui: &mut egui::Ui, icon: &CardIcon, colors: &ThemeColors) {
        let size = egui::vec2(TILE, TILE);
        let radius = CornerRadius::same(RADIUS_CONTROL as u8);
        match icon {
            CardIcon::Glyph(glyph) => {
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                if ui.is_rect_visible(rect) {
                    ui.painter().rect_filled(
                        rect,
                        RADIUS_CONTROL,
                        with_alpha(colors.accent, TILE_TINT_ALPHA),
                    );
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        glyph,
                        phosphor_font_id(TILE_GLYPH),
                        colors.accent,
                    );
                }
            }
            CardIcon::Image { uri, bytes } => {
                ui.add(
                    egui::Image::from_bytes(uri.clone(), bytes.clone())
                        .fit_to_exact_size(size)
                        .corner_radius(radius),
                );
            }
            CardIcon::IconFile { path } => {
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                if let Some(texture) = crate::components::helpers::load_icon_texture(
                    ui.ctx(),
                    std::path::Path::new(path),
                    "card_icon",
                ) {
                    ui.put(
                        rect,
                        egui::Image::new(&texture)
                            .fit_to_exact_size(rect.size())
                            .corner_radius(radius),
                    );
                }
            }
        }
    }
}
