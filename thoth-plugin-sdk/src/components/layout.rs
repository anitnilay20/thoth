//! Structural / layout components.
//!
//! These compose other [`RenderNode`]s rather than drawing a leaf widget, so —
//! like every other component — they're serializable for the plugin DSL and
//! buildable in Rust for the host. Each owns its children and renders them via
//! its `show(&mut self, ui)` method (which recurses into `RenderNode::show`).

use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::render_node::RenderNode;
#[cfg(feature = "egui")]
use crate::render_node::UiEvent;

/// Cross-axis alignment of a [`Row`]'s children.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Align {
    /// Pack at the start (left).
    #[default]
    Start,
    /// Center within available width.
    Center,
    /// Pack at the end (right).
    End,
    /// Distribute to fill the available width (prefix LTR, suffix RTL).
    Fill,
}

/// A semantic background-fill token, resolved against the active theme.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BgColor {
    /// No fill (transparent) — the default.
    #[default]
    None,
    /// Main app background (`bg`).
    Bg,
    /// Secondary panel background (`bg-panel`).
    BgPanel,
    /// Deepest inset background (`bg-sunken`).
    BgSunken,
    /// Resting widget surface (`surface`).
    Surface,
    /// Raised/hover surface (`surface-raised`).
    SurfaceRaised,
    /// Active/pressed surface (`surface-active`).
    SurfaceActive,
}

#[cfg(feature = "egui")]
impl BgColor {
    /// Resolve to a concrete colour, or `None` for [`BgColor::None`].
    fn resolve(self, c: &crate::theme::ThemeColors) -> Option<egui::Color32> {
        Some(match self {
            BgColor::None => return None,
            BgColor::Bg => c.bg,
            BgColor::BgPanel => c.bg_panel,
            BgColor::BgSunken => c.bg_sunken,
            BgColor::Surface => c.surface,
            BgColor::SurfaceRaised => c.surface_raised,
            BgColor::SurfaceActive => c.surface_active,
        })
    }
}

/// Lay children out left-to-right.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[non_exhaustive]
pub struct Row {
    /// Child nodes, in order.
    #[builder(default)]
    #[serde(default)]
    pub children: Vec<RenderNode>,
    /// Horizontal gap between children, in points.
    #[builder(default)]
    #[serde(default)]
    pub gap: f32,
    /// Inner padding around the row, in points.
    #[builder(default)]
    #[serde(default)]
    pub padding: f32,
    /// Cross-axis alignment of children.
    #[builder(default)]
    #[serde(default)]
    pub align: Align,
    /// Background fill token.
    #[builder(default)]
    #[serde(default, rename = "bg-color")]
    pub bg_color: BgColor,
    /// Stretch to the full available width.
    #[builder(default)]
    #[serde(default, rename = "max-width")]
    pub max_width: bool,
    /// Optional fixed height, in points.
    #[serde(default)]
    pub height: Option<f32>,
}

/// Lay children out top-to-bottom.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[non_exhaustive]
pub struct Column {
    /// Child nodes, in order.
    #[builder(default)]
    #[serde(default)]
    pub children: Vec<RenderNode>,
    /// Vertical gap between children, in points.
    #[builder(default)]
    #[serde(default)]
    pub gap: f32,
    /// Wrap the column in a bordered, filled card (panel background + surface
    /// border + rounded corners + margin). Defaults to `false`.
    #[builder(default)]
    #[serde(default)]
    pub framed: bool,
    /// Inner padding around the children, in points (matching [`Row::padding`]).
    /// Defaults to `0.0` — an unpadded column, which is what most callers want;
    /// set it for an inset region such as the design's `.tree{padding:4px}`.
    /// Nests *inside* [`framed`](Column::framed)'s card margin when both are set.
    ///
    /// egui margins are whole points (`i8`), so the value is truncated towards
    /// zero — `4.6` renders as 4 — and saturates at ±127. Values at or below
    /// `0.0` skip the padding frame entirely.
    #[builder(default)]
    #[serde(default)]
    pub padding: f32,
}

