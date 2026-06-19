use std::path::Path;

use eframe::egui::{self, Color32, CornerRadius, Frame, Layout, Sense, Vec2};

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
    Default,
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
            .fill(colors.surface)
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
                            if let Some(texture) =
                                super::helpers::load_icon_texture(ui.ctx(), path, "card_icon")
                            {
                                ui.put(
                                    rect,
                                    egui::Image::new(&texture)
                                        .fit_to_exact_size(rect.size())
                                        .corner_radius(CornerRadius::same(10)),
                                );
                            }
                        }
                    }

                    ui.add_space(12.0);

                    ui.vertical(|ui| {
                        // Title row — toggle on the right when present
                        ui.horizontal(|ui| {
                            ui.label(
                                egui::RichText::new(props.title)
                                    .color(colors.fg)
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
                                .color(colors.fg_muted)
                                .size(13.0),
                        );

                        if !props.tags.is_empty() {
                            ui.add_space(6.0);
                            ui.horizontal(|ui| {
                                for tag in props.tags {
                                    egui::Frame::new()
                                        .fill(colors.surface_raised)
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
                                    .color(colors.fg_muted.linear_multiply(0.8))
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
                                        CardActionVariant::Default => ButtonColor::Default,
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
                                            ..Default::default()
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
