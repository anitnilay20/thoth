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
pub struct Column {
    /// Child nodes, in order.
    #[builder(default)]
    #[serde(default)]
    pub children: Vec<RenderNode>,
    /// Vertical gap between children, in points.
    #[builder(default)]
    #[serde(default)]
    pub gap: f32,
}

/// A scrollable region wrapping a single child.
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
pub struct Scroll {
    /// The scrolled content.
    #[builder(into)]
    pub child: Box<RenderNode>,
    /// Optional fixed max height, in points.
    #[serde(default)]
    pub max_height: Option<f32>,
}

/// Empty space of a fixed size, in points.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Builder)]
pub struct Spacer {
    /// The amount of space.
    pub size: f32,
}

/// Proportional horizontal split. `widths` are relative weights (empty = equal
/// shares).
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
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
}

/// A collapsible section, open by default.
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
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
pub struct Colored {
    /// Colour applied to the subtree's text.
    pub color: String,
    /// The node to colour.
    #[builder(into)]
    pub child: Box<RenderNode>,
}

// ── Rendering ────────────────────────────────────────────────────────────────

#[cfg(feature = "egui")]
impl Row {
    /// Render the row.
    pub fn show(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) {
        let colors = crate::theme::ThemeColors::from_ctx(ui.ctx());
        let fill = self.bg_color.resolve(&colors);
        let (gap, padding, align, max_width, height) =
            (self.gap, self.padding, self.align, self.max_width, self.height);

        let mut frame = egui::Frame::new().inner_margin(egui::Margin::same(padding as i8));
        if let Some(f) = fill {
            frame = frame.fill(f);
        }
        frame.show(ui, |ui| {
            if max_width {
                ui.set_min_width(ui.available_width());
            }
            if let Some(h) = height {
                ui.set_min_height(h);
            }
            ui.spacing_mut().item_spacing.x = gap;
            let layout = match align {
                Align::End => egui::Layout::right_to_left(egui::Align::Center),
                _ => egui::Layout::left_to_right(egui::Align::Center),
            };
            ui.with_layout(layout, |ui| {
                for child in &mut self.children {
                    child.show(ui, events);
                }
            });
        });
    }
}

#[cfg(feature = "egui")]
impl Column {
    /// Render the column.
    pub fn show(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) {
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
        let mut area = egui::ScrollArea::vertical();
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
    /// Render the proportional columns.
    pub fn show(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) {
        let n = self.children.len();
        if n == 0 {
            return;
        }
        let total_gap = self.gap * (n.saturating_sub(1)) as f32;
        let avail = (ui.available_width() - total_gap).max(0.0);
        let weights: Vec<f32> = if self.widths.len() == n {
            self.widths.clone()
        } else {
            vec![1.0; n]
        };
        let sum: f32 = weights.iter().sum::<f32>().max(1.0);
        let separator = self.separator;
        ui.horizontal_top(|ui| {
            ui.spacing_mut().item_spacing.x = self.gap;
            for (i, child) in self.children.iter_mut().enumerate() {
                let w = avail * (weights[i] / sum);
                ui.allocate_ui_with_layout(
                    egui::vec2(w, ui.available_height()),
                    egui::Layout::top_down(egui::Align::Min),
                    |ui| child.show(ui, events),
                );
                if separator && i + 1 < n {
                    ui.separator();
                }
            }
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
        let resolved = crate::theme::parse_hex_color(&self.color);
        ui.scope(|ui| {
            if let Some(c) = resolved {
                ui.visuals_mut().override_text_color = Some(c);
            }
            self.child.show(ui, events);
        });
    }
}
