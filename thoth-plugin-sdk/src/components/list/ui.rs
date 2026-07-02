use egui::{Align, Align2, CornerRadius, FontId, Layout, RichText, Sense, Vec2};

use crate::components::helpers::load_icon_texture;
use crate::theme::{ThemeColors, get_contrast_text_color, phosphor_font_id, resolve_color};

use super::{List, ListItem, ListItemPostfix, ListItemPrefix};

// Row heights — must match what a row actually draws.
const ROW_H: f32 = 36.0; // title only
const ROW_H_DESC: f32 = 50.0; // title + description
const ROW_H_DESC2: f32 = 70.0; // two-line description
const ROW_H_TILE: f32 = 48.0; // icon-tile / file-icon prefix
const ROW_H_COMPACT: f32 = 26.0; // compact strip rows
const ROW_H_TAGS_ADD: f32 = 20.0; // extra height for a tags row
const SEP_H: f32 = 1.5;

/// What the user did in a [`List`] this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListEvent {
    /// Row `index` was clicked (outside any action/postfix).
    ItemClicked(usize),
    /// A hover-revealed `action` on row `item` was clicked.
    ActionClicked {
        /// Row index.
        item: usize,
        /// Action index within that row.
        action: usize,
    },
    /// Row `index`'s postfix button was clicked.
    PostfixClicked(usize),
}

fn item_height(item: &ListItem, compact: bool) -> f32 {
    if compact {
        return ROW_H_COMPACT;
    }
    let tile = matches!(
        item.prefix,
        Some(ListItemPrefix::IconTile { .. }) | Some(ListItemPrefix::IconFile { .. })
    );
    let two_line = item
        .description
        .as_deref()
        .is_some_and(|d| d.contains('\n'));
    let tags_h = if item.tags.is_empty() {
        0.0
    } else {
        ROW_H_TAGS_ADD
    };
    let base = if two_line {
        ROW_H_DESC2
    } else {
        match (tile, item.description.is_some()) {
            (true, true) => ROW_H_TILE.max(ROW_H_DESC),
            (true, false) => ROW_H_TILE,
            (false, true) => ROW_H_DESC,
            (false, false) => ROW_H,
        }
    };
    base + tags_h
}

impl List {
    /// Render the list. Returns the user's action this frame, if any.
    pub fn show(&self, ui: &mut egui::Ui) -> Option<ListEvent> {
        let colors = ThemeColors::from_ctx(ui.ctx());
        if self.framed {
            egui::Frame::new()
                .fill(colors.bg_panel)
                .stroke(egui::Stroke::new(1.0, colors.surface))
                .corner_radius(6)
                .inner_margin(egui::Margin::same(4))
                .outer_margin(egui::Margin::same(8))
                .show(ui, |ui| self.render(ui, colors))
                .inner
        } else {
            self.render(ui, colors)
        }
    }

