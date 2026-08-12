use egui::{Align, Align2, CornerRadius, FontId, Layout, RichText, Sense, StrokeKind, Vec2};

use crate::components::helpers::load_icon_texture;
use crate::theme::{
    FONT_BODY, FONT_CAPTION, GUTTER_GAP, ICON_CONTROL, RADIUS_CHIP, RADIUS_CONTROL, RADIUS_PANEL,
    RADIUS_PILL, ROW_HEIGHT, ThemeColors, edge_stroke, phosphor_font_id, resolve_color, with_alpha,
};

use super::{List, ListItem, ListItemPostfix, ListItemPrefix, ListStyle};

// ── Row metrics ───────────────────────────────────────────────────────────────
//
// A row is *never* a fixed box. Design draws it as padding around the taller of
// its leading media and its text block, which is why the display sheet's
// `.li{height:48px}` and the app-mockup's padded `.card` are the same rule with
// different padding. Reproducing that rule (rather than pinning 48px) is what
// makes a title-only sidebar row short and a title + description row tall.

/// Compact strip rows — design `.dr{height:22px}`, the dense data-row shape.
const ROW_H_COMPACT: f32 = ROW_HEIGHT;

// Card rows — app-mockup `.card`, the sidebar shape.
/// `.card{padding:10px}`.
const CARD_PAD: f32 = 10.0;
/// `.card{margin-bottom:6px}` — cards are parted by a gap, never by a divider.
const CARD_GAP_Y: f32 = 6.0;
/// `.cscroll{padding:0 12px}`, pinned to the panel's content gutter so a card's
/// edge lines up with the `SidebarHeader` above it.
const CARD_INSET_X: f32 = GUTTER_GAP;
/// `.cmeta{gap:2px}`.
const CARD_TEXT_GAP: f32 = 2.0;
/// `.card::before{width:3px}`.
const CARD_STRIPE_W: f32 = 3.0;

// Flush rows — display `.li`, the rows of a framed `.list`.
/// `.li{padding:0 8px}`; also the vertical padding implied by
/// `.li{height:48px}` around a 32px tile.
const FLUSH_PAD: f32 = 8.0;
/// `.li .ld{margin-top:1px}`.
const FLUSH_TEXT_GAP: f32 = 1.0;
/// `.li.sel::before{width:2px}`.
const FLUSH_STRIPE_W: f32 = 2.0;

/// Gap between a row's elements — design `.li{gap:10px}` / `.card{gap:10px}`.
const GAP: f32 = 10.0;
/// …and in a compact strip — design `.dr{gap:5px}`.
const GAP_COMPACT: f32 = 5.0;
/// Framed container padding — design `.list{padding:4px}`.
const FRAME_PAD: i8 = 4;

/// Divider between adjacent flush rows — design
/// `.li+.li{box-shadow:inset 0 1px 0 surface1@24%}`.
const DIVIDER_ALPHA: u8 = 61; // 24% of 255
/// Hover wash — design `.li:hover{background:text@5%}`.
const HOVER_ALPHA: u8 = 13; // 5% of 255
/// Selected wash — design `.li.sel{background:accent@12%}`.
const SELECTED_ALPHA: u8 = 31; // 12% of 255
/// Compact rows follow the data row instead — design `.dr:hover{text@7%}`.
const HOVER_ALPHA_COMPACT: u8 = 18; // 7% of 255
/// …and `.dr.sel{background:accent@16%}`.
const SELECTED_ALPHA_COMPACT: u8 = 41; // 16% of 255

/// Accent stripe inset from the row's top and bottom edges — design
/// `top:9px;bottom:9px`.
const STRIPE_INSET_Y: f32 = 9.0;

/// Leading media box — design `.li .tile{width:32px;height:32px}`. Tiles,
/// host icons and embedded logos all share it, so every row with leading media
/// is the same shape.
const TILE: f32 = 32.0;
/// Tile tint — design `.tile{background:color-mix(accent 15%,transparent)}`.
const TILE_TINT_ALPHA: u8 = 38; // 15% of 255
/// Bare-glyph prefixes in a flush/compact row — design `.dr .lead{font-size:13px}`.
const PREFIX_GLYPH: f32 = 13.0;

