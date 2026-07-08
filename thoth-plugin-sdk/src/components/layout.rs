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
/// shares).
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

#[cfg(feature = "egui")]
impl Split {
    /// Render the proportional columns, each filling the full row height.
    pub fn show(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) {
        let n = self.children.len();
        if n == 0 {
            return;
        }
        let gap = self.gap;
        let total_gap = gap * n.saturating_sub(1) as f32;
        let available = ui.available_width();
        let usable = (available - total_gap).max(0.0);

        // Resolve per-column widths from relative weights (equal if absent).
        let col_widths: Vec<f32> = if self.widths.len() == n {
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
                        child.show(ui, events);
                    },
                );
                if i + 1 < n {
                    if separator {
                        ui.add(egui::Separator::default().vertical());
                    } else {
                        ui.add_space(gap);
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
        let colors = crate::theme::ThemeColors::from_ctx(ui.ctx());
        let handle_h = 7.0_f32;

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
                    ui.set_clip_rect(ui.max_rect());
                    self.top.show(ui, events);
                },
            );

            // ── Divider ──────────────────────────────────────────────────────
            let (handle_rect, resp) =
                ui.allocate_exact_size(egui::vec2(width, handle_h), egui::Sense::drag());
            let hovered = resp.hovered() || resp.dragged();
            let line_y = handle_rect.center().y;
            ui.painter().hline(
                handle_rect.x_range(),
                line_y,
                egui::Stroke::new(1.0, colors.surface_raised),
            );
            // A short grip in the centre, brighter on hover/drag.
            let grip_c = if hovered {
                colors.accent
            } else {
                colors.fg_muted
            };
            let grip_w = 24.0;
            ui.painter().hline(
                (handle_rect.center().x - grip_w / 2.0)..=(handle_rect.center().x + grip_w / 2.0),
                line_y,
                egui::Stroke::new(2.0, grip_c),
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
                    ui.set_clip_rect(ui.max_rect());
                    self.bottom.show(ui, events);
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