/// A scrollable region wrapping a single child.
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[non_exhaustive]
pub struct Scroll {
    /// The scrolled content.
    #[builder(into)]
    pub child: Box<RenderNode>,
    /// Optional fixed max height, in points.
    #[serde(default)]
    pub max_height: Option<f32>,
    /// Scroll horizontally as well as vertically. Defaults to `false`
    /// (vertical only).
    #[builder(default)]
    #[serde(default)]
    pub both: bool,
    /// Optional id salt to disambiguate this scroll area from sibling scroll
    /// areas (egui derives a scroll id from tree position, which can collide
    /// between two scrolls at equivalent positions, e.g. split columns).
    #[builder(into)]
    #[serde(default)]
    pub id: Option<String>,
}

/// Empty space of a fixed size, in points.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Builder)]
#[non_exhaustive]
pub struct Spacer {
    /// The amount of space.
    pub size: f32,
}

/// Proportional horizontal split. `widths` are relative weights (empty = equal
/// shares). A two-column split can opt into a draggable divider with
/// [`resizable`](Split::resizable), in which case `widths` only seeds the initial
/// ratio.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[non_exhaustive]
pub struct Split {
    /// Column nodes, in order.
    #[builder(default)]
    #[serde(default)]
    pub children: Vec<RenderNode>,
    /// Gap between columns, in points.
    #[builder(default)]
    #[serde(default)]
    pub gap: f32,
    /// Relative column weights; empty means equal shares.
    #[builder(default)]
    #[serde(default)]
    pub widths: Vec<f32>,
    /// Draw a vertical separator line between columns.
    #[builder(default)]
    #[serde(default)]
    pub separator: bool,
    /// Vertical alignment of each column's content within the row height.
    /// Defaults to [`Align::Start`] (top); [`Align::Center`] centers vertically.
    #[builder(default)]
    #[serde(default)]
    pub align: Align,
    /// When true, each column fills the full available height (so a column can
    /// hold a scroll region that fills the pane). Defaults to false — a
    /// content-height row, which is what tabular rows and form-field pairs want.
    #[builder(default)]
    #[serde(default)]
    pub fill_height: bool,
    /// When true, each column becomes a floating card — panel fill, hairline edge
    /// and [`RADIUS_PANEL`](crate::theme::RADIUS_PANEL) corners on the sunken
    /// gutter, matching [`VSplit`]'s panes.
    ///
    /// Off by default, and deliberately opt-in: most `Split`s lay out form-field
    /// pairs or table-like rows, which must stay flush. Turn it on only for a
    /// split whose columns are genuine resizable *regions* (a request/response
    /// pair, say).
    #[builder(default)]
    #[serde(default)]
    pub framed: bool,
    /// Let the user drag the gutter between the two columns to re-apportion the
    /// width — the horizontal twin of [`VSplit`]'s divider. The dragged ratio is
    /// persisted in egui memory, keyed by [`id`](Split::id).
    ///
    /// Off by default, for the same reason as [`framed`](Split::framed): a
    /// form-field pair or a table-like row must stay proportional and
    /// non-interactive. Only honoured for a *two*-column split — with three or
    /// more columns there is no single unambiguous divider, so the flag is
    /// ignored and the split stays purely proportional.
    #[builder(default)]
    #[serde(default)]
    pub resizable: bool,
    /// Stable id salt for the persisted divider position. Only consulted when
    /// [`resizable`](Split::resizable) is set; `None` (the default) falls back to
    /// egui's positional id for this `Ui`, which is enough while a view holds a
    /// single resizable split.
    #[builder(into)]
    #[serde(default)]
    pub id: Option<String>,
    /// Minimum width, in points, for each column of a [`resizable`](Split::resizable)
    /// split (keeps a drag from swallowing either side). Defaults to `120.0` —
    /// narrower than that and a pane stops being able to hold a toolbar. Ignored
    /// when the split is not resizable.
    #[builder(default = 120.0)]
    #[serde(default = "min_col")]
    pub min_pane: f32,
}

fn min_col() -> f32 {
    120.0
}

