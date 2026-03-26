use std::path::Path;

use eframe::egui::{self, Color32, CornerRadius, Frame, Layout, Sense, TextureHandle, Vec2};

use crate::components::traits::StatelessComponent;
use crate::theme::{Theme, ThemeColors};

// ── Icon ──────────────────────────────────────────────────────────────────────

pub enum CardIcon<'a> {
    /// Render a colored circle placeholder (default when no image is available)
    Color(Color32),
    /// Decode a PNG from disk and cache the texture in egui memory.
    /// The bytes are read and decoded only on the first frame; subsequent
    /// frames hit the egui `TextureHandle` cache with no I/O or allocation.
    Path(&'a Path),
}

// ── Action definition ─────────────────────────────────────────────────────────

pub struct CardAction<'a> {
    pub label: &'a str,
    pub variant: CardActionVariant,
}

pub enum CardActionVariant {
    /// Normal button — uses theme text color
    Primary,
    /// Destructive button — uses theme error color
    Danger,
}

// ── Props ─────────────────────────────────────────────────────────────────────

pub struct CardProps<'a> {
    pub title: &'a str,
    pub subtitle: &'a str,
    /// Optional single-line metadata shown below the subtitle (e.g. "v1.0 | by Author")
    pub meta: Option<&'a str>,
    /// `Some(bool)` shows a toggle in the header; `None` hides it
    pub is_enabled: Option<bool>,
    pub icon: CardIcon<'a>,
    /// Action buttons rendered at the bottom-right of the card
    pub actions: &'a [CardAction<'a>],
}

// ── Output ────────────────────────────────────────────────────────────────────

pub struct CardOutput {
    /// `true` when the toggle was clicked (only meaningful when `is_enabled` is `Some`)
    pub toggled: bool,
    /// Index into `props.actions` of the button that was clicked, if any
    pub action_clicked: Option<usize>,
}

// ── Component ─────────────────────────────────────────────────────────────────

pub struct Card;

impl StatelessComponent for Card {
    type Props<'a> = CardProps<'a>;
    type Output = CardOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| Theme::default().colors())
        });

        let mut output = CardOutput {
            toggled: false,
            action_clicked: None,
        };

        Frame::new()
            .fill(colors.surface0)
            .corner_radius(CornerRadius::same(12))
            .inner_margin(16.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Icon — 48×48 box
                    let (rect, _) =
                        ui.allocate_exact_size(Vec2::new(48.0, 48.0), Sense::hover());
                    match &props.icon {
                        CardIcon::Color(color) => {
                            ui.painter().rect_filled(
                                rect,
                                CornerRadius::same(10),
                                color.linear_multiply(0.2),
                            );
                            ui.painter().circle_filled(rect.center(), 12.0, *color);
                        }
                        CardIcon::Path(path) => {
                            let texture = load_icon_texture(ui.ctx(), path);
                            ui.put(rect, egui::Image::new(&texture).fit_to_exact_size(rect.size())
                                .corner_radius(CornerRadius::same(10)));
                        }
                    }

                    ui.add_space(12.0);

                    ui.vertical(|ui| {
                        // Title row — toggle on the right when present
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(props.title)
                                    .color(colors.text)
                                    .size(16.0)
                                    .strong(),
                            );
                            if let Some(enabled) = props.is_enabled {
                                ui.with_layout(
                                    Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        if toggle_switch(
                                            ui,
                                            enabled,
                                            colors.success,
                                            colors.surface2,
                                        )
                                        .clicked()
                                        {
                                            output.toggled = true;
                                        }
                                    },
                                );
                            }
                        });

                        ui.add_space(4.0);

                        ui.label(
                            egui::RichText::new(props.subtitle)
                                .color(colors.overlay1)
                                .size(13.0),
                        );

                        if let Some(meta) = props.meta {
                            ui.add_space(4.0);
                            ui.label(
                                egui::RichText::new(meta)
                                    .color(colors.overlay1.linear_multiply(0.8))
                                    .size(11.0),
                            );
                        }

                        if !props.actions.is_empty() {
                            ui.add_space(8.0);
                            ui.with_layout(
                                Layout::right_to_left(egui::Align::Min),
                                |ui| {
                                    // Right-to-left layout: iterate in reverse so buttons
                                    // appear in the order the caller declared them.
                                    for (i, action) in props.actions.iter().enumerate().rev() {
                                        let label_color = match action.variant {
                                            CardActionVariant::Primary => colors.text,
                                            CardActionVariant::Danger => colors.error,
                                        };
                                        let btn = egui::Button::new(
                                            egui::RichText::new(action.label).color(label_color),
                                        )
                                        .fill(colors.surface1)
                                        .corner_radius(6.0);
                                        if ui.add(btn).clicked() {
                                            output.action_clicked = Some(i);
                                        }
                                    }
                                },
                            );
                        }
                    });
                });
            });

        output
    }
}

// ── Icon texture loader ───────────────────────────────────────────────────────

/// Decode a PNG from `path` and upload it to the GPU as an egui texture.
/// The `TextureHandle` is stored in egui's per-frame memory keyed by path,
/// so the file is read and decoded only once per session.
fn load_icon_texture(ctx: &egui::Context, path: &Path) -> TextureHandle {
    let key = egui::Id::new(("card_icon", path));

    ctx.memory(|mem| mem.data.get_temp::<TextureHandle>(key))
        .unwrap_or_else(|| {
            let texture = decode_png_to_texture(ctx, path);
            ctx.memory_mut(|mem| mem.data.insert_temp(key, texture.clone()));
            texture
        })
}

fn decode_png_to_texture(ctx: &egui::Context, path: &Path) -> TextureHandle {
    // Attempt to decode; on failure fall back to a 1×1 transparent pixel.
    let color_image = std::fs::read(path)
        .ok()
        .and_then(|bytes| image::load_from_memory(&bytes).ok())
        .map(|img| {
            let rgba = img.to_rgba8();
            let (w, h) = rgba.dimensions();
            egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba)
        })
        .unwrap_or_else(|| egui::ColorImage::from_rgba_unmultiplied([1, 1], &[0, 0, 0, 0]));

    ctx.load_texture(
        path.to_string_lossy(),
        color_image,
        egui::TextureOptions::LINEAR,
    )
}

// ── Toggle switch helper ──────────────────────────────────────────────────────

/// Pill-shaped toggle. Returns a `Response` — check `.clicked()` to detect changes.
fn toggle_switch(
    ui: &mut egui::Ui,
    enabled: bool,
    on_color: Color32,
    off_color: Color32,
) -> egui::Response {
    let (rect, response) =
        ui.allocate_exact_size(Vec2::new(36.0, 20.0), Sense::click());

    if ui.is_rect_visible(rect) {
        let bg = if enabled { on_color } else { off_color };
        ui.painter().rect_filled(rect, CornerRadius::same(10), bg);
        let knob_x = if enabled {
            rect.right() - 10.0
        } else {
            rect.left() + 10.0
        };
        ui.painter()
            .circle_filled(egui::pos2(knob_x, rect.center().y), 8.0, Color32::WHITE);
    }

    response
}
