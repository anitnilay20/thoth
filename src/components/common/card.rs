use std::path::Path;

use eframe::egui::{self, Color32, CornerRadius, Frame, Layout, Sense, TextureHandle, Vec2};

use crate::components::button::{Button, ButtonColor, ButtonProps, ButtonType};
use crate::components::toggle_switch::{ToggleSwitch, ToggleSwitchEvent, ToggleSwitchProps};
use crate::components::traits::StatelessComponent;
use crate::theme::{Theme, ThemeColors};

pub enum CardIcon<'a> {
    /// Render a colored circle placeholder (default when no image is available)
    Color(Color32),
    /// Decode a PNG from disk and cache the texture in egui memory.
    /// The bytes are read and decoded only on the first frame; subsequent
    /// frames hit the egui `TextureHandle` cache with no I/O or allocation.
    Path(&'a Path),
}

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

pub struct CardProps<'a> {
    pub title: &'a str,
    pub subtitle: &'a str,
    /// Optional single-line metadata shown below the subtitle (e.g. "v1.0 | by Author")
    pub meta: Option<&'a str>,
    /// Optional tags rendered as small pill badges below the subtitle
    pub tags: &'a [&'a str],
    /// `Some(bool)` shows a toggle in the header; `None` hides it
    pub is_enabled: Option<bool>,
    pub icon: CardIcon<'a>,
    /// Action buttons rendered at the bottom-right of the card
    pub actions: &'a [CardAction<'a>],
}

pub enum CardEvent {
    /// Emitted when the toggle switch (if shown) is toggled. The new value is provided.
    Toggled(bool),
    /// Emitted when an action button is clicked. The index of the clicked button is provided.
    ActionClicked(usize),
}

pub struct CardOutput {
    pub events: Vec<CardEvent>,
}

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

        let mut output = CardOutput { events: Vec::new() };

        Frame::new()
            .fill(colors.surface0)
            .corner_radius(CornerRadius::same(12))
            .inner_margin(16.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // Icon — 48×48 box
                    let (rect, _) = ui.allocate_exact_size(Vec2::new(48.0, 48.0), Sense::hover());
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
                            ui.put(
                                rect,
                                egui::Image::new(&texture)
                                    .fit_to_exact_size(rect.size())
                                    .corner_radius(CornerRadius::same(10)),
                            );
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
                                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                                    let toggle_switch = ToggleSwitch::render(
                                        ui,
                                        ToggleSwitchProps {
                                            enabled,
                                            hover_text: Some(
                                                "Enable or disable this plugin".into(),
                                            ),
                                        },
                                    );
                                    for event in toggle_switch.events {
                                        match event {
                                            ToggleSwitchEvent::Toggled(toggled) => {
                                                output.events.push(CardEvent::Toggled(toggled))
                                            }
                                        }
                                    }
                                });
                            }
                        });

                        ui.add_space(4.0);

                        ui.label(
                            egui::RichText::new(props.subtitle)
                                .color(colors.overlay1)
                                .size(13.0),
                        );

                        if !props.tags.is_empty() {
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                for tag in props.tags {
                                    egui::Frame::new()
                                        .fill(colors.surface1)
                                        .corner_radius(4.0)
                                        .inner_margin(egui::Margin::symmetric(6, 2))
                                        .show(ui, |ui| {
                                            ui.label(
                                                egui::RichText::new(*tag)
                                                    .size(10.0)
                                                    .color(colors.info),
                                            );
                                        });
                                    ui.add_space(2.0);
                                }
                            });
                        }

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
                            ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
                                // Right-to-left layout: iterate in reverse so buttons
                                // appear in the order the caller declared them.
                                for (i, action) in props.actions.iter().enumerate().rev() {
                                    let button_color = match action.variant {
                                        CardActionVariant::Primary => ButtonColor::Default,
                                        CardActionVariant::Danger => ButtonColor::Danger,
                                    };
                                    let btn = Button::render(
                                        ui,
                                        ButtonProps {
                                            label: action.label.to_string(),
                                            button_type: ButtonType::Elevated,
                                            color: button_color,
                                            hover_text: None,
                                            size: None,
                                            width: None,
                                            height: None,
                                        },
                                    );
                                    if btn.clicked {
                                        output.events.push(CardEvent::ActionClicked(i));
                                    }
                                }
                            });
                        }
                    });
                });
            });

        output
    }
}

// TODO: Move load icon texture logic to a shared utility module if other components need it
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

/// Maximum icon file size we're willing to read. Protects against accidentally
/// pointing at a large asset and allocating huge amounts of memory.
const MAX_ICON_SIZE_BYTES: u64 = 5_000_000;

fn decode_png_to_texture(ctx: &egui::Context, path: &Path) -> TextureHandle {
    // Attempt to decode; on failure log a warning and fall back to a 1×1 transparent pixel.

    // Guard against unexpectedly large files before reading into memory.
    match std::fs::metadata(path) {
        Ok(meta) if meta.len() > MAX_ICON_SIZE_BYTES => {
            eprintln!(
                "warn: icon at {} is too large ({} bytes > {MAX_ICON_SIZE_BYTES}), skipping",
                path.display(),
                meta.len()
            );
            let fallback = egui::ColorImage::from_rgba_unmultiplied([1, 1], &[0, 0, 0, 0]);
            return ctx.load_texture(
                path.to_string_lossy(),
                fallback,
                egui::TextureOptions::LINEAR,
            );
        }
        Err(e) => {
            eprintln!("warn: failed to stat icon at {}: {e}", path.display());
            let fallback = egui::ColorImage::from_rgba_unmultiplied([1, 1], &[0, 0, 0, 0]);
            return ctx.load_texture(
                path.to_string_lossy(),
                fallback,
                egui::TextureOptions::LINEAR,
            );
        }
        _ => {}
    }

    let color_image = match std::fs::read(path) {
        Err(e) => {
            eprintln!("warn: failed to read icon at {}: {e}", path.display());
            None
        }
        Ok(bytes) => match image::load_from_memory(&bytes) {
            Err(e) => {
                eprintln!("warn: failed to decode icon at {}: {e}", path.display());
                None
            }
            Ok(img) => {
                let rgba = img.to_rgba8();
                let (w, h) = rgba.dimensions();
                Some(egui::ColorImage::from_rgba_unmultiplied(
                    [w as usize, h as usize],
                    &rgba,
                ))
            }
        },
    }
    .unwrap_or_else(|| egui::ColorImage::from_rgba_unmultiplied([1, 1], &[0, 0, 0, 0]));

    ctx.load_texture(
        path.to_string_lossy(),
        color_image,
        egui::TextureOptions::LINEAR,
    )
}
