use std::path::PathBuf;

use eframe::egui::{self, CornerRadius, Sense, Vec2};

use crate::{
    components::{
        common::button::{Button, ButtonProps},
        icon_button::{IconButton, IconButtonProps},
        traits::StatelessComponent,
    },
    theme::{ThemeColors, icon_rich_text, phosphor_font_id},
};

// Row height constants — must match what `render_row` actually draws.
// top-pad(8) + label(~15) + bottom-pad(8) = 31, rounded up with item_spacing.
const ROW_H: f32 = 36.0;
// top-pad(8) + label(~15) + desc(~13) + spacing(~2) + bottom-pad(8) = 46.
const ROW_H_DESC: f32 = 50.0;
// two-line description: adds a second 13px line + 5px gap between lines.
const ROW_H_DESC2: f32 = 70.0;
// top-pad(8) + icon-tile(32) + bottom-pad(8) = 48.
const ROW_H_TILE: f32 = 48.0;
// Compact strip rows (category filters, nav items).
const ROW_H_COMPACT: f32 = 26.0;
// Extra height added when a row has category tags (4px gap + 16px pill row).
const ROW_H_TAGS_ADD: f32 = 20.0;
// egui separator height.
const SEP_H: f32 = 1.5;

fn item_height(item: &ListItem<'_>, compact: bool) -> f32 {
    if compact {
        return ROW_H_COMPACT;
    }
    let tile = matches!(item.prefix, Some(ListItemPrefix::IconTile { .. }));
    let two_line = item.description.is_some_and(|d| d.contains('\n'));
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

// ── Prefix ────────────────────────────────────────────────────────────────────

/// Widget rendered to the left of each row's content area with consistent padding.
pub enum ListItemPrefix<'a> {
    /// A single phosphor icon glyph. `color: None` falls back to the theme `fg_muted`.
    Icon {
        glyph: &'a str,
        color: Option<egui::Color32>,
    },
    /// A 32 × 32 rounded-square tile with a phosphor icon glyph centred inside.
    /// Use this for richer leading visuals such as plugin icons or avatars.
    IconTile {
        glyph: &'a str,
        /// Accent color — used for the tile background (at low opacity) and the glyph.
        color: egui::Color32,
    },

    IconFile {
        path: PathBuf,
    },
}

// ── Postfix ───────────────────────────────────────────────────────────────────

/// Widget rendered on the right side of the title row, always visible
/// (unlike `action` buttons which are hover-revealed).
pub enum ListItemPostfix<'a> {
    /// A small pill badge — e.g. an install state indicator.
    Badge {
        text: &'a str,
        bg: egui::Color32,
        fg: egui::Color32,
    },
    /// A full Button. Click is reported via `ListOutput::postfix_clicked`.
    ActionButton(ButtonProps),
    IconButton(IconButtonProps<'a>),
    /// A thin progress bar (0–100). 80 px wide, 4 px tall, `colors.info` fill.
    ProgressBar(u8),
}

// ── List item ─────────────────────────────────────────────────────────────────

/// A colored badge shown *before* the title (e.g. HTTP method labels).
pub struct ListItemBadge<'a> {
    pub text: &'a str,
    pub color: egui::Color32,
    pub text_color: egui::Color32,
}

/// A single item in the list.
pub struct ListItem<'a> {
    /// Primary label.
    pub title: &'a str,
    /// Optional secondary description (muted, smaller).
    pub description: Option<&'a str>,
    /// Optional leading element rendered before the content area.
    pub prefix: Option<ListItemPrefix<'a>>,
    /// Optional colored badge shown *before* the title text (e.g. HTTP method).
    pub badge: Option<ListItemBadge<'a>>,
    /// Optional always-visible element on the right of the title.
    pub postfix: Option<ListItemPostfix<'a>>,
    /// Persistent highlight — used for active/selected state (e.g. category strip).
    pub selected: bool,
    /// Optional left accent border color. Pass `Some(color)` to draw a 2 px strip
    /// on the left edge in the given color (e.g. per-notification-kind color).
    /// Drawn for non-compact rows only; independent of `selected`.
    pub accent: Option<egui::Color32>,
    /// Optional category/tag pills rendered below the description row.
    pub tags: &'a [&'a str],
}

// ── Output / props ────────────────────────────────────────────────────────────

pub struct ListOutput {
    /// Index of the row that was clicked (outside any postfix button).
    pub row_clicked: Option<usize>,
    /// Index of the item whose postfix `ActionButton` was clicked.
    pub postfix_clicked: Option<usize>,
}

