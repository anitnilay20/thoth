use bon::Builder;
use serde::{Deserialize, Serialize};

/// egui's own `Separator` reserves this much space around the line; the SDK
/// keeps it as the default so untouched call sites lay out exactly as before.
#[cfg(feature = "egui")]
const DEFAULT_SPACING: f32 = 6.0;

/// A horizontal divider line with optional vertical margins.
///
/// By default this is egui's separator: the theme's hairline stroke inside 6pt
/// of breathing room. [`color`](Separator::color), [`thickness`](Separator::thickness)
/// and [`spacing`](Separator::spacing) override that, which is what the design's
/// band edges need — `box-shadow: inset 0 -1px 0 var(--surface)` is a 1px rule
/// in a *specific* colour that occupies no space of its own.
///
/// ```
/// use thoth_plugin_sdk::components::Separator;
///
/// let sep = Separator::with_margin(8.0);
/// // …or a flush 1px band edge in a theme colour:
/// let rule = Separator::rule("fg-faint");
/// ```
#[derive(Debug, Clone, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct Separator {
    /// Space added above the line, in points.
    #[builder(default)]
    #[serde(default, rename = "margin-top")]
    pub margin_top: f32,
    /// Space added below the line, in points.
    #[builder(default)]
    #[serde(default, rename = "margin-bottom")]
    pub margin_bottom: f32,
    /// Line colour as a `#rrggbb(aa)` hex string or a theme token. When unset,
    /// the theme's own separator stroke colour is used.
    #[serde(default)]
    pub color: Option<String>,
    /// Line thickness in points. When unset, the theme's separator stroke width
    /// is used (1pt).
    #[serde(default)]
    pub thickness: Option<f32>,
    /// Total vertical space the separator occupies, in points — the line is
    /// centred in it. Defaults to egui's 6pt; pass `0.0` for a flush band edge
    /// that takes no more room than its own thickness.
    #[serde(default)]
    pub spacing: Option<f32>,
}

impl Separator {
    /// A separator with no margins.
    pub fn plain() -> Self {
        Self::default()
    }

    /// A separator with equal top and bottom margins.
    pub fn with_margin(margin: f32) -> Self {
        Self {
            margin_top: margin,
            margin_bottom: margin,
            ..Self::default()
        }
    }

    /// A separator with independent top and bottom margins.
    pub fn with_margins(top: f32, bottom: f32) -> Self {
        Self {
            margin_top: top,
            margin_bottom: bottom,
            ..Self::default()
        }
    }

    /// A flush 1pt rule in `color` (hex or theme token) that occupies nothing
    /// but its own thickness — the design's `box-shadow: inset 0 ±1px 0 …` band
    /// edge, which parts two sections without adding a gap between them.
    pub fn rule(color: impl Into<String>) -> Self {
        Self {
            color: Some(color.into()),
            thickness: Some(1.0),
            spacing: Some(0.0),
            ..Self::default()
        }
    }

    /// Whether any of the paint overrides are set — if not, this is egui's own
    /// separator and is drawn by it, unchanged.
    #[cfg_attr(not(any(feature = "egui", test)), allow(dead_code))]
    fn is_styled(&self) -> bool {
        self.color.is_some() || self.thickness.is_some() || self.spacing.is_some()
    }
}

#[cfg(feature = "egui")]
impl Separator {
    /// This separator's resolved line stroke against the active theme.
    pub fn stroke(&self, ui: &egui::Ui) -> egui::Stroke {
        let base = ui.visuals().widgets.noninteractive.bg_stroke;
        let colors = crate::theme::ThemeColors::from_ctx(ui.ctx());
        egui::Stroke::new(
            self.thickness.unwrap_or(base.width),
            self.color
                .as_deref()
                .and_then(|c| crate::theme::resolve_color(c, &colors))
                .unwrap_or(base.color),
        )
    }

    /// Paint this separator's line along `x_range` at `y` **without allocating**
    /// any layout space.
    ///
    /// For the rules that are an edge of something already laid out — a row's
    /// bottom hairline, the divider along a list row's top — where consuming a
    /// point of height would push the very thing being underlined.
    pub fn paint_at(&self, ui: &egui::Ui, x_range: egui::Rangef, y: f32) {
        ui.painter().hline(x_range, y, self.stroke(ui));
    }
}