/// Flush/compact row title — design `.li .lt{font-size:12px}` (a card row uses
/// the app-mockup's `.cn`, i.e. [`FONT_BODY`]).
const FONT_TITLE: f32 = 12.0;
/// Quietest text tier — design `.lbl{font-size:10.5px}`.
const FONT_META: f32 = 10.5;
/// Trailing chip — design `.li .lpost{font-size:10px;padding:2px 8px}`.
const POSTFIX_FONT: f32 = 10.0;
/// …and its padding.
const POSTFIX_PAD_X: f32 = 8.0;
/// …vertically.
const POSTFIX_PAD_Y: f32 = 2.0;
/// Leading badge before the title — app-mockup `.card .badge`, rendered with the
/// shared [`Badge`](crate::components::Badge) component at this size.
const BADGE_SIZE: crate::components::Size = crate::components::Size::Medium;
/// Gap between the leading badge and the title — design `.card{gap:10px}` is for
/// the row's top-level children; inside the title line the sheet uses 5px.
const BADGE_GAP: f32 = 5.0;
/// Tag pills — an SDK extension, drawn as app-mockup `.chip`
/// (`border-radius:6px;padding:0 6px;color:overlay1`).
const TAG_PAD_X: f32 = 6.0;
/// …with enough vertical padding to clear the 10px text.
const TAG_PAD_Y: f32 = 2.0;
/// Gap above the tag row, and between pills.
const TAG_GAP: f32 = 4.0;
/// Trailing icon actions — design `.li .lacts{gap:1px}`.
const ACTION_GAP: f32 = 1.0;
/// …each a ghost `.ib`: 24px square with a 14px glyph.
const ACTION_SIZE: f32 = 24.0;
/// …glyph size inside it.
const ACTION_GLYPH: f32 = 14.0;

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

/// A row title at a faux-bold weight, truncated to the width it is given —
/// design `.pl .nm{font-weight:600}` / `.nrow .nt{font-weight:600}`. egui ships
/// no bold face, so the galley is painted twice 0.5pt apart to thicken its
/// vertical strokes.
fn bold_title(ui: &mut egui::Ui, text: &str, size: f32, color: egui::Color32) {
    let mut job = egui::text::LayoutJob::single_section(
        text.to_owned(),
        egui::TextFormat {
            font_id: FontId::proportional(size),
            color,
            ..Default::default()
        },
    );
    // The same one-line-with-an-ellipsis shape `Label::truncate` produces.
    job.wrap.max_width = ui.available_width();
    job.wrap.max_rows = 1;
    job.wrap.break_anywhere = false;
    job.wrap.overflow_character = Some('…');
    let galley = ui.painter().layout_job(job);
    let (rect, _) = ui.allocate_exact_size(galley.size(), Sense::hover());
    if ui.is_rect_visible(rect) {
        ui.painter()
            .galley(rect.min + egui::vec2(0.5, 0.0), galley.clone(), color);
        ui.painter().galley(rect.min, galley, color);
    }
}

/// Every number a row draws with, resolved once per frame from the list's
/// [`ListStyle`]. Text line heights are measured from the live font set here so
/// per-row height stays plain arithmetic even for a list of thousands.
struct RowMetrics {
    /// Card rows paint their own fill and hairline edge; flush rows don't.
    card: bool,
    /// Compact rows are a fixed `.dr` height with no description or media.
    compact: bool,
    /// Inset of the row box from the list's full width.
    inset_x: f32,
    /// Padding inside the row box.
    pad: f32,
    /// Gap between the row's top-level elements.
    gap: f32,
    /// Gap below each row.
    gap_y: f32,
    /// Row box corner radius.
    radius: f32,
    /// Draw a hairline between adjacent rows.
    dividers: bool,
    /// Width of the left accent stripe.
    stripe_w: f32,
    /// Size of a bare leading glyph.
    prefix_glyph: f32,
    /// Title font size and its laid-out line height.
    title_font: f32,
    title_h: f32,
    /// Thicken every title — [`ListTextStyle::title_bold`].
    title_bold: bool,
    /// Gap above each line below the title.
    text_gap: f32,
    /// Description font size and the laid-out height of one line.
    desc_font: f32,
    desc_h: f32,
    /// Meta-line font size, whether it is monospace, and its laid-out height.
    meta_font: f32,
    meta_mono: bool,
    meta_h: f32,
    /// Height of the tag-pill row, including the gap above it. Tags are a single
    /// row (see [`List::row_content`]), so this is one pill tall.
    tags_h: f32,
    /// List-wide type-tier colour overrides, resolved once per frame. `None`
    /// leaves the tier on its own default.
    title_color: Option<egui::Color32>,
    desc_color: Option<egui::Color32>,
    meta_color: Option<egui::Color32>,
    /// Hover wash alpha over `fg` (unused by card rows, which go opaque).
    hover_alpha: u8,
    /// Selected wash alpha over `accent`.
    selected_alpha: u8,
}

