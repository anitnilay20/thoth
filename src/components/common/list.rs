use eframe::egui;

use crate::{components::traits::StatelessComponent, theme::ThemeColors};

/// A single item in the list
pub struct ListItem<'a> {
    /// Primary label
    pub title: &'a str,
    /// Optional secondary description (muted, smaller)
    pub description: Option<&'a str>,
    /// Optional leading icon (e.g. from egui_phosphor)
    pub icon: Option<&'a str>,
    /// Optional icon color override
    pub icon_color: Option<egui::Color32>,
    /// Optional action button revealed on hover
    pub action: Option<ListItemAction<'a>>,
}

pub struct ListItemAction<'a> {
    pub icon: &'a str,
    pub tooltip: &'a str,
}

pub struct ListOutput {
    /// Index of the item whose action button was clicked, if any
    pub action_clicked: Option<usize>,
}

pub struct ListProps<'a> {
    pub items: &'a [ListItem<'a>],
    /// Text shown when `items` is empty
    pub empty_label: Option<&'a str>,
}

pub struct List;

impl StatelessComponent for List {
    type Props<'a> = ListProps<'a>;
    type Output = ListOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| crate::theme::Theme::default().colors())
        });

        let mut action_clicked: Option<usize> = None;

        if props.items.is_empty() {
            ui.add_space(12.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(props.empty_label.unwrap_or("No items"))
                        .color(colors.overlay1)
                        .size(12.0),
                );
            });
            ui.add_space(12.0);
            return ListOutput { action_clicked };
        }

        for (idx, item) in props.items.iter().enumerate() {
            let item_id = ui.id().with(idx);

            // Read hover state from the previous frame so the button stays
            // visible (and clickable) as the pointer moves onto it.
            let was_hovered = ui
                .ctx()
                .memory(|m| m.data.get_temp::<bool>(item_id).unwrap_or(false));

            let mut btn_clicked = false;

            // allocate_ui with height=0 lets the row shrink to its content
            // height instead of expanding to fill the parent container.
            let row_resp = ui
                .push_id(item_id, |ui| {
                    let avail_width = ui.available_width();
                    let alloc = ui.allocate_ui(egui::vec2(avail_width, 0.0), |ui| {
                        ui.add_space(8.0);
                        ui.horizontal_top(|ui| {
                            ui.set_min_width(ui.available_width());

                            // Leading icon — top-aligned with title
                            if let Some(icon) = item.icon {
                                let color = item.icon_color.unwrap_or(colors.overlay1);
                                ui.label(egui::RichText::new(icon).size(14.0).color(color));
                            }

                            // Remaining space: RTL so the button lands at the far
                            // right, then the text fills everything to its left.
                            // Align::Min prevents egui from allocating full parent
                            // height for centering.
                            ui.with_layout(egui::Layout::right_to_left(egui::Align::Min), |ui| {
                                // Button — always in the tree so it is always
                                // interactive; transparent icon hides it when
                                // the row is not hovered.
                                if let Some(action) = &item.action {
                                    let icon_color = if was_hovered {
                                        colors.overlay1
                                    } else {
                                        egui::Color32::TRANSPARENT
                                    };

                                    let resp = ui
                                        .add(
                                            egui::Button::new(
                                                egui::RichText::new(action.icon)
                                                    .size(12.0)
                                                    .color(icon_color),
                                            )
                                            .fill(egui::Color32::TRANSPARENT)
                                            .stroke(egui::Stroke::NONE)
                                            .min_size(egui::vec2(20.0, 20.0)),
                                        )
                                        .on_hover_text(action.tooltip);

                                    if resp.clicked() {
                                        btn_clicked = true;
                                    }
                                }

                                // Text fills the space to the left of the button.
                                ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
                                    ui.label(
                                        egui::RichText::new(item.title)
                                            .size(12.0)
                                            .color(colors.text),
                                    );
                                    if let Some(desc) = item.description {
                                        ui.label(
                                            egui::RichText::new(desc)
                                                .size(11.0)
                                                .color(colors.overlay1),
                                        );
                                    }
                                });
                            });
                        });
                        ui.add_space(8.0);
                    });
                    alloc.response
                })
                .inner;

            // Persist hover for next frame.
            let is_hovered = ui.rect_contains_pointer(row_resp.rect);
            ui.ctx()
                .memory_mut(|m| m.data.insert_temp(item_id, is_hovered));

            // Hover background drawn behind the row.
            if is_hovered || was_hovered {
                ui.painter().rect_filled(
                    row_resp.rect,
                    2.0,
                    egui::Color32::from_rgba_premultiplied(
                        colors.surface2.r(),
                        colors.surface2.g(),
                        colors.surface2.b(),
                        30,
                    ),
                );
            }

            if btn_clicked {
                action_clicked = Some(idx);
            }

            if idx < props.items.len() - 1 {
                ui.separator();
            }
        }

        ListOutput { action_clicked }
    }
}