/// A vertical split with a draggable divider: `top` over `bottom`, each filling
/// its share of the available height so their content can scroll independently.
/// The divider position (top's fraction of the height) is dragged by the user
/// and persisted in egui memory, keyed by [`id`](VSplit::id).
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct VSplit {
    /// Stable id salt — must be unique per on-screen instance (persists the
    /// dragged divider position across frames).
    pub id: String,
    /// The top pane.
    #[builder(into)]
    pub top: Box<RenderNode>,
    /// The bottom pane.
    #[builder(into)]
    pub bottom: Box<RenderNode>,
    /// Initial fraction of the height given to `top` (0.0–1.0) before the user
    /// drags. Defaults to `0.5`.
    #[builder(default = 0.5)]
    #[serde(default = "half")]
    pub default_ratio: f32,
    /// Minimum height, in points, for each pane (keeps the divider from swallowing
    /// either side). Defaults to `80.0`.
    #[builder(default = 80.0)]
    #[serde(default = "min_pane")]
    pub min_pane: f32,
}

fn half() -> f32 {
    0.5
}
fn min_pane() -> f32 {
    80.0
}

/// A collapsible section, open by default.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct Group {
    /// Header label.
    pub label: String,
    /// Section content.
    #[builder(default)]
    #[serde(default)]
    pub children: Vec<RenderNode>,
}

/// A collapsible section, closed by default.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct Collapsible {
    /// Header label.
    pub label: String,
    /// Section content.
    #[builder(default)]
    #[serde(default)]
    pub children: Vec<RenderNode>,
}

/// A bottom-aligned group of children (rendered vertically with padding).
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[non_exhaustive]
pub struct Footer {
    /// Footer content, top-to-bottom.
    #[builder(default)]
    #[serde(default)]
    pub children: Vec<RenderNode>,
    /// Vertical gap between children, in points.
    #[builder(default)]
    #[serde(default)]
    pub gap: f32,
    /// Inner padding, in points.
    #[builder(default)]
    #[serde(default)]
    pub padding: f32,
}

/// Inline `key: value` display, where the value is itself a node.
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct KeyValue {
    /// The key label.
    pub key: String,
    /// The value node.
    #[builder(into)]
    pub value: Box<RenderNode>,
}

/// Render `child` with an overridden text colour (`#rrggbb` hex).
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct Colored {
    /// Colour applied to the subtree's text.
    pub color: String,
    /// The node to colour.
    #[builder(into)]
    pub child: Box<RenderNode>,
}

// ── Rendering ────────────────────────────────────────────────────────────────

/// Does this child act as the "grow" element in a `fill` row (it expands to
/// claim the remaining width)?
#[cfg(feature = "egui")]
fn is_grow(node: &RenderNode) -> bool {
    match node {
        RenderNode::Input(i) => i.grow,
        RenderNode::Button(b) => b.full_width,
        RenderNode::Spacer(_) => true,
        _ => false,
    }
}

#[cfg(feature = "egui")]
impl Row {
    /// Render the row.
    pub fn show(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) {
        let colors = crate::theme::ThemeColors::from_ctx(ui.ctx());
        let fill = self.bg_color.resolve(&colors);
        let (gap, padding, align, max_width, height) = (
            self.gap,
            self.padding,
            self.align,
            self.max_width,
            self.height,
        );

        let mut frame = egui::Frame::new().inner_margin(egui::Margin::same(padding as i8));
        if let Some(f) = fill {
            frame = frame.fill(f);
        }
        frame.show(ui, |ui| {
            if max_width {
                ui.set_width(ui.available_width());
            }
            if let Some(h) = height {
                ui.set_height(h);
            }
            ui.spacing_mut().item_spacing.x = gap;

            match align {
                Align::Start => {
                    ui.horizontal(|ui| {
                        for child in &mut self.children {
                            child.show(ui, events);
                        }
                    });
                }
                Align::Center => {
                    // Center the row of children horizontally: lay them out
                    // left-to-right inside a top-down/centered wrapper so the
                    // group is centered within the available width.
                    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
                        ui.horizontal(|ui| {
                            ui.spacing_mut().item_spacing.x = gap;
                            for child in &mut self.children {
                                child.show(ui, events);
                            }
                        });
                    });
                }
                Align::End => {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        for child in self.children.iter_mut().rev() {
                            child.show(ui, events);
                        }
                    });
                }
                Align::Fill => {
                    // [prefix LTR…] [grow fills middle] [suffix RTL…]
                    let grow = self.children.iter().position(is_grow);
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = gap;
                        match grow {
                            Some(gi) => {
                                let (prefix, rest) = self.children.split_at_mut(gi);
                                let (grow_child, suffix) = rest.split_at_mut(1);
                                for child in prefix {
                                    child.show(ui, events);
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.spacing_mut().item_spacing.x = gap;
                                        for child in suffix.iter_mut().rev() {
                                            child.show(ui, events);
                                        }
                                        grow_child[0].show(ui, events);
                                    },
                                );
                            }
                            None => {
                                for child in &mut self.children {
                                    child.show(ui, events);
                                }
                            }
                        }
                    });
                }
            }
        });
    }
}

