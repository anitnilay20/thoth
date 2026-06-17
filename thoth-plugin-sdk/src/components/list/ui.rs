use egui::{RichText, Sense};

use crate::components::{Badge, IconButton};
use crate::theme::ThemeColors;

use super::List;

/// What the user did in a [`List`] this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListEvent {
    /// Row `index` was clicked.
    ItemClicked(usize),
    /// Action `action` on row `item` was clicked.
    ActionClicked {
        /// Row index.
        item: usize,
        /// Action index within that row.
        action: usize,
    },
}

impl List {
    /// Render the list. Returns the user's action this frame, if any.
    pub fn show(&self, ui: &mut egui::Ui) -> Option<ListEvent> {
        let colors = ThemeColors::from_ctx(ui.ctx());

        if self.items.is_empty() {
            if let Some(label) = &self.empty_label {
                ui.add_space(8.0);
                ui.label(RichText::new(label).color(colors.fg_muted));
            }
            return None;
        }

        let mut event = None;

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                for (i, item) in self.items.iter().enumerate() {
                    let mut action_hit: Option<usize> = None;

                    let row = egui::Frame::new()
                        .inner_margin(egui::Margin::symmetric(8, 6))
                        .show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.horizontal(|ui| {
                                if let Some(glyph) = &item.icon {
                                    ui.add(
                                        crate::components::Icon::builder()
                                            .glyph(glyph.as_str())
                                            .build(),
                                    );
                                }
                                ui.vertical(|ui| {
                                    ui.horizontal(|ui| {
                                        if let Some(badge) = &item.badge {
                                            ui.add(
                                                Badge::builder()
                                                    .label(badge.text.as_str())
                                                    .maybe_color(badge.color.as_deref())
                                                    .build(),
                                            );
                                        }
                                        ui.label(RichText::new(&item.title).color(colors.fg));
                                    });
                                    if let Some(desc) = &item.description {
                                        ui.label(
                                            RichText::new(desc).color(colors.fg_muted).size(11.0),
                                        );
                                    }
                                });

                                if !item.actions.is_empty() {
                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Center),
                                        |ui| {
                                            for (j, action) in item.actions.iter().enumerate().rev()
                                            {
                                                let btn = IconButton::builder()
                                                    .icon(action.icon.as_str())
                                                    .maybe_tooltip(action.tooltip.as_deref())
                                                    .build();
                                                if ui.add(btn).clicked() {
                                                    action_hit = Some(j);
                                                }
                                            }
                                        },
                                    );
                                }
                            });
                        })
                        .response
                        .interact(Sense::click());

                    if let Some(j) = action_hit {
                        event = Some(ListEvent::ActionClicked { item: i, action: j });
                    } else if row.clicked() {
                        event = Some(ListEvent::ItemClicked(i));
                    }
                    if row.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                }
            });

        event
    }
}
