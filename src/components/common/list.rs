use eframe::egui;

use crate::{
    components::{
        icon_button::{IconButton, IconButtonProps},
        traits::StatelessComponent,
    },
    theme::ThemeColors,
};

// Row height constants — must match what `render_row` actually draws.
// top-pad(8) + label(~15) + bottom-pad(8) = 31, rounded up with item_spacing.
const ROW_H: f32 = 36.0;
// top-pad(8) + label(~15) + desc(~13) + spacing(~2) + bottom-pad(8) = 46.
const ROW_H_DESC: f32 = 50.0;
// egui separator height.
const SEP_H: f32 = 1.5;

fn item_height(item: &ListItem<'_>) -> f32 {
    if item.description.is_some() {
        ROW_H_DESC
    } else {
        ROW_H
    }
}

/// A colored badge shown before the title.
pub struct ListItemBadge<'a> {
    pub text: &'a str,
    pub color: egui::Color32,
    pub text_color: egui::Color32,
}

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
    /// Optional colored badge shown before the title (e.g. HTTP method)
    pub badge: Option<ListItemBadge<'a>>,
    /// Optional action button revealed on hover
    pub action: Vec<ListItemAction<'a>>,
}

pub struct ListItemAction<'a> {
    pub icon: &'a str,
    pub tooltip: &'a str,
}

pub struct ListOutput {
    /// Index of the item whose action button was clicked, if any
    pub action_clicked: (Option<usize>, Option<usize>), // item idx, action idx
    /// Index of the row that was clicked (outside any action button)
    pub row_clicked: Option<usize>,
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

        let mut action_clicked: (Option<usize>, Option<usize>) = (None, None);
        let mut row_clicked: Option<usize> = None;

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
            return ListOutput {
                action_clicked,
                row_clicked,
            };
        }

        // Pre-compute cumulative Y offsets for every item so we can binary-search
        // the visible window without iterating all items.
        //
        // offsets[i]     = Y where item i starts
        // offsets[n]     = total content height
        let n = props.items.len();
        let mut offsets = Vec::with_capacity(n + 1);
        offsets.push(0.0f32);
        for (i, item) in props.items.iter().enumerate() {
            let sep = if i + 1 < n { SEP_H } else { 0.0 };
            offsets.push(offsets[i] + item_height(item) + sep);
        }
        let total_h = offsets[n];

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show_viewport(ui, |ui, viewport| {
                // Reserve the full content height so the scroll bar is correct.
                ui.set_min_height(total_h);

                // Binary-search the first item whose bottom edge is visible.
                let start = offsets
                    .partition_point(|&y| y < viewport.min.y)
                    .saturating_sub(1);
                // First item whose top edge is past the bottom of the viewport.
                let end = offsets.partition_point(|&y| y <= viewport.max.y).min(n);

                // Advance the cursor past all off-screen items above.
                if offsets[start] > 0.0 {
                    ui.add_space(offsets[start]);
                }

                // Render only visible items.
                for idx in start..end {
                    let item = &props.items[idx];
                    let item_id = ui.id().with(idx);

                    let was_hovered = ui
                        .ctx()
                        .memory(|m| m.data.get_temp::<bool>(item_id).unwrap_or(false));

                    let mut btn_clicked: Option<usize> = None;

                    let row_resp = ui
                        .push_id(item_id, |ui| {
                            let avail_width = ui.available_width();
                            let alloc = ui.allocate_ui(egui::vec2(avail_width, 0.0), |ui| {
                                ui.add_space(8.0);
                                ui.horizontal_top(|ui| {
                                    ui.set_min_width(ui.available_width());

                                    if let Some(icon) = item.icon {
                                        ui.add_space(8.0);
                                        let color = item.icon_color.unwrap_or(colors.overlay1);
                                        ui.label(egui::RichText::new(icon).size(14.0).color(color));
                                        ui.add_space(4.0);
                                    }

                                    ui.with_layout(
                                        egui::Layout::right_to_left(egui::Align::Min),
                                        |ui| {
                                            ui.add_space(8.0);
                                            for (action_idx, action) in
                                                item.action.iter().enumerate()
                                            {
                                                let resp = IconButton::render(
                                                    ui,
                                                    IconButtonProps {
                                                        icon: action.icon,
                                                        tooltip: Some(action.tooltip),
                                                        frame: true,
                                                        badge_color: None,
                                                        size: None,
                                                        disabled: false,
                                                    },
                                                );
                                                if resp.clicked {
                                                    btn_clicked = Some(action_idx);
                                                }
                                            }

                                            ui.with_layout(
                                                egui::Layout::top_down(egui::Align::LEFT),
                                                |ui| {
                                                    ui.horizontal(|ui| {
                                                        ui.spacing_mut().item_spacing.x = 5.0;
                                                        if let Some(badge) = &item.badge {
                                                            // Approximate text width: ~6px per char at size 10
                                                            let text_w =
                                                                badge.text.len() as f32 * 6.0;
                                                            let badge_size =
                                                                egui::vec2(text_w + 8.0, 14.0);
                                                            let (badge_rect, _) = ui
                                                                .allocate_exact_size(
                                                                    badge_size,
                                                                    egui::Sense::hover(),
                                                                );
                                                            ui.painter().rect_filled(
                                                                badge_rect,
                                                                3.0,
                                                                badge.color,
                                                            );
                                                            ui.painter().text(
                                                                badge_rect.center(),
                                                                egui::Align2::CENTER_CENTER,
                                                                badge.text,
                                                                egui::FontId::proportional(10.0),
                                                                badge.text_color,
                                                            );
                                                        }
                                                        ui.label(
                                                            egui::RichText::new(item.title)
                                                                .size(12.0)
                                                                .color(colors.text),
                                                        );
                                                    });
                                                    if let Some(desc) = item.description {
                                                        ui.label(
                                                            egui::RichText::new(desc)
                                                                .size(11.0)
                                                                .color(colors.overlay1),
                                                        );
                                                    }
                                                },
                                            );
                                        },
                                    );
                                });
                                ui.add_space(8.0);
                            });
                            alloc.response
                        })
                        .inner;

                    // Persist hover for next frame.
                    let is_hovered = ui.rect_contains_pointer(row_resp.rect);
                    if is_hovered {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    ui.ctx()
                        .memory_mut(|m| m.data.insert_temp(item_id, is_hovered));

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

                    if let Some(clicked_idx) = btn_clicked {
                        action_clicked = (Some(idx), Some(clicked_idx));
                    } else if is_hovered && ui.input(|i| i.pointer.primary_clicked()) {
                        // Don't use ui.interact() here — it registers a new
                        // interactive region covering the whole row rect, which
                        // overlaps the action buttons and steals their clicks.
                        // Instead we check the raw pointer state directly.
                        row_clicked = Some(idx);
                    }

                    if idx + 1 < n {
                        ui.separator();
                    }
                }

                // Advance the cursor past all off-screen items below.
                let remaining = total_h - offsets[end];
                if remaining > 0.0 {
                    ui.add_space(remaining);
                }
            });

        ListOutput {
            action_clicked,
            row_clicked,
        }
    }
}