#[cfg(feature = "egui")]
impl Column {
    /// Render the column.
    pub fn show(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) {
        if self.framed {
            let colors = crate::theme::ThemeColors::from_ctx(ui.ctx());
            egui::Frame::new()
                .fill(colors.bg_panel)
                .stroke(egui::Stroke::new(1.0, colors.surface))
                .corner_radius(6)
                .inner_margin(egui::Margin::same(4))
                .outer_margin(egui::Margin::same(8))
                .show(ui, |ui| self.body(ui, events));
        } else {
            self.body(ui, events);
        }
    }

    fn body(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) {
        // Zero padding skips the frame entirely, so the (many) unpadded columns
        // keep exactly the layout they had before the prop existed.
        if self.padding > 0.0 {
            egui::Frame::NONE
                .inner_margin(egui::Margin::same(self.padding as i8))
                .show(ui, |ui| self.children_vertical(ui, events));
        } else {
            self.children_vertical(ui, events);
        }
    }

    fn children_vertical(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) {
        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing.y = self.gap;
            for child in &mut self.children {
                child.show(ui, events);
            }
        });
    }
}

#[cfg(feature = "egui")]
impl Scroll {
    /// Render the scroll area and its child.
    pub fn show(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) {
        // Claim the full available area so the scroll region fills its slot
        // (and its content can fill it too) rather than collapsing to content.
        // `auto_shrink(false)` is what actually makes the `ScrollArea` fill —
        // otherwise egui shrinks it back to content height (leaving the slot
        // half-empty) even after `set_min_size`.
        ui.set_min_size(ui.available_size());
        let mut area = if self.both {
            egui::ScrollArea::both()
        } else {
            egui::ScrollArea::vertical()
        }
        .auto_shrink([false, false]);
        if let Some(id) = &self.id {
            area = area.id_salt(id);
        }
        if let Some(h) = self.max_height {
            area = area.max_height(h);
        }
        area.show(ui, |ui| self.child.show(ui, events));
    }
}

#[cfg(feature = "egui")]
impl Spacer {
    /// Add the fixed space.
    pub fn show(&self, ui: &mut egui::Ui) {
        ui.add_space(self.size);
    }
}

/// Width of a [`resizable`](Split::resizable) split's first column: `ratio` of the
/// `usable` width, clamped so neither column drops below `min_pane`.
///
/// When the split is too narrow to honour the minimum on both sides the minimum
/// degrades to half the usable width, so the columns stay even rather than one
/// collapsing — the same rule `VSplit` applies to its pane heights.
#[cfg(feature = "egui")]
fn resizable_col_width(usable: f32, ratio: f32, min_pane: f32) -> f32 {
    let min = min_pane.min(usable / 2.0).max(0.0);
    (usable * ratio).clamp(min, (usable - min).max(min))
}

