use bon::Builder;
use serde::{Deserialize, Serialize};

/// Default diameter — the design sheet's smallest `.spin` (16px).
#[cfg(feature = "egui")]
const DEFAULT_SIZE: f32 = 16.0;
/// Ring thickness — design `.spin{border:2.5px …}`.
#[cfg(feature = "egui")]
const STROKE: f32 = 2.5;
/// …thickened on the largest size — design the 26px `.spin{border-width:3px}`.
#[cfg(feature = "egui")]
const STROKE_LARGE: f32 = 3.0;
/// The size at which the thicker ring kicks in.
#[cfg(feature = "egui")]
const LARGE_SIZE: f32 = 26.0;
/// One full turn, in seconds — design `animation:sp .7s linear infinite`.
#[cfg(feature = "egui")]
const PERIOD: f64 = 0.7;
/// Fraction of the ring drawn solid: the CSS colours a single border side, i.e.
/// a quarter turn.
#[cfg(feature = "egui")]
const ARC_FRACTION: f32 = 0.25;
/// Line segments used to approximate the leading arc.
#[cfg(feature = "egui")]
const ARC_SEGMENTS: usize = 12;
/// Alpha of the ring behind the arc — design `accent 26%`.
#[cfg(feature = "egui")]
const TRACK_ALPHA: u8 = 66;

/// An indeterminate loading spinner: a faint accent ring with a solid accent arc
/// sweeping around it. The design sheet uses 16 / 20 / 26 pt.
///
/// ```
/// use thoth_plugin_sdk::components::Spinner;
///
/// let spinner = Spinner::builder().size(20.0).build();
/// ```
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize, Builder)]
#[non_exhaustive]
pub struct Spinner {
    /// Diameter in points; defaults to 16.
    #[serde(default)]
    pub size: Option<f32>,
}

#[cfg(feature = "egui")]
impl egui::Widget for Spinner {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        use crate::theme::{ThemeColors, with_alpha};
        use std::f32::consts::TAU;

        let colors = ThemeColors::from_ctx(ui.ctx());
        let diameter = self.size.unwrap_or(DEFAULT_SIZE);
        let stroke_w = if diameter >= LARGE_SIZE {
            STROKE_LARGE
        } else {
            STROKE
        };

        let (rect, response) =
            ui.allocate_exact_size(egui::Vec2::splat(diameter), egui::Sense::hover());

        if ui.is_rect_visible(rect) {
            // Hand-painted rather than `egui::Spinner`, which draws a single-colour
            // arc: the design needs a two-tone ring (faint full circle + solid
            // leading arc).
            let center = rect.center();
            let radius = (diameter - stroke_w) / 2.0;
            let painter = ui.painter();
            painter.circle_stroke(
                center,
                radius,
                egui::Stroke::new(stroke_w, with_alpha(colors.accent, TRACK_ALPHA)),
            );

            let start = (ui.input(|i| i.time).rem_euclid(PERIOD) / PERIOD) as f32 * TAU;
            let sweep = TAU * ARC_FRACTION;
            let arc: Vec<egui::Pos2> = (0..=ARC_SEGMENTS)
                .map(|i| {
                    let angle = start + sweep * i as f32 / ARC_SEGMENTS as f32;
                    center + egui::vec2(angle.cos(), angle.sin()) * radius
                })
                .collect();
            ui.painter().add(egui::Shape::line(
                arc,
                egui::Stroke::new(stroke_w, colors.accent),
            ));

            // Keep the animation running.
            ui.ctx().request_repaint();
        }

        response
    }
}
