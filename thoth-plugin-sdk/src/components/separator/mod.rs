use bon::Builder;
use serde::{Deserialize, Serialize};

/// A horizontal divider line with optional vertical margins.
///
/// ```
/// use thoth_plugin_sdk::components::Separator;
///
/// let sep = Separator::with_margin(8.0);
/// ```
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, Builder)]
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
        }
    }

    /// A separator with independent top and bottom margins.
    pub fn with_margins(top: f32, bottom: f32) -> Self {
        Self {
            margin_top: top,
            margin_bottom: bottom,
        }
    }
}

#[cfg(feature = "egui")]
impl egui::Widget for Separator {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        if self.margin_top > 0.0 {
            ui.add_space(self.margin_top);
        }
        let response = ui.separator();
        if self.margin_bottom > 0.0 {
            ui.add_space(self.margin_bottom);
        }
        response
    }
}

#[cfg(test)]
mod tests {
    use super::Separator;
    use serde_json::Value;

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
}