#[cfg(feature = "egui")]
impl Split {
    /// Render the proportional columns, each filling the full row height.
    pub fn show(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) {
        if self.children.is_empty() {
            return;
        }
        // A framed split owns the whole floating posture, exactly as `VSplit`
        // does: the crust canvas its cards sit on, and a `GUTTER_GAP` inset so
        // they don't touch the container's edges. Leaving the canvas to the caller
        // made the two split components asymmetric and every call site had to
        // re-derive it.
        if self.framed {
            let colors = crate::theme::ThemeColors::from_ctx(ui.ctx());
            let gutter = crate::theme::GUTTER_GAP;
            egui::Frame::NONE
                .fill(colors.bg_sunken)
                .inner_margin(gutter as i8)
                .show(ui, |ui| self.columns(ui, events));
        } else {
            self.columns(ui, events);
        }
    }

    /// Lay the columns out (and their cards, when framed) in a horizontal row.
    fn columns(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) {
        let n = self.children.len();
        let gap = self.gap;
        // Only a two-column split has one unambiguous divider to drag; with three
        // or more the flag is ignored and the layout stays proportional (see the
        // prop's docs).
        let resizable = self.resizable && n == 2;
        // A resizable split's divider doubles as the gutter, so it is sized to the
        // design's grab handle (`.hhandle{width:11px}`) rather than the plain 8px
        // panel gap — exactly as `VSplit` sizes its 11px `.vhandle`. `gap` is
        // superseded for that one divider.
        let handle_w = 11.0_f32;
        let total_gap = if resizable {
            handle_w
        } else {
            gap * n.saturating_sub(1) as f32
        };
        let available = ui.available_width();
        let usable = (available - total_gap).max(0.0);

        // Persisted first-column fraction, seeded from `widths` (or an equal share)
        // until the user drags. Keyed by `id` so it survives across frames.
        let ratio_id = ui.make_persistent_id((&self.id, "split_ratio"));
        let min_pane = self.min_pane;

        // Resolve per-column widths from relative weights (equal if absent), or —
        // when resizable — from the dragged ratio.
        let weight_ratio = |w: &[f32]| w[0] / (w[0] + w[1]).max(0.001);
        let col_widths: Vec<f32> = if resizable {
            let seed = if self.widths.len() == n {
                weight_ratio(&self.widths)
            } else {
                0.5
            };
            let ratio: f32 = ui.ctx().data(|d| d.get_temp(ratio_id)).unwrap_or(seed);
            let first = resizable_col_width(usable, ratio, min_pane);
            vec![first, (usable - first).max(0.0)]
        } else if self.widths.len() == n {
            let sum: f32 = self.widths.iter().copied().sum::<f32>().max(0.001);
            self.widths.iter().map(|w| usable * (w / sum)).collect()
        } else {
            vec![usable / n as f32; n]
        };

        // Lay the columns out in a horizontal row. Content-height by default (for
        // centred alignment, give the row a uniform min-height and use
        // `horizontal`, which centres its children vertically; start alignment
        // top-aligns via `horizontal_top`). When `fill_height` is set, each column
        // is given the full available height so it can hold a pane-filling scroll
        // region — this is what a request/response-style split needs.
        let separator = self.separator;
        let fill = self.fill_height;
        let center = self.align == Align::Center && !fill;
        // Opt-in floating columns, matching `VSplit`'s panes. The fill is carried
        // alongside the frame so the corner mask can restore it.
        let card = if self.framed {
            let colors = crate::theme::ThemeColors::from_ctx(ui.ctx());
            Some((
                egui::Frame::NONE
                    .fill(colors.bg)
                    .corner_radius(crate::theme::RADIUS_PANEL)
                    .stroke(crate::theme::edge_stroke(&colors)),
                colors.bg,
            ))
        } else {
            None
        };
        // Grabber colours resolved up front — the layout closure below only gets a
        // `&mut Ui`, and a non-resizable split must not pay for the lookup.
        let grip_colors = resizable.then(|| {
            let c = crate::theme::ThemeColors::from_ctx(ui.ctx());
            (c.surface_raised, c.accent)
        });
        let row_min = ui.spacing().interact_size.y;
        let avail_h = ui.available_height();
        let children = &mut self.children;
        let body = |ui: &mut egui::Ui| {
            if fill {
                ui.set_min_height(avail_h);
            } else if center {
                ui.set_min_height(row_min);
            }
            ui.spacing_mut().item_spacing.x = 0.0;
            for (i, child) in children.iter_mut().enumerate() {
                let col_w = col_widths[i];
                let col_h = if fill { avail_h } else { 0.0 };
                ui.allocate_ui_with_layout(
                    egui::vec2(col_w, col_h),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| {
                        ui.set_min_width(col_w);
                        ui.set_max_width(col_w);
                        if fill {
                            ui.set_min_height(avail_h);
                        } else {
                            // Keep each cell to one line; extend past the column
                            // rather than wrapping to a second row.
                            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Extend);
                        }
                        match &card {
                            Some((frame, pane_fill)) => {
                                let painted = frame.show(ui, |ui| {
                                    ui.set_min_size(ui.available_size());
                                    ui.set_clip_rect(ui.max_rect());
                                    child.show(ui, events);
                                });
                                // A child that fills its own band (a status strip,
                                // a tab bar) paints square over the card's corners,
                                // since egui has no rounded clip rect. Mask the
                                // corner wedges back to the card's fill and re-lay
                                // the hairline on top — same approach as
                                // `TableView`/`DataView`.
                                let rect = painted.response.rect;
                                crate::components::table_view::ui::paint_corner_mask(
                                    ui.painter(),
                                    rect,
                                    crate::theme::RADIUS_PANEL,
                                    *pane_fill,
                                );
                                ui.painter().rect_stroke(
                                    rect,
                                    crate::theme::RADIUS_PANEL,
                                    crate::theme::edge_stroke(
                                        &crate::theme::ThemeColors::from_ctx(ui.ctx()),
                                    ),
                                    egui::StrokeKind::Inside,
                                );
                            }
                            None => child.show(ui, events),
                        }
                    },
                );
                if i + 1 < n {
                    match &grip_colors {
                        // ── Draggable divider ────────────────────────────────
                        // Sits in the gutter *between* the two cards: `framed`'s
                        // sunken canvas shows through around it, so the grip reads
                        // as floating on the crust rather than drawn on a pane.
                        Some((resting, active)) => {
                            let handle_h = if fill { avail_h } else { row_min };
                            let (rect, resp) = ui.allocate_exact_size(
                                egui::vec2(handle_w, handle_h),
                                egui::Sense::drag(),
                            );
                            let hovered = resp.hovered() || resp.dragged();
                            // No rule down the gutter — the panes are separate
                            // floating cards and a line spanning the gap would tie
                            // them back together. Just the design's centred pill
                            // grip (`.hhandle .grip`: 4x36, fully round,
                            // `surface-raised`), tinted mauve while hovered or
                            // dragging.
                            // 36pt tall where there's room; clamped to the handle
                            // otherwise, since an unfilled row sizes it from
                            // `row_min` and the grip would overhang both ends.
                            let grip = egui::Rect::from_center_size(
                                rect.center(),
                                egui::vec2(4.0, 36.0_f32.min(rect.height())),
                            );
                            ui.painter().rect_filled(
                                grip,
                                crate::theme::RADIUS_PILL,
                                if hovered { *active } else { *resting },
                            );
                            if hovered {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
                            }
                            if resp.dragged() && usable > 0.0 {
                                let dragged = col_widths[0] + resp.drag_delta().x;
                                let first = resizable_col_width(usable, dragged / usable, min_pane);
                                ui.ctx()
                                    .data_mut(|d| d.insert_temp(ratio_id, first / usable));
                            }
                        }
                        None if separator => {
                            ui.add(egui::Separator::default().vertical());
                        }
                        None => ui.add_space(gap),
                    }
                }
            }
        };
        if center {
            ui.horizontal(body);
        } else {
            ui.horizontal_top(body);
        }
    }
}