impl RowMetrics {
    fn new(ui: &egui::Ui, list: &List) -> Self {
        let compact = list.compact;
        // `Auto`: a framed list holds flush `.li` rows, a bare one holds sidebar
        // `.card` rows. Compact strips are always the flush `.dr` shape.
        let card = match list.style {
            ListStyle::Card => !compact,
            ListStyle::Flush => false,
            ListStyle::Auto => !compact && !list.framed,
        };
        let line = |size: f32| ui.fonts_mut(|f| f.row_height(&FontId::proportional(size)));
        let mono_line = |size: f32| ui.fonts_mut(|f| f.row_height(&FontId::monospace(size)));
        // Type overrides layer on top of the row shape's own ramp; unset fields
        // leave every existing list exactly where it was.
        let style = &list.text_style;
        let title_font = style
            .title_size
            .unwrap_or(if card { FONT_BODY } else { FONT_TITLE });
        let desc_font = style.description_size.unwrap_or(FONT_CAPTION);
        let meta_font = style.meta_size.unwrap_or(FONT_META);
        let meta_mono = style.meta_mono;
        let colors = ThemeColors::from_ctx(ui.ctx());
        let tint =
            |token: &Option<String>| token.as_deref().and_then(|c| resolve_color(c, &colors));
        Self {
            card,
            compact,
            inset_x: if card { CARD_INSET_X } else { 0.0 },
            pad: if card { CARD_PAD } else { FLUSH_PAD },
            gap: if compact { GAP_COMPACT } else { GAP },
            gap_y: if card { CARD_GAP_Y } else { 0.0 },
            radius: if card { RADIUS_CONTROL } else { RADIUS_CHIP },
            dividers: list.show_separators && !card && !compact,
            stripe_w: if card { CARD_STRIPE_W } else { FLUSH_STRIPE_W },
            prefix_glyph: if card { ICON_CONTROL } else { PREFIX_GLYPH },
            title_font,
            title_h: line(title_font),
            title_bold: style.title_bold,
            text_gap: if card { CARD_TEXT_GAP } else { FLUSH_TEXT_GAP },
            desc_font,
            desc_h: line(desc_font),
            meta_font,
            meta_mono,
            meta_h: if meta_mono {
                mono_line(meta_font)
            } else {
                line(meta_font)
            },
            tags_h: TAG_GAP + line(POSTFIX_FONT) + TAG_PAD_Y * 2.0,
            title_color: tint(&style.title_color),
            desc_color: tint(&style.description_color),
            meta_color: tint(&style.meta_color),
            hover_alpha: if compact {
                HOVER_ALPHA_COMPACT
            } else {
                HOVER_ALPHA
            },
            selected_alpha: if compact {
                SELECTED_ALPHA_COMPACT
            } else {
                SELECTED_ALPHA
            },
        }
    }

    /// Height of a row's stacked text: title, description line(s), meta, tags.
    fn text_height(&self, item: &ListItem) -> f32 {
        let mut h = self.title_h;
        if self.compact {
            return h;
        }
        if let Some(desc) = &item.description {
            let lines = if desc.contains('\n') { 2.0 } else { 1.0 };
            h += (self.text_gap + self.desc_h) * lines;
        }
        if item.meta.is_some() {
            h += self.text_gap + self.meta_h;
        }
        if !item.tags.is_empty() {
            h += self.tags_h;
        }
        h
    }

    /// Total row height: padding around the taller of the row's leading media
    /// and its text block. For a flush row with a 32px tile plus a title and a
    /// description this is `8 + 32 + 8` — the design's `.li{height:48px}`.
    fn height(&self, item: &ListItem) -> f32 {
        if self.compact {
            return ROW_H_COMPACT;
        }
        self.pad * 2.0 + self.text_height(item).max(prefix_height(item))
    }
}

/// Height a row's leading element needs. Bare glyphs never out-measure the
/// title line they sit beside, so only media boxes count.
fn prefix_height(item: &ListItem) -> f32 {
    match &item.prefix {
        Some(
            ListItemPrefix::IconTile { .. }
            | ListItemPrefix::IconFile { .. }
            | ListItemPrefix::Image { .. },
        ) => TILE,
        _ => 0.0,
    }
}

