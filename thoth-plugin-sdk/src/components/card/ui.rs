use egui::{CornerRadius, Layout, RichText};

use crate::components::{Button, ButtonColor, ButtonType, Icon, ToggleSwitch};
use crate::theme::ThemeColors;

use super::{Card, CardIcon};

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

        egui::Frame::new()
            .fill(colors.surface)
            .corner_radius(CornerRadius::same(12))
            .inner_margin(16.0)
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    // ── Leading icon ─────────────────────────────────────────
                    match &self.icon {
                        Some(CardIcon::Glyph(glyph)) => {
                            ui.add(Icon::builder().glyph(glyph.as_str()).size(28.0).build());
                        }
                        Some(CardIcon::Image { uri, bytes }) => {
                            ui.add(
                                egui::Image::from_bytes(uri.clone(), bytes.clone())
                                    .fit_to_exact_size(egui::vec2(48.0, 48.0))
                                    .corner_radius(CornerRadius::same(10)),
                            );
                        }
                        None => {}
                    }
                    ui.add_space(12.0);

                    ui.vertical(|ui| {
                        // ── Title row (+ toggle) ─────────────────────────────
                        ui.horizontal(|ui| {
                            ui.label(RichText::new(&self.title).color(colors.fg).size(16.0).strong());
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
                            ui.add_space(4.0);
                            ui.label(RichText::new(subtitle).color(colors.fg_muted).size(13.0));
                        }

                        if !self.tags.is_empty() {
                            ui.add_space(6.0);
                            ui.horizontal_wrapped(|ui| {
                                for tag in &self.tags {
                                    ui.add(
                                        crate::components::Badge::builder()
                                            .label(tag.as_str())
                                            .build(),
                                    );
                                }
                            });
                        }

                        if let Some(meta) = &self.meta {
                            ui.add_space(4.0);
                            ui.label(RichText::new(meta).color(colors.fg_muted).size(11.0));
                        }

                        if let Some(body) = &mut self.body {
                            ui.add_space(8.0);
                            body.show(ui, events);
                        }

                        if !self.actions.is_empty() {
                            ui.add_space(8.0);
                            ui.with_layout(Layout::right_to_left(egui::Align::Min), |ui| {
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
}