#[cfg(feature = "egui")]
impl VSplit {
    /// Render `top` over `bottom` with a draggable divider between them. Each
    /// pane is given a fixed height (from the persisted ratio) so its content
    /// scrolls within, and dragging the divider re-apportions the height.
    pub fn show(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) {
        use crate::theme::{GUTTER_GAP, RADIUS_PANEL, edge_stroke, panel_shadow};

        let colors = crate::theme::ThemeColors::from_ctx(ui.ctx());
        // The divider doubles as the gutter between the two panes, so it is sized
        // to the design's grab handle (`.vhandle{height:11px}`) rather than the
        // plain 8px panel gap.
        let handle_h = 11.0_f32;
        // Each pane is a floating card — fill, hairline edge, panel corners and a
        // soft shadow (design: editor and results are two stacked `.panel sq`
        // cards). The shadow has somewhere to fall because the crust gutter below
        // insets them from the container on every side.
        let pane_frame = egui::Frame::NONE
            .fill(colors.bg)
            .corner_radius(RADIUS_PANEL)
            .stroke(edge_stroke(&colors))
            .shadow(panel_shadow(ui.visuals().dark_mode));

        // The split's own area is the *sunken* canvas, not a panel: the two cards
        // float on it and the crust shows through as the gutter, which is what
        // separates them (design `.splitv{background:bg-sunken;padding:8px}`).
        // Leaving it transparent would inherit the enclosing panel's `bg` and the
        // two cards would blend back into one continuous slab.
        egui::Frame::NONE
            .fill(colors.bg_sunken)
            .inner_margin(GUTTER_GAP as i8)
            .show(ui, |ui| {
                self.panes(ui, events, &colors, &pane_frame, handle_h);
            });
    }

