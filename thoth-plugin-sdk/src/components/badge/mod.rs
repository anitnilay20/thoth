use bon::Builder;
use serde::{Deserialize, Serialize};

use crate::components::Size;

fn default_true() -> bool {
    true
}

/// A small colored pill label (e.g. an HTTP method or status tag).
///
/// ```
/// use thoth_plugin_sdk::components::Badge;
///
/// let badge = Badge::builder().label("GET").color("#89b4fa").build();
/// ```
#[derive(Clone, Debug, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct Badge {
    /// Text shown inside the pill.
    pub label: String,
    /// Fill colour as a `#rrggbb` hex string; defaults to the secondary accent.
    #[serde(default)]
    pub color: Option<String>,
    /// When true, render as an outlined pill (transparent fill, coloured 1px
    /// border and coloured monospace text) instead of a filled one.
    #[builder(default)]
    #[serde(default)]
    pub outlined: bool,
    /// When true, render as a soft pill: a faint tint of `color` filled behind
    /// coloured monospace text (the enum-value chip style). Takes precedence
    /// over [`outlined`](Badge::outlined).
    #[builder(default)]
    #[serde(default)]
    pub soft: bool,
    /// Pill size (font + padding). Defaults to [`Size::Small`] — a slim pill.
    #[builder(default = Size::Small)]
    #[serde(default = "default_badge_size")]
    pub size: Size,
    /// Optional leading Phosphor glyph, drawn at the label's size with the
    /// label's colour — design `.dt-badge{gap:5px}` with its `<i class="ph …">`.
    /// `None` (the default) draws the label alone.
    #[serde(default)]
    pub icon: Option<String>,
    /// Draw the chip fully round ([`RADIUS_PILL`]) instead of the variant's own
    /// radius — design `.pill`/`.dt-badge{border-radius:999px}`. Defaults to
    /// `false`, which keeps the filled/outlined 3px and soft 8px corners.
    ///
    /// [`RADIUS_PILL`]: crate::theme::RADIUS_PILL
    #[builder(default)]
    #[serde(default)]
    pub pill: bool,
    /// Render the label in the monospace family. Defaults to `true` — design
    /// `.badge{font-family:var(--mono)}`. Set `false` for the proportional chips
    /// (`.pill`, `.dt-badge`), which carry prose rather than a code token.
    #[builder(default = true)]
    #[serde(default = "default_true")]
    pub mono: bool,
    /// Thicken the label's strokes, standing in for design `.pill`'s
    /// `font-weight:700`. Defaults to `false`: the base chip's 600 is drawn as
    /// plain weight, as it always has been.
    #[builder(default)]
    #[serde(default)]
    pub bold: bool,
    /// Font size override in points; derived from [`size`](Badge::size) when
    /// unset. Padding still comes from the size preset.
    #[serde(default, rename = "font-size")]
    pub font_size: Option<f32>,
}

fn default_badge_size() -> Size {
    Size::Small
}

/// Filled and outlined chips — design `.badge.filled`/`.badge.outlined`
/// (`border-radius:3px`). Below the radius ladder's smallest rung, so the
/// handoff pins them as raw values.
#[cfg(feature = "egui")]
const RADIUS_BADGE: f32 = 3.0;
/// Soft chips — design `.badge.soft{border-radius:8px}`, the squircle pill.
#[cfg(feature = "egui")]
const RADIUS_BADGE_SOFT: f32 = 8.0;
/// Soft fill tint — design `.badge.soft{background:currentColor@18%}`.
#[cfg(feature = "egui")]
const SOFT_TINT_ALPHA: u8 = 46; // 18% of 255
/// Gap between a leading glyph and the label — design `.dt-badge{gap:5px}`.
#[cfg(feature = "egui")]
const ICON_GAP: f32 = 5.0;

#[cfg(feature = "egui")]
impl Badge {
    /// The label (and any leading glyph) in `color`, laid out as the chip's
    /// content. A glyph-less badge is a bare label, exactly as before.
    fn content(&self, ui: &mut egui::Ui, font: f32, color: egui::Color32) {
        match self.icon.as_deref().filter(|g| !g.is_empty()) {
            Some(glyph) => {
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = ICON_GAP;
                    ui.label(
                        egui::RichText::new(glyph)
                            .font(crate::theme::phosphor_font_id(font))
                            .color(color),
                    );
                    self.label(ui, font, color);
                });
            }
            None => self.label(ui, font, color),
        }
    }

    /// One run of label text. `bold` thickens it with a second 0.5px-offset
    /// pass, the same faux-weight trick [`Typography`] uses — egui has no bold
    /// face to switch to.
    ///
    /// [`Typography`]: crate::components::Typography
    fn label(&self, ui: &mut egui::Ui, font: f32, color: egui::Color32) {
        let font_id = if self.mono {
            egui::FontId::monospace(font)
        } else {
            egui::FontId::proportional(font)
        };
        if self.bold {
            let galley = ui
                .painter()
                .layout_no_wrap(self.label.clone(), font_id, color);
            let (rect, _) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
            if ui.is_rect_visible(rect) {
                ui.painter()
                    .galley(rect.min + egui::vec2(0.5, 0.0), galley.clone(), color);
                ui.painter().galley(rect.min, galley, color);
            }
        } else {
            // `.strong()` is kept for the mono chips that have always carried it,
            // so their rendering is untouched.
            ui.label(
                egui::RichText::new(&self.label)
                    .font(font_id)
                    .strong()
                    .color(color),
            );
        }
    }
}