    fn render(&self, ui: &mut egui::Ui, colors: ThemeColors) -> Option<ListEvent> {
        if self.items.is_empty() {
            ui.add_space(12.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(self.empty_label.as_deref().unwrap_or("No items"))
                        .color(colors.fg_muted)
                        .size(12.0),
                );
            });
            ui.add_space(12.0);
            return None;
        }

        let n = self.items.len();
        let compact = self.compact;
        let show_sep = self.show_separators;

        // Cumulative Y offsets for virtual scrolling.
        let mut offsets = Vec::with_capacity(n + 1);
        offsets.push(0.0f32);
        for (i, item) in self.items.iter().enumerate() {
            let sep = if show_sep && i + 1 < n { SEP_H } else { 0.0 };
            offsets.push(offsets[i] + item_height(item, compact) + sep);
        }
        let total_h = offsets[n];

        let scroll_id = ui.next_auto_id();
        let mut scroll = egui::ScrollArea::vertical()
            .id_salt(scroll_id)
            .auto_shrink([false, self.shrink_to_fit]);
        if let Some(h) = self.max_height {
            scroll = scroll.max_height(h);
        }

        let mut event = None;

        scroll.show_viewport(ui, |ui, viewport| {
            ui.set_min_height(total_h);
            let start = offsets
                .partition_point(|&y| y < viewport.min.y)
                .saturating_sub(1);
            let end = offsets.partition_point(|&y| y <= viewport.max.y).min(n);
            if offsets[start] > 0.0 {
                ui.add_space(offsets[start]);
            }

            for idx in start..end {
                let item = &self.items[idx];
                let item_id = scroll_id.with(idx);
                let row_h = item_height(item, compact);
                let was_hovered = ui
                    .ctx()
                    .memory(|m| m.data.get_temp::<bool>(item_id).unwrap_or(false));

                let mut postfix_clicked = false;
                let mut row_action_clicked: Option<usize> = None;

                // Reserve a paint slot before the content so the background draws
                // behind icons, badges, and text.
                let bg_slot = ui.painter().add(egui::Shape::Noop);

                let row_resp = ui
                    .push_id(item_id, |ui| {
                        let avail_w = ui.available_width();
                        ui.allocate_ui(egui::vec2(avail_w, row_h), |ui| {
                            ui.horizontal(|ui| {
                                ui.set_min_width(ui.available_width());
                                ui.set_min_height(row_h);
                                Self::row_prefix(ui, item, &colors);
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    Self::row_postfix(
                                        ui,
                                        item,
                                        &colors,
                                        &mut postfix_clicked,
                                        &mut row_action_clicked,
                                    );
                                    Self::row_content(ui, item, &colors, compact, row_h);
                                });
                            });
                        })
                        .response
                    })
                    .inner;

                let is_hovered = ui.rect_contains_pointer(row_resp.rect);
                if is_hovered {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                ui.ctx()
                    .memory_mut(|m| m.data.insert_temp(item_id, is_hovered));

                // Row background.
                let hovering = is_hovered || was_hovered;
                let bg = if item.selected && hovering {
                    egui::Shape::rect_filled(row_resp.rect, 3.0, colors.surface_raised)
                } else if item.selected || hovering {
                    egui::Shape::rect_filled(row_resp.rect, 3.0, colors.sidebar_hover)
                } else {
                    egui::Shape::Noop
                };
                ui.painter().set(bg_slot, bg);

                // Left accent border.
                if !compact
                    && let Some(accent) = item
                        .accent
                        .as_deref()
                        .and_then(|c| resolve_color(c, &colors))
                {
                    let border = egui::Rect::from_min_size(
                        row_resp.rect.min,
                        egui::vec2(2.0, row_resp.rect.height()),
                    );
                    ui.painter().rect_filled(border, 0.0, accent);
                }

                if postfix_clicked {
                    event = Some(ListEvent::PostfixClicked(idx));
                } else if let Some(a) = row_action_clicked {
                    event = Some(ListEvent::ActionClicked {
                        item: idx,
                        action: a,
                    });
                } else if is_hovered && ui.input(|i| i.pointer.primary_clicked()) {
                    event = Some(ListEvent::ItemClicked(idx));
                }

                if show_sep && idx + 1 < n {
                    ui.add(crate::components::Separator::plain());
                }
            }

            let remaining = total_h - offsets[end];
            if remaining > 0.0 {
                ui.add_space(remaining);
            }
        });

        event
    }

    fn row_prefix(ui: &mut egui::Ui, item: &ListItem, colors: &ThemeColors) {
        match &item.prefix {
            Some(ListItemPrefix::Icon { glyph, color }) => {
                ui.add_space(8.0);
                let c = color
                    .as_deref()
                    .and_then(|c| resolve_color(c, colors))
                    .unwrap_or(colors.fg_muted);
                ui.label(RichText::new(glyph).font(phosphor_font_id(13.0)).color(c));
                ui.add_space(4.0);
            }
            Some(ListItemPrefix::IconTile { glyph, color }) => {
                ui.add_space(8.0);
                let c = resolve_color(color, colors).unwrap_or(colors.accent);
                let tile_slot = ui.painter().add(egui::Shape::Noop);
                let (rect, _) = ui.allocate_exact_size(egui::vec2(32.0, 32.0), Sense::hover());
                ui.painter().text(
                    rect.center(),
                    Align2::CENTER_CENTER,
                    glyph,
                    phosphor_font_id(14.0),
                    c,
                );
                ui.painter().set(
                    tile_slot,
                    egui::Shape::rect_filled(rect, 7.0, c.linear_multiply(0.15)),
                );
                ui.add_space(8.0);
            }
            Some(ListItemPrefix::IconFile { path }) => {
                ui.add_space(8.0);
                let (rect, _) = ui.allocate_exact_size(Vec2::new(48.0, 48.0), Sense::hover());
                if let Some(texture) =
                    load_icon_texture(ui.ctx(), std::path::Path::new(path), "list_icon")
                {
                    ui.put(
                        rect,
                        egui::Image::new(&texture)
                            .fit_to_exact_size(rect.size())
                            .corner_radius(CornerRadius::same(10)),
                    );
                }
                ui.add_space(8.0);
            }
            // Still inset so titles line up with prefixed rows.
            None => {
                ui.add_space(8.0);
            }
        }
    }

    fn row_postfix(
        ui: &mut egui::Ui,
        item: &ListItem,
        colors: &ThemeColors,
        postfix_clicked: &mut bool,
        row_action_clicked: &mut Option<usize>,
    ) {
        match &item.postfix {
            Some(ListItemPostfix::Badge { text, bg, fg }) => {
                ui.add_space(6.0);
                let bg_c = bg
                    .as_deref()
                    .and_then(|c| resolve_color(c, colors))
                    .unwrap_or(colors.accent_secondary);
                let fg_c = fg
                    .as_deref()
                    .and_then(|c| resolve_color(c, colors))
                    .unwrap_or_else(|| get_contrast_text_color(bg_c));
                let bg_slot = ui.painter().add(egui::Shape::Noop);
                let galley =
                    ui.painter()
                        .layout_no_wrap(text.clone(), FontId::proportional(10.0), fg_c);
                let pad = egui::vec2(6.0, 2.0);
                let size = galley.size() + pad * 2.0;
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                if ui.is_rect_visible(rect) {
                    ui.painter().galley(rect.min + pad, galley, fg_c);
                    ui.painter()
                        .set(bg_slot, egui::Shape::rect_filled(rect, u8::MAX, bg_c));
                }
            }
            Some(ListItemPostfix::Button(btn)) => {
                ui.add_space(8.0);
                if ui.add(btn.clone()).clicked() {
                    *postfix_clicked = true;
                }
            }
            Some(ListItemPostfix::IconButton(btn)) => {
                ui.add_space(8.0);
                if ui.add(btn.clone()).clicked() {
                    *postfix_clicked = true;
                }
            }
            Some(ListItemPostfix::Progress(bar)) => {
                ui.add_space(8.0);
                // Keep list bars compact; the Progress component fills the width
                // it's given and carries its own colour/height.
                ui.allocate_ui(egui::vec2(80.0, 6.0), |ui| {
                    ui.add(bar.clone());
                });
            }
            None => {}
        }

        // Hover-revealed trailing action icons (rightmost; iterate reversed so
        // action[0] ends up leftmost).
        for (a, action) in item.actions.iter().enumerate().rev() {
            ui.add_space(4.0);
            let hit = ui
                .add(
                    crate::components::IconButton::builder()
                        .icon(action.icon.as_str())
                        .maybe_tooltip(action.tooltip.as_deref())
                        .frame(false)
                        .size(22.0)
                        .build(),
                )
                .clicked();
            if hit {
                *row_action_clicked = Some(a);
            }
        }
    }

    fn row_content(
        ui: &mut egui::Ui,
        item: &ListItem,
        colors: &ThemeColors,
        compact: bool,
        row_h: f32,
    ) {
        let content_w = ui.available_width();
        ui.allocate_ui_with_layout(
            egui::vec2(content_w, row_h),
            Layout::top_down(Align::LEFT),
            |ui| {
                let title_h = 15.0;
                let two_line = item
                    .description
                    .as_deref()
                    .is_some_and(|d| d.contains('\n'));
                let desc_h = if two_line {
                    2.0 + 13.0 + 5.0 + 13.0
                } else if item.description.is_some() {
                    2.0 + 13.0
                } else {
                    0.0
                };
                let tags_h = if item.tags.is_empty() {
                    0.0
                } else {
                    ROW_H_TAGS_ADD
                };
                let pad = (row_h - (title_h + desc_h + tags_h)).max(0.0) / 2.0;
                if pad > 0.0 {
                    ui.add_space(pad);
                }

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 5.0;
                    if let Some(badge) = &item.badge {
                        let bg = badge
                            .color
                            .as_deref()
                            .and_then(|c| resolve_color(c, colors))
                            .unwrap_or(colors.accent_secondary);
                        let fg = get_contrast_text_color(bg);
                        let text_w = badge.text.len() as f32 * 6.0;
                        let (rect, _) =
                            ui.allocate_exact_size(egui::vec2(text_w + 8.0, 14.0), Sense::hover());
                        ui.painter().rect_filled(rect, 3.0, bg);
                        ui.painter().text(
                            rect.center(),
                            Align2::CENTER_CENTER,
                            &badge.text,
                            FontId::proportional(10.0),
                            fg,
                        );
                    }
                    let title_color = if compact && !item.selected {
                        colors.fg_muted
                    } else {
                        colors.fg
                    };
                    let title = RichText::new(&item.title).size(12.0).color(title_color);
                    let title = if item.selected && compact {
                        title.strong()
                    } else {
                        title
                    };
                    ui.add(egui::Label::new(title).truncate());
                });

                if let Some(desc) = &item.description {
                    let mut parts = desc.splitn(2, '\n');
                    if let Some(first) = parts.next() {
                        ui.add(
                            egui::Label::new(
                                RichText::new(first).size(11.0).color(colors.fg_muted),
                            )
                            .truncate(),
                        );
                    }
                    if let Some(second) = parts.next() {
                        ui.add_space(3.0);
                        ui.add(
                            egui::Label::new(
                                RichText::new(second).size(11.0).color(colors.fg_muted),
                            )
                            .truncate(),
                        );
                    }
                }

                if !item.tags.is_empty() {
                    ui.add_space(4.0);
                    ui.horizontal_wrapped(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(4.0, 2.0);
                        for tag in &item.tags {
                            let galley = ui.painter().layout_no_wrap(
                                tag.clone(),
                                FontId::proportional(10.0),
                                colors.fg_muted,
                            );
                            let pad = egui::vec2(5.0, 2.0);
                            let size = galley.size() + pad * 2.0;
                            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                            if ui.is_rect_visible(rect) {
                                ui.painter().rect_filled(rect, 3.0, colors.bg_sunken);
                                ui.painter().galley(rect.min + pad, galley, colors.fg_muted);
                            }
                        }
                    });
                }
            },
        );
    }
}