    /// Lay the two cards and the drag handle out inside the crust gutter.
    fn panes(
        &mut self,
        ui: &mut egui::Ui,
        events: &mut Vec<UiEvent>,
        colors: &crate::theme::ThemeColors,
        pane_frame: &egui::Frame,
        handle_h: f32,
    ) {
        let full = ui.available_size();
        let width = full.x;
        let total_h = full.y;
        let panes_h = (total_h - handle_h).max(0.0);

        // Persisted top fraction, defaulting to `default_ratio` on first render.
        let ratio_id = ui.make_persistent_id((&self.id, "vsplit_ratio"));
        let mut ratio: f32 = ui
            .ctx()
            .data(|d| d.get_temp(ratio_id))
            .unwrap_or(self.default_ratio);

        // Clamp so neither pane drops below `min_pane` (when there's room for both).
        let min = self.min_pane.min(panes_h / 2.0).max(0.0);
        let top_h = (panes_h * ratio).clamp(min, (panes_h - min).max(min));
        let bottom_h = (panes_h - top_h).max(0.0);

        ui.vertical(|ui| {
            ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

            // ── Top pane ─────────────────────────────────────────────────────
            ui.allocate_ui_with_layout(
                egui::vec2(width, top_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_size(egui::vec2(width, top_h));
                    ui.set_max_size(egui::vec2(width, top_h));
                    // Clip the card's *content*, not the card: narrowing this
                    // `ui`'s clip would also cut off the frame's shadow, which is
                    // painted outside the frame rect.
                    pane_frame.show(ui, |ui| {
                        ui.set_min_size(ui.available_size());
                        ui.set_clip_rect(ui.max_rect());
                        self.top.show(ui, events);
                    });
                },
            );

            // ── Divider ──────────────────────────────────────────────────────
            let (handle_rect, resp) =
                ui.allocate_exact_size(egui::vec2(width, handle_h), egui::Sense::drag());
            let hovered = resp.hovered() || resp.dragged();
            // No rule across the gutter — the panes are separate floating cards and
            // a line spanning the gap between them would tie them back together.
            // Just the design's centred pill grip (`.vhandle .grip`: 36x4, fully
            // round, `surface1`), tinted mauve while hovered or dragging.
            let grip = egui::Rect::from_center_size(handle_rect.center(), egui::vec2(36.0, 4.0));
            ui.painter().rect_filled(
                grip,
                crate::theme::RADIUS_PILL,
                if hovered {
                    colors.accent
                } else {
                    colors.surface_raised
                },
            );
            if hovered {
                ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeVertical);
            }
            if resp.dragged() && panes_h > 0.0 {
                let new_top = (top_h + resp.drag_delta().y).clamp(min, (panes_h - min).max(min));
                ratio = new_top / panes_h;
                ui.ctx().data_mut(|d| d.insert_temp(ratio_id, ratio));
            }

            // ── Bottom pane ──────────────────────────────────────────────────
            ui.allocate_ui_with_layout(
                egui::vec2(width, bottom_h),
                egui::Layout::top_down(egui::Align::Min),
                |ui| {
                    ui.set_min_size(egui::vec2(width, bottom_h));
                    ui.set_max_size(egui::vec2(width, bottom_h));
                    pane_frame.show(ui, |ui| {
                        ui.set_min_size(ui.available_size());
                        ui.set_clip_rect(ui.max_rect());
                        self.bottom.show(ui, events);
                    });
                },
            );
        });
    }
}