#[cfg(feature = "egui")]
impl egui::Widget for Badge {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        use crate::theme::{
            RADIUS_PILL, ThemeColors, get_contrast_text_color, resolve_color, with_alpha,
        };
        let colors = ThemeColors::from_ctx(ui.ctx());
        let color = self
            .color
            .as_deref()
            .and_then(|c| resolve_color(c, &colors))
            .unwrap_or(colors.accent_secondary);

        // Slim by default; padding + font scale with the size. Vertical padding
        // is 0 at the small size so the pill hugs the text (matches the handoff's
        // 1px-tall enum chip).
        let (size_font, pad_x, pad_y): (f32, i8, i8) = match self.size {
            Size::Small => (9.0, 6, 0),
            Size::Medium => (10.0, 7, 1),
            Size::Large => (12.0, 9, 2),
        };
        let font = self.font_size.unwrap_or(size_font);
        let margin = egui::Margin::symmetric(pad_x, pad_y);
        // `pill` overrides every variant's own corner; otherwise the soft chip is
        // the squircle 8px and the filled/outlined ones the tighter 3px.
        let radius = match (self.pill, self.soft) {
            (true, _) => RADIUS_PILL,
            (false, true) => RADIUS_BADGE_SOFT,
            (false, false) => RADIUS_BADGE,
        };

        if self.soft {
            // A faint tint of the colour behind coloured text (the chip look) —
            // design `.badge.soft`.
            egui::Frame::new()
                .fill(with_alpha(color, SOFT_TINT_ALPHA))
                .corner_radius(radius)
                .inner_margin(margin)
                .show(ui, |ui| self.content(ui, font, color))
                .response
        } else if self.outlined {
            // Transparent fill, coloured border + coloured monospace text — the
            // schema/structure constraint-tag style (design `.badge.outlined`).
            egui::Frame::new()
                .stroke(egui::Stroke::new(1.0, color))
                .corner_radius(radius)
                .inner_margin(margin)
                .show(ui, |ui| self.content(ui, font, color))
                .response
        } else {
            // Design `.badge.filled`: solid fill, auto-contrast text.
            let fg = get_contrast_text_color(color);
            egui::Frame::new()
                .fill(color)
                .corner_radius(radius)
                .inner_margin(margin)
                .show(ui, |ui| self.content(ui, font, fg))
                .response
        }
    }
}

#[cfg(test)]
mod tests {
    use super::Badge;
    use crate::components::Size;
    use serde_json::json;

    #[test]
    fn new_props_default_to_the_original_rendering() {
        let b = Badge::builder().label("GET").build();
        assert!(b.icon.is_none(), "no leading glyph by default");
        assert!(!b.pill, "the 3px/8px corners are still the default");
        assert!(b.mono, "design pins the badge to the mono family");
        assert!(!b.bold, "the base chip keeps its plain weight");
        assert!(b.font_size.is_none(), "font size derives from `size`");
        assert_eq!(b.size, Size::Small);
    }

    #[test]
    fn builder_sets_the_new_props() {
        let b = Badge::builder()
            .label("SENSITIVE")
            .pill(true)
            .mono(false)
            .bold(true)
            .font_size(9.5)
            .icon("\u{e182}")
            .build();
        assert!(b.pill && !b.mono && b.bold);
        assert_eq!(b.font_size, Some(9.5));
        assert_eq!(b.icon.as_deref(), Some("\u{e182}"));
    }

    #[test]
    fn deserialises_without_the_new_props() {
        // A plugin built against the older schema must still round-trip.
        let b: Badge = serde_json::from_value(json!({ "label": "POST" })).unwrap();
        assert_eq!(b.label, "POST");
        assert!(b.mono, "the mono default has to survive a missing field");
        assert!(!b.pill);
        assert!(b.icon.is_none());
    }

    #[test]
    fn font_size_serialises_kebab_case() {
        let b = Badge::builder().label("x").font_size(9.5).build();
        let v = serde_json::to_value(&b).unwrap();
        assert_eq!(v["font-size"], 9.5);
    }
}