pub struct ListProps<'a> {
    pub items: &'a [ListItem<'a>],
    /// Text shown when `items` is empty.
    pub empty_label: Option<&'a str>,
    /// Shrink the scroll area to the content height instead of filling available space.
    /// Use `true` for inline strips (e.g. category filters); `false` (default) for sidebar lists.
    pub shrink_to_fit: bool,
    /// Draw a separator line between rows. Defaults to `true`.
    pub show_separators: bool,
    /// Use compact 26 px rows — no description or tile prefix support.
    /// Intended for navigation / category strips.
    pub compact: bool,
    /// Cap the scroll area at this height (px) and scroll beyond it.
    /// `None` (default) lets the list grow to fit all its content.
    /// Pass `Some(h)` for large or lazily-loaded lists (e.g. search results).
    pub max_height: Option<f32>,
}

// ── Component ─────────────────────────────────────────────────────────────────

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

        let mut row_clicked: Option<usize> = None;
        let mut postfix_clicked: Option<usize> = None;

        if props.items.is_empty() {
            ui.add_space(12.0);
            ui.vertical_centered(|ui| {
                ui.label(
                    egui::RichText::new(props.empty_label.unwrap_or("No items"))
                        .color(colors.fg_muted)
                        .size(12.0),
                );
            });
            ui.add_space(12.0);
            return ListOutput {
                row_clicked,
                postfix_clicked,
            };
        }

        let n = props.items.len();
        let show_sep = props.show_separators;
        let compact = props.compact;

        // Pre-compute cumulative Y offsets for virtual scrolling.
        let mut offsets = Vec::with_capacity(n + 1);
        offsets.push(0.0f32);
        for (i, item) in props.items.iter().enumerate() {
            let sep = if show_sep && i + 1 < n { SEP_H } else { 0.0 };
            offsets.push(offsets[i] + item_height(item, compact) + sep);
        }
        let total_h = offsets[n];

        // next_auto_id() gives a call-site-stable unique ID, so two List renders
        // in the same parent UI get different scroll IDs and different item IDs.
        let scroll_id = ui.next_auto_id();

        let mut scroll = egui::ScrollArea::vertical()
            .id_salt(scroll_id)
            .auto_shrink([false, props.shrink_to_fit]);
        if let Some(h) = props.max_height {
            scroll = scroll.max_height(h);
        }
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
                    let item = &props.items[idx];
                    // Key off scroll_id, not ui.id(), so items in sibling lists
                    // (same parent UI) never share hover-state storage keys.
                    let item_id = scroll_id.with(idx);
                    let row_h = item_height(item, compact);

                    let was_hovered = ui
                        .ctx()
                        .memory(|m| m.data.get_temp::<bool>(item_id).unwrap_or(false));

                    let mut postfix_btn_clicked = false;

                    // Reserve a paint slot BEFORE the row content so the
                    // background is always drawn behind badges and icons.
                    // painter.set() fills it in after we know the rect + state.
                    let bg_slot = ui.painter().add(egui::Shape::Noop);

                    let row_resp = ui
                        .push_id(item_id, |ui| {
                            let avail_width = ui.available_width();
                            let alloc = ui.allocate_ui(egui::vec2(avail_width, row_h), |ui| {
                                ui.horizontal(|ui| {
                                        ui.set_min_width(ui.available_width());
                                        ui.set_min_height(row_h);

                                        // ── Prefix ───────────────────────────────
                                        match &item.prefix {
                                            Some(ListItemPrefix::Icon { glyph, color }) => {
                                                ui.add_space(8.0);
                                                let c = color.unwrap_or(colors.fg_muted);
                                                ui.label(icon_rich_text(glyph, 13.0).color(c));
                                                ui.add_space(4.0);
                                            }
                                            Some(ListItemPrefix::IconTile { glyph, color }) => {
                                                ui.add_space(8.0);
                                                let tile_bg_slot =
                                                    ui.painter().add(egui::Shape::Noop);
                                                let (rect, _) = ui.allocate_exact_size(
                                                    egui::vec2(32.0, 32.0),
                                                    egui::Sense::hover(),
                                                );
                                                ui.painter().text(
                                                    rect.center(),
                                                    egui::Align2::CENTER_CENTER,
                                                    *glyph,
                                                    phosphor_font_id(14.0),
                                                    *color,
                                                );
                                                ui.painter().set(
                                                    tile_bg_slot,
                                                    egui::Shape::rect_filled(
                                                        rect,
                                                        7.0,
                                                        color.linear_multiply(0.15),
                                                    ),
                                                );
                                                ui.add_space(8.0);
                                            }
                                            Some(ListItemPrefix::IconFile { path }) => {
                                                ui.add_space(8.0);
                                                let (rect, _) = ui.allocate_exact_size(
                                                    Vec2::new(48.0, 48.0),
                                                    Sense::hover(),
                                                );
                                                if let Some(texture) =
                                                    super::helpers::load_icon_texture(
                                                        ui.ctx(),
                                                        path,
                                                        "list_icon",
                                                    )
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
                                            None => {}
                                        }

                                        // ── Content + postfix + actions ──────────
                                        ui.with_layout(
                                            egui::Layout::right_to_left(egui::Align::Center),
                                            |ui| {
                                                // Postfix (always visible)
                                                if let Some(postfix) = &item.postfix {
                                                    match postfix {
                                                        ListItemPostfix::Badge { text, bg, fg } => {
                                                            ui.add_space(6.0);
                                                            let badge_bg_slot =
                                                                ui.painter().add(egui::Shape::Noop);
                                                            let galley =
                                                                ui.painter().layout_no_wrap(
                                                                    text.to_string(),
                                                                    egui::FontId::proportional(
                                                                        10.0,
                                                                    ),
                                                                    *fg,
                                                                );
                                                            let pad_x = 6.0_f32;
                                                            let pad_y = 2.0_f32;
                                                            let badge_size = egui::vec2(
                                                                galley.size().x + pad_x * 2.0,
                                                                galley.size().y + pad_y * 2.0,
                                                            );
                                                            let (rect, _) = ui.allocate_exact_size(
                                                                badge_size,
                                                                egui::Sense::hover(),
                                                            );
                                                            if ui.is_rect_visible(rect) {
                                                                ui.painter().galley(
                                                                    rect.min
                                                                        + egui::vec2(pad_x, pad_y),
                                                                    galley,
                                                                    *fg,
                                                                );
                                                                ui.painter().set(
                                                                    badge_bg_slot,
                                                                    egui::Shape::rect_filled(
                                                                        rect,
                                                                        u8::MAX,
                                                                        *bg,
                                                                    ),
                                                                );
                                                            }
                                                        }
                                                        ListItemPostfix::ActionButton(props) => {
                                                            ui.add_space(8.0);
                                                            let out =
                                                                Button::render(ui, props.clone());
                                                            if out.clicked {
                                                                postfix_btn_clicked = true;
                                                            }
                                                        }
                                                        ListItemPostfix::IconButton(props) => {
                                                            ui.add_space(8.0);
                                                            let out = IconButton::render(
                                                                ui,
                                                                IconButtonProps { ..*props },
                                                            );
                                                            if out.clicked {
                                                                postfix_btn_clicked = true;
                                                            }
                                                        }
                                                        ListItemPostfix::ProgressBar(pct) => {
                                                            ui.add_space(8.0);
                                                            let (track, _) = ui
                                                                .allocate_exact_size(
                                                                    egui::vec2(80.0, 4.0),
                                                                    egui::Sense::hover(),
                                                                );
                                                            ui.painter().rect_filled(
                                                                track,
                                                                2.0,
                                                                colors.surface,
                                                            );
                                                            let fill = egui::Rect::from_min_size(
                                                                track.min,
                                                                egui::vec2(
                                                                    track.width()
                                                                        * (*pct as f32 / 100.0),
                                                                    4.0,
                                                                ),
                                                            );
                                                            ui.painter().rect_filled(
                                                                fill,
                                                                2.0,
                                                                colors.info,
                                                            );
                                                        }
                                                    }
                                                }

                                                // Title + badge + description.
                                                // Explicitly bounded to the remaining width so the
                                                // postfix always has its own reserved space at the end.
                                                let content_w = ui.available_width();
                                                ui.allocate_ui_with_layout(
                                                    egui::vec2(content_w, row_h),
                                                    egui::Layout::top_down(egui::Align::LEFT),
                                                    |ui| {
                                                        // Vertically centre content within row_h.
                                                        let title_h: f32 = 15.0;
                                                        let two_line = item
                                                            .description
                                                            .is_some_and(|d| d.contains('\n'));
                                                        let desc_h: f32 = if two_line {
                                                            2.0 + 13.0 + 5.0 + 13.0
                                                        } else if item.description.is_some() {
                                                            2.0 + 13.0
                                                        } else {
                                                            0.0
                                                        };
                                                        let tags_h: f32 = if item.tags.is_empty() {
                                                            0.0
                                                        } else {
                                                            ROW_H_TAGS_ADD
                                                        };
                                                        let content_h = title_h + desc_h + tags_h;
                                                        let pad =
                                                            (row_h - content_h).max(0.0) / 2.0;
                                                        if pad > 0.0 {
                                                            ui.add_space(pad);
                                                        }
                                                        ui.horizontal(|ui| {
                                                            ui.spacing_mut().item_spacing.x = 5.0;
                                                            if let Some(badge) = &item.badge {
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
                                                                    egui::FontId::proportional(
                                                                        10.0,
                                                                    ),
                                                                    badge.text_color,
                                                                );
                                                            }
                                                            // Compact nav rows: dim inactive,
                                                            // brighten active — matches design's
                                                            // overlay1 vs text color distinction.
                                                            let title_color =
                                                                if compact && !item.selected {
                                                                    colors.fg_muted
                                                                } else {
                                                                    colors.fg
                                                                };
                                                            let title_rt =
                                                                egui::RichText::new(item.title)
                                                                    .size(12.0)
                                                                    .color(title_color);
                                                            ui.add(
                                                                egui::Label::new(
                                                                    if item.selected && compact {
                                                                        title_rt.strong()
                                                                    } else {
                                                                        title_rt
                                                                    },
                                                                )
                                                                .truncate(),
                                                            );
                                                        });
                                                        if let Some(desc) = item.description {
                                                            let mut parts = desc.splitn(2, '\n');
                                                            if let Some(first) = parts.next() {
                                                                ui.add(
                                                                    egui::Label::new(
                                                                        egui::RichText::new(first)
                                                                            .size(11.0)
                                                                            .color(colors.fg_muted),
                                                                    )
                                                                    .truncate(),
                                                                );
                                                            }
                                                            if let Some(second) = parts.next() {
                                                                ui.add_space(3.0);
                                                                ui.add(
                                                                    egui::Label::new(
                                                                        egui::RichText::new(second)
                                                                            .size(11.0)
                                                                            .color(colors.fg_muted),
                                                                    )
                                                                    .truncate(),
                                                                );
                                                            }
                                                        }
                                                        if !item.tags.is_empty() {
                                                            ui.add_space(4.0);
                                                            ui.horizontal_wrapped(|ui| {
                                                                ui.spacing_mut().item_spacing.x =
                                                                    4.0;
                                                                ui.spacing_mut().item_spacing.y =
                                                                    2.0;
                                                                for tag in item.tags {
                                                                    let galley = ui
                                                                        .painter()
                                                                        .layout_no_wrap(
                                                                        tag.to_string(),
                                                                        egui::FontId::proportional(
                                                                            10.0,
                                                                        ),
                                                                        colors.fg_muted,
                                                                    );
                                                                    let pad_x = 5.0_f32;
                                                                    let pad_y = 2.0_f32;
                                                                    let tag_size = egui::vec2(
                                                                        galley.size().x
                                                                            + pad_x * 2.0,
                                                                        galley.size().y
                                                                            + pad_y * 2.0,
                                                                    );
                                                                    let (rect, _) = ui
                                                                        .allocate_exact_size(
                                                                            tag_size,
                                                                            egui::Sense::hover(),
                                                                        );
                                                                    if ui.is_rect_visible(rect) {
                                                                        ui.painter().rect_filled(
                                                                            rect,
                                                                            3.0,
                                                                            colors.bg_sunken,
                                                                        );
                                                                        ui.painter().galley(
                                                                            rect.min
                                                                                + egui::vec2(
                                                                                    pad_x, pad_y,
                                                                                ),
                                                                            galley,
                                                                            colors.fg_muted,
                                                                        );
                                                                    }
                                                                }
                                                            });
                                                        }
                                                    },
                                                );
                                            },
                                        );
                                    });
                            });
                            alloc.response
                        })
                        .inner;

                    let is_hovered = ui.rect_contains_pointer(row_resp.rect);
                    if is_hovered {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                    }
                    ui.ctx()
                        .memory_mut(|m| m.data.insert_temp(item_id, is_hovered));

                    // ── Row background — painted into the pre-reserved slot ──
                    // Because bg_slot was inserted before the row content, this
                    // always renders behind badges, icons, and text.
                    let is_hovering = is_hovered || was_hovered;
                    let bg_shape = if item.selected && is_hovering {
                        egui::Shape::rect_filled(row_resp.rect, 3.0, colors.surface_raised)
                    } else if item.selected || is_hovering {
                        egui::Shape::rect_filled(row_resp.rect, 3.0, colors.sidebar_hover)
                    } else {
                        egui::Shape::Noop
                    };
                    ui.painter().set(bg_slot, bg_shape);

                    // Left accent border — driven by the `accent` prop, not `selected`,
                    // so callers control both color and presence independently.
                    if let Some(accent_color) = item.accent
                        && !compact
                    {
                        let border = egui::Rect::from_min_size(
                            row_resp.rect.min,
                            egui::vec2(2.0, row_resp.rect.height()),
                        );
                        ui.painter().rect_filled(border, 0.0, accent_color);
                    }

                    if postfix_btn_clicked {
                        postfix_clicked = Some(idx);
                    } else if is_hovered && ui.input(|i| i.pointer.primary_clicked()) {
                        row_clicked = Some(idx);
                    }

                    if show_sep && idx + 1 < n {
                        ui.separator();
                    }
                }

                let remaining = total_h - offsets[end];
                if remaining > 0.0 {
                    ui.add_space(remaining);
                }
            });

        ListOutput {
            row_clicked,
            postfix_clicked,
        }
    }
}