#[cfg(feature = "egui")]
impl Group {
    /// Render the collapsible (open by default).
    pub fn show(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) {
        egui::CollapsingHeader::new(self.label.as_str())
            .default_open(true)
            .show(ui, |ui| {
                for child in &mut self.children {
                    child.show(ui, events);
                }
            });
    }
}

#[cfg(feature = "egui")]
impl Collapsible {
    /// Render the collapsible (closed by default).
    pub fn show(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) {
        egui::CollapsingHeader::new(self.label.as_str())
            .default_open(false)
            .show(ui, |ui| {
                for child in &mut self.children {
                    child.show(ui, events);
                }
            });
    }
}

#[cfg(feature = "egui")]
impl Footer {
    /// Render the footer content.
    pub fn show(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) {
        egui::Frame::new()
            .inner_margin(egui::Margin::same(self.padding as i8))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.y = self.gap;
                for child in &mut self.children {
                    child.show(ui, events);
                }
            });
    }
}

#[cfg(feature = "egui")]
impl KeyValue {
    /// Render the `key: value` pair.
    pub fn show(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) {
        ui.horizontal(|ui| {
            let muted = ui.visuals().weak_text_color();
            ui.label(egui::RichText::new(format!("{}: ", self.key)).color(muted));
            self.value.show(ui, events);
        });
    }
}

#[cfg(feature = "egui")]
impl Colored {
    /// Render `child` with the overridden text colour.
    pub fn show(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) {
        let colors = crate::theme::ThemeColors::from_ctx(ui.ctx());
        let resolved = crate::theme::resolve_color(&self.color, &colors);
        ui.scope(|ui| {
            if let Some(c) = resolved {
                ui.visuals_mut().override_text_color = Some(c);
            }
            self.child.show(ui, events);
        });
    }
}

#[cfg(all(test, feature = "egui"))]
mod tests {
    use super::*;

    #[test]
    fn a_dragged_ratio_never_collapses_a_column() {
        // 600pt of usable width, 120pt minimum per column: a ratio dragged past
        // either end pins to the minimum instead of swallowing the other pane.
        assert_eq!(resizable_col_width(600.0, 0.5, 120.0), 300.0);
        assert_eq!(resizable_col_width(600.0, 0.0, 120.0), 120.0);
        assert_eq!(resizable_col_width(600.0, -3.0, 120.0), 120.0);
        assert_eq!(resizable_col_width(600.0, 1.0, 120.0), 480.0);
        assert_eq!(resizable_col_width(600.0, 4.0, 120.0), 480.0);
    }

    #[test]
    fn a_split_too_narrow_for_the_minimum_stays_even() {
        // Below `2 * min_pane` there is no honouring the minimum on both sides, so
        // it degrades to half the width rather than starving one column.
        assert_eq!(resizable_col_width(200.0, 0.9, 120.0), 100.0);
        assert_eq!(resizable_col_width(0.0, 0.5, 120.0), 0.0);
    }

    #[test]
    fn the_defaults_leave_a_split_proportional() {
        // Every pre-existing call site builds a `Split` without the new props, and
        // must keep the non-interactive proportional behaviour.
        let split = Split::builder().build();
        assert!(!split.resizable);
        assert_eq!(split.id, None);
        assert_eq!(split.min_pane, min_col());
        assert_eq!(Column::builder().build().padding, 0.0);
    }
}
