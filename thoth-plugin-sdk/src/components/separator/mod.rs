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