impl List {
    /// Render the list. Returns the user's action this frame, if any.
    pub fn show(&self, ui: &mut egui::Ui) -> Option<ListEvent> {
        let colors = ThemeColors::from_ctx(ui.ctx());
        if self.framed {
            // Design `.list`: a panel-filled box with a hairline edge and 4px of
            // padding, so the first and last rows' hover fills stay inside it.
            egui::Frame::new()
                .fill(colors.bg_panel)
                .stroke(edge_stroke(&colors))
                .corner_radius(RADIUS_PANEL)
                .inner_margin(egui::Margin::same(FRAME_PAD))
                .outer_margin(egui::Margin::same(GUTTER_GAP as i8))
                .show(ui, |ui| self.render(ui, colors))
                .inner
        } else {
            self.render(ui, colors)
        }
    }

    fn render(&self, ui: &mut egui::Ui, colors: ThemeColors) -> Option<ListEvent> {
        let m = RowMetrics::new(ui, self);

        if self.items.is_empty() {
            ui.add_space(GAP);
            ui.vertical_centered(|ui| {
                ui.label(
                    RichText::new(self.empty_label.as_deref().unwrap_or("No items"))
                        .color(colors.fg_muted)
                        .size(m.title_font),
                );
            });
            ui.add_space(GAP);
            return None;
        }

        let n = self.items.len();

        // Cumulative Y offsets for virtual scrolling. A row's slot is its height
        // plus the gap below it (0 for flush rows, whose divider is an inset line
        // rather than a spacer).
        let mut offsets = Vec::with_capacity(n + 1);
        offsets.push(0.0f32);
        for (i, item) in self.items.iter().enumerate() {
            offsets.push(offsets[i] + m.height(item) + m.gap_y);
        }
        let total_h = offsets[n];

        // Design `.li+.li{box-shadow:inset 0 1px 0 surface1@24%}` — built once per
        // frame, then painted per visible row.
        let divider = crate::components::Separator::rule(crate::theme::color_to_hex(with_alpha(
            colors.surface_raised,
            DIVIDER_ALPHA,
        )));

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
            // Row spacing is explicit — either a card gap or nothing at all.
            ui.spacing_mut().item_spacing.y = 0.0;
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
                let row_h = m.height(item);
                let was_hovered = ui
                    .ctx()
                    .memory(|mem| mem.data.get_temp::<bool>(item_id).unwrap_or(false));

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
                                // Every gap inside a row is explicit.
                                ui.spacing_mut().item_spacing.x = 0.0;
                                Self::row_prefix(ui, item, &colors, &m);
                                ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                                    Self::row_postfix(
                                        ui,
                                        item,
                                        &colors,
                                        &m,
                                        was_hovered,
                                        &mut postfix_clicked,
                                        &mut row_action_clicked,
                                    );
                                    Self::row_content(ui, item, &colors, &m, row_h);
                                });
                            });
                        })
                        .response
                    })
                    .inner;

                // The painted row box — inset from the list's width for cards, so
                // they read as separate panels sitting in a gutter.
                let box_rect = row_resp.rect.shrink2(egui::vec2(m.inset_x, 0.0));

                let is_hovered = ui.rect_contains_pointer(box_rect);
                if is_hovered {
                    ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                }
                ui.ctx()
                    .memory_mut(|mem| mem.data.insert_temp(item_id, is_hovered));

                // Row background — design `.li.sel` wins over `.li:hover`.
                let hovering = is_hovered || was_hovered;
                let mut shapes: Vec<egui::Shape> = Vec::new();
                if m.card {
                    // Design `.card`: base fill lifted to `surface0` on hover,
                    // under a hairline edge.
                    let fill = if hovering { colors.surface } else { colors.bg };
                    shapes.push(egui::Shape::rect_filled(box_rect, m.radius, fill));
                    if item.selected {
                        shapes.push(egui::Shape::rect_filled(
                            box_rect,
                            m.radius,
                            with_alpha(colors.accent, m.selected_alpha),
                        ));
                    }
                    shapes.push(egui::Shape::rect_stroke(
                        box_rect,
                        m.radius,
                        edge_stroke(&colors),
                        StrokeKind::Inside,
                    ));
                } else if item.selected {
                    shapes.push(egui::Shape::rect_filled(
                        box_rect,
                        m.radius,
                        with_alpha(colors.accent, m.selected_alpha),
                    ));
                } else if hovering {
                    shapes.push(egui::Shape::rect_filled(
                        box_rect,
                        m.radius,
                        with_alpha(colors.fg, m.hover_alpha),
                    ));
                }
                ui.painter().set(bg_slot, egui::Shape::Vec(shapes));

                // Divider — an inset hairline along the top edge, between the rows
                // of a framed list only. Cards are already parted by a gap and
                // their own edge; a hairline there just adds noise. Drawn through
                // the shared `Separator`, without allocating: the row is already
                // laid out, and a point of height here would push it down.
                if m.dividers && idx > 0 {
                    divider.paint_at(ui, box_rect.x_range(), box_rect.top());
                }

                // Left accent stripe — design `.li.sel::before` / `.card::before`.
                // Selection wins over the row's own accent so the two can't
                // disagree about the colour.
                if !m.compact {
                    let stripe = if item.selected {
                        Some(colors.accent)
                    } else {
                        item.accent
                            .as_deref()
                            .and_then(|c| resolve_color(c, &colors))
                    };
                    if let Some(color) = stripe {
                        Self::left_stripe(ui, box_rect, color, m.stripe_w);
                    }
                }

                if m.gap_y > 0.0 {
                    ui.add_space(m.gap_y);
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
            }

            let remaining = total_h - offsets[end];
            if remaining > 0.0 {
                ui.add_space(remaining);
            }
        });

        event
    }

    /// A vertical bar on the row's left edge, inset from top and bottom. Its
    /// corner radius is its own width — design `width:2px;border-radius:2px`
    /// and `width:3px;border-radius:3px`.
    fn left_stripe(ui: &egui::Ui, row: egui::Rect, color: egui::Color32, width: f32) {
        let rect = egui::Rect::from_min_size(
            egui::pos2(row.left(), row.top() + STRIPE_INSET_Y),
            egui::vec2(width, (row.height() - STRIPE_INSET_Y * 2.0).max(0.0)),
        );
        ui.painter().rect_filled(rect, width, color);
    }

    fn row_prefix(ui: &mut egui::Ui, item: &ListItem, colors: &ThemeColors, m: &RowMetrics) {
        ui.add_space(m.inset_x + m.pad);
        match &item.prefix {
            Some(ListItemPrefix::Icon { glyph, color }) => {
                let c = color
                    .as_deref()
                    .and_then(|c| resolve_color(c, colors))
                    .unwrap_or(colors.fg_muted);
                ui.label(
                    RichText::new(glyph)
                        .font(phosphor_font_id(m.prefix_glyph))
                        .color(c),
                );
                ui.add_space(m.gap);
            }
            Some(ListItemPrefix::IconTile { glyph, color }) => {
                // Design `.li .tile`: a tinted square behind a centred glyph.
                let c = resolve_color(color, colors).unwrap_or(colors.accent);
                let (rect, _) = ui.allocate_exact_size(egui::vec2(TILE, TILE), Sense::hover());
                if ui.is_rect_visible(rect) {
                    ui.painter()
                        .rect_filled(rect, RADIUS_CONTROL, with_alpha(c, TILE_TINT_ALPHA));
                    ui.painter().text(
                        rect.center(),
                        Align2::CENTER_CENTER,
                        glyph,
                        phosphor_font_id(ICON_CONTROL),
                        c,
                    );
                }
                ui.add_space(m.gap);
            }
            Some(ListItemPrefix::IconFile { path }) => {
                let (rect, _) = ui.allocate_exact_size(Vec2::new(TILE, TILE), Sense::hover());
                if let Some(texture) =
                    load_icon_texture(ui.ctx(), std::path::Path::new(path), "list_icon")
                {
                    ui.put(
                        rect,
                        egui::Image::new(&texture)
                            .fit_to_exact_size(rect.size())
                            .corner_radius(CornerRadius::same(RADIUS_CONTROL as u8)),
                    );
                }
                ui.add_space(m.gap);
            }
            Some(ListItemPrefix::Image { uri, bytes }) => {
                // Fit within the media box preserving aspect ratio — logos aren't
                // square, so exact-fit would distort wide marks (e.g. MySQL).
                ui.add(
                    egui::Image::from_bytes(uri.clone(), bytes.clone())
                        .maintain_aspect_ratio(true)
                        .max_size(Vec2::new(TILE, TILE)),
                );
                ui.add_space(m.gap);
            }
            // The row's left padding above is enough — titles in prefix-less rows
            // line up with the padding, not with prefixed rows' text.
            None => {}
        }
    }

    fn row_postfix(
        ui: &mut egui::Ui,
        item: &ListItem,
        colors: &ThemeColors,
        m: &RowMetrics,
        hovering: bool,
        postfix_clicked: &mut bool,
        row_action_clicked: &mut Option<usize>,
    ) {
        // Right-to-left layout: this is the row's right-hand padding.
        ui.add_space(m.inset_x + m.pad);

        // Hover-revealed trailing action icons (rightmost; iterate reversed so
        // action[0] ends up leftmost). Design `.lacts{opacity:0}` /
        // `.li:hover .lacts{opacity:1}` — the space stays reserved either way so
        // the row's text doesn't reflow under the pointer.
        for (a, action) in item.actions.iter().enumerate().rev() {
            if a + 1 < item.actions.len() {
                ui.add_space(ACTION_GAP);
            }
            if hovering {
                let hit = ui
                    .add(
                        crate::components::IconButton::builder()
                            .icon(action.icon.as_str())
                            .maybe_tooltip(action.tooltip.as_deref())
                            .frame(false)
                            .size_px(ACTION_SIZE)
                            .icon_size(ACTION_GLYPH)
                            .build(),
                    )
                    .clicked();
                if hit {
                    *row_action_clicked = Some(a);
                }
            } else {
                ui.allocate_exact_size(egui::vec2(ACTION_SIZE, ACTION_SIZE), Sense::hover());
            }
        }
        if !item.actions.is_empty() && item.postfix.is_some() {
            ui.add_space(m.gap);
        }

        match &item.postfix {
            Some(ListItemPostfix::Badge { text, bg, fg }) => {
                // Design `.li .lpost`: a fully-round mono chip.
                let bg_c = bg
                    .as_deref()
                    .and_then(|c| resolve_color(c, colors))
                    .unwrap_or(colors.accent_secondary);
                let fg_c = fg
                    .as_deref()
                    .and_then(|c| resolve_color(c, colors))
                    .unwrap_or_else(|| crate::theme::get_contrast_text_color(bg_c));
                let bg_slot = ui.painter().add(egui::Shape::Noop);
                let galley = ui.painter().layout_no_wrap(
                    text.clone(),
                    FontId::monospace(POSTFIX_FONT),
                    fg_c,
                );
                let pad = egui::vec2(POSTFIX_PAD_X, POSTFIX_PAD_Y);
                let size = galley.size() + pad * 2.0;
                let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                if ui.is_rect_visible(rect) {
                    ui.painter().galley(rect.min + pad, galley, fg_c);
                    ui.painter().set(
                        bg_slot,
                        egui::Shape::rect_filled(rect, CornerRadius::same(RADIUS_PILL as u8), bg_c),
                    );
                }
            }
            Some(ListItemPostfix::Text { text, color, mono }) => {
                // Design `.cat .c`: a bare count with no chip chrome at all.
                let c = color
                    .as_deref()
                    .and_then(|c| resolve_color(c, colors))
                    .unwrap_or(colors.fg_muted);
                let font = if *mono {
                    FontId::monospace(POSTFIX_FONT)
                } else {
                    FontId::proportional(POSTFIX_FONT)
                };
                ui.add(egui::Label::new(RichText::new(text).font(font).color(c)));
            }
            Some(ListItemPostfix::Button(btn)) => {
                *postfix_clicked |= ui.add(btn.clone()).clicked();
            }
            Some(ListItemPostfix::IconButton(btn)) => {
                *postfix_clicked |= ui.add(btn.clone()).clicked();
            }
            Some(ListItemPostfix::Progress(bar)) => {
                // Keep list bars compact; the Progress component fills the width
                // it's given and carries its own colour/height.
                ui.allocate_ui(egui::vec2(80.0, 6.0), |ui| {
                    ui.add(bar.clone());
                });
            }
            None => {}
        }

        if item.postfix.is_some() || !item.actions.is_empty() {
            ui.add_space(m.gap);
        }
    }

    fn row_content(
        ui: &mut egui::Ui,
        item: &ListItem,
        colors: &ThemeColors,
        m: &RowMetrics,
        row_h: f32,
    ) {
        let content_w = ui.available_width();
        ui.allocate_ui_with_layout(
            egui::vec2(content_w, row_h),
            Layout::top_down(Align::LEFT),
            |ui| {
                // Design `.cmeta{gap:2px}` / `.ld{margin-top:1px}` — the text is
                // one tight block, vertically centred in the row.
                ui.spacing_mut().item_spacing.y = m.text_gap;
                let pad = (row_h - m.text_height(item)).max(0.0) / 2.0;
                if pad > 0.0 {
                    ui.add_space(pad);
                }

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = BADGE_GAP;
                    if let Some(badge) = &item.badge {
                        // App-mockup `.card .badge`: a filled mono chip — the
                        // shared Badge component, so list badges and standalone
                        // ones can't drift apart.
                        ui.add(
                            crate::components::Badge::builder()
                                .label(badge.text.clone())
                                .maybe_color(badge.color.clone())
                                .size(BADGE_SIZE)
                                .build(),
                        );
                    }
                    // A row's own colour wins, then the list's override, then
                    // the row shape's default.
                    let title_color = item
                        .title_color
                        .as_deref()
                        .and_then(|c| resolve_color(c, colors))
                        .or(m.title_color)
                        .unwrap_or(if m.compact && !item.selected {
                            colors.fg_muted
                        } else {
                            colors.fg
                        });
                    if m.title_bold {
                        // egui has no bold face, and `RichText::strong` only
                        // reaches for a colour we've already pinned — so weight
                        // is faked with a second 0.5pt-offset pass, as
                        // `Typography` does.
                        bold_title(ui, &item.title, m.title_font, title_color);
                    } else {
                        let title = RichText::new(&item.title)
                            .size(m.title_font)
                            .color(title_color);
                        let title = if item.selected && m.compact {
                            title.strong()
                        } else {
                            title
                        };
                        ui.add(egui::Label::new(title).truncate());
                    }
                });

                if m.compact {
                    return;
                }

                if let Some(desc) = &item.description {
                    let color = m.desc_color.unwrap_or(colors.fg_muted);
                    for line in desc.splitn(2, '\n') {
                        ui.add(
                            egui::Label::new(RichText::new(line).size(m.desc_font).color(color))
                                .truncate(),
                        );
                    }
                }

                // Third tier: one step quieter than the description.
                if let Some(meta) = &item.meta {
                    let color = m.meta_color.unwrap_or_else(|| colors.fg_faint());
                    let text = if m.meta_mono {
                        RichText::new(meta)
                            .font(FontId::monospace(m.meta_font))
                            .color(color)
                    } else {
                        RichText::new(meta).size(m.meta_font).color(color)
                    };
                    ui.add(egui::Label::new(text).truncate());
                }

                if !item.tags.is_empty() {
                    ui.add_space(TAG_GAP - m.text_gap);
                    // One row only: `RowMetrics::tags_h` reserves a single pill's
                    // height, so a wrapped second row would paint over the row
                    // below (virtual-scroll offsets come from that metric). Pills
                    // that don't fit the content width are dropped rather than
                    // wrapped — the first one always draws, however narrow the row.
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(TAG_GAP, TAG_PAD_Y);
                        for (i, tag) in item.tags.iter().enumerate() {
                            let galley = ui.painter().layout_no_wrap(
                                tag.clone(),
                                FontId::proportional(POSTFIX_FONT),
                                colors.fg_muted,
                            );
                            let pad = egui::vec2(TAG_PAD_X, TAG_PAD_Y);
                            let size = galley.size() + pad * 2.0;
                            if i > 0 && size.x > ui.available_width() {
                                break;
                            }
                            let (rect, _) = ui.allocate_exact_size(size, Sense::hover());
                            if ui.is_rect_visible(rect) {
                                ui.painter()
                                    .rect_filled(rect, RADIUS_CHIP, colors.bg_sunken);
                                ui.painter().galley(rect.min + pad, galley, colors.fg_muted);
                            }
                        }
                    });
                }
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{ListItem, ListItemPrefix};

    /// Run one headless frame and hand the closure a real `Ui`, so row metrics
    /// are measured against live font data like they are in the app.
    fn with_ui<R>(f: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let ctx = egui::Context::default();
        let mut f = Some(f);
        let mut out = None;
        let _ = ctx.run_ui(Default::default(), |ui| {
            if let Some(f) = f.take() {
                out = Some(f(ui));
            }
        });
        out.expect("the test frame ran")
    }

    fn tile_row() -> ListItem {
        ListItem::builder()
            .title("production.db")
            .description("PostgreSQL · 24 tables")
            .prefix(ListItemPrefix::IconTile {
                glyph: "d".to_string(),
                color: "accent".to_string(),
            })
            .build()
    }

    #[test]
    fn framed_tile_row_is_the_designs_48px() {
        // Design `.li{height:48px}` is 8px of padding around a 32px tile — the
        // content-driven rule has to reproduce it exactly.
        let list = List::builder().framed(true).build();
        let h = with_ui(|ui| RowMetrics::new(ui, &list).height(&tile_row()));
        assert_eq!(h, 48.0);
    }

    #[test]
    fn card_rows_grow_with_their_content() {
        let list = List::builder().build();
        let title_only = ListItem::builder().title("data.json").build();
        let with_desc = ListItem::builder()
            .title("data.json")
            .description("~/Documents")
            .build();
        let with_meta = ListItem::builder()
            .title("data.json")
            .description("~/Documents")
            .meta("2 minutes ago")
            .build();
        with_ui(|ui| {
            let m = RowMetrics::new(ui, &list);
            assert!(m.card, "a bare list uses the sidebar card shape");
            let (a, b, c) = (
                m.height(&title_only),
                m.height(&with_desc),
                m.height(&with_meta),
            );
            assert_eq!(a, CARD_PAD * 2.0 + m.title_h);
            assert!(
                a < b,
                "title-only rows must be shorter than titled+described"
            );
            assert!(b < c, "a meta line must add a line's height");
        });
    }

    #[test]
    fn compact_rows_are_the_dense_data_row() {
        let list = List::builder().compact(true).build();
        with_ui(|ui| {
            let m = RowMetrics::new(ui, &list);
            assert_eq!(m.height(&tile_row()), ROW_HEIGHT);
            assert!(!m.card, "a compact strip is never a stack of cards");
            assert!(!m.dividers, "design `.dr` rows carry no hairline");
        });
    }

    #[test]
    fn text_style_defaults_leave_every_tier_alone() {
        // The whole point of the override struct: a list that doesn't set one
        // measures exactly as it did before it existed.
        let plain = List::builder().framed(true).build();
        with_ui(|ui| {
            let m = RowMetrics::new(ui, &plain);
            assert_eq!(m.title_font, FONT_TITLE);
            assert_eq!(m.desc_font, FONT_CAPTION);
            assert_eq!(m.meta_font, FONT_META);
            assert!(!m.title_bold);
            assert!(!m.meta_mono);
            assert!(m.title_color.is_none());
            assert!(m.desc_color.is_none());
            assert!(m.meta_color.is_none());
        });
    }

    #[test]
    fn text_style_sizes_drive_the_row_height() {
        // Design `.pl`: a 12.5px title over an 11px description over a 10.5px
        // monospace author line. The taller ramp has to make the row taller.
        let styled = List::builder()
            .style(ListStyle::Flush)
            .text_style(
                crate::components::ListTextStyle::builder()
                    .title_size(20.0)
                    .title_bold(true)
                    .meta_mono(true)
                    .build(),
            )
            .build();
        let plain = List::builder().style(ListStyle::Flush).build();
        let row = ListItem::builder()
            .title("plugin")
            .description("does a thing")
            .meta("by someone")
            .build();
        with_ui(|ui| {
            let (a, b) = (RowMetrics::new(ui, &plain), RowMetrics::new(ui, &styled));
            assert_eq!(b.title_font, 20.0);
            assert!(b.title_bold);
            assert!(b.meta_mono);
            assert!(
                b.height(&row) > a.height(&row),
                "a bigger title tier must grow the row"
            );
        });
    }

    #[test]
    fn style_resolution_follows_the_lists_context() {
        let bare = List::builder().build();
        let framed = List::builder().framed(true).build();
        let forced_flush = List::builder().style(ListStyle::Flush).build();
        let forced_card = List::builder().framed(true).style(ListStyle::Card).build();
        let no_seps = List::builder().framed(true).show_separators(false).build();
        with_ui(|ui| {
            assert!(RowMetrics::new(ui, &bare).card);
            assert!(!RowMetrics::new(ui, &bare).dividers);
            assert!(!RowMetrics::new(ui, &framed).card);
            assert!(RowMetrics::new(ui, &framed).dividers);
            assert!(!RowMetrics::new(ui, &forced_flush).card);
            assert!(RowMetrics::new(ui, &forced_card).card);
            assert!(!RowMetrics::new(ui, &no_seps).dividers);
        });
    }
}