#[cfg(feature = "egui")]
impl egui::Widget for Separator {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        if self.margin_top > 0.0 {
            ui.add_space(self.margin_top);
        }
        let response = if self.is_styled() {
            let stroke = self.stroke(ui);
            // The line is centred in its slot, as egui's separator centres in
            // its 6pt. A slot can never be thinner than the line it carries, so
            // `spacing(0.0)` yields a rule exactly `thickness` tall.
            let height = self.spacing.unwrap_or(DEFAULT_SPACING).max(stroke.width);
            let (rect, response) = ui.allocate_exact_size(
                egui::vec2(ui.available_width(), height),
                egui::Sense::hover(),
            );
            if ui.is_rect_visible(rect) {
                let line = egui::Rect::from_min_size(
                    egui::pos2(rect.left(), rect.center().y - stroke.width / 2.0),
                    egui::vec2(rect.width(), stroke.width),
                );
                ui.painter().rect_filled(line, 0.0, stroke.color);
            }
            response
        } else {
            ui.separator()
        };
        if self.margin_bottom > 0.0 {
            ui.add_space(self.margin_bottom);
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::Separator;
    use serde_json::{Value, json};

    #[test]
    fn plain_has_zero_margins() {
        let sep = Separator::plain();
        assert_eq!(sep.margin_top, 0.0);
        assert_eq!(sep.margin_bottom, 0.0);
    }

    #[test]
    fn with_margin_sets_equal_margins() {
        let sep = Separator::with_margin(6.0);
        assert_eq!(sep.margin_top, 6.0);
        assert_eq!(sep.margin_bottom, 6.0);
    }

    #[test]
    fn with_margins_sets_independent_margins() {
        let sep = Separator::with_margins(2.0, 8.0);
        assert_eq!(sep.margin_top, 2.0);
        assert_eq!(sep.margin_bottom, 8.0);
    }

    #[test]
    fn builder_sets_margins() {
        let sep = Separator::builder()
            .margin_top(3.0)
            .margin_bottom(5.0)
            .build();
        assert_eq!(sep.margin_top, 3.0);
        assert_eq!(sep.margin_bottom, 5.0);
    }

    #[test]
    fn plain_serialises_with_zero_margins() {
        let sep = Separator::plain();
        let v: Value = serde_json::to_value(sep).unwrap();
        // margin-top and margin-bottom are 0.0 (default, may be skipped or 0)
        assert!(v["margin-top"].as_f64().unwrap_or(0.0) == 0.0);
        assert!(v["margin-bottom"].as_f64().unwrap_or(0.0) == 0.0);
    }

    #[test]
    fn with_margins_serialises_renamed_fields() {
        let sep = Separator::with_margins(4.0, 8.0);
        let v: Value = serde_json::to_value(sep).unwrap();
        assert_eq!(v["margin-top"].as_f64().unwrap(), 4.0);
        assert_eq!(v["margin-bottom"].as_f64().unwrap(), 8.0);
    }

    #[test]
    fn round_trips_through_json() {
        let original = Separator::with_margins(1.5, 2.5);
        let json = serde_json::to_string(&original).unwrap();
        let restored: Separator = serde_json::from_str(&json).unwrap();
        assert_eq!(restored.margin_top, original.margin_top);
        assert_eq!(restored.margin_bottom, original.margin_bottom);
    }

    // ── paint overrides ───────────────────────────────────────────────────────

    #[test]
    fn constructors_leave_the_paint_overrides_unset() {
        // Every existing call site goes through these, so all three must stay
        // `None` — that is what keeps them on egui's own separator.
        for sep in [
            Separator::plain(),
            Separator::with_margin(6.0),
            Separator::with_margins(1.0, 2.0),
            Separator::builder().build(),
        ] {
            assert!(sep.color.is_none());
            assert!(sep.thickness.is_none());
            assert!(sep.spacing.is_none());
            assert!(!sep.is_styled());
        }
    }

    #[test]
    fn rule_is_a_flush_one_point_line() {
        let sep = Separator::rule("fg-faint");
        assert_eq!(sep.color.as_deref(), Some("fg-faint"));
        assert_eq!(sep.thickness, Some(1.0));
        assert_eq!(sep.spacing, Some(0.0));
        assert_eq!(sep.margin_top, 0.0);
        assert_eq!(sep.margin_bottom, 0.0);
        assert!(sep.is_styled());
    }

    #[test]
    fn deserialises_without_the_paint_overrides() {
        let sep: Separator = serde_json::from_value(json!({ "margin-top": 4.0 })).unwrap();
        assert_eq!(sep.margin_top, 4.0);
        assert!(sep.color.is_none());
        assert!(sep.thickness.is_none());
        assert!(sep.spacing.is_none());
    }
}
