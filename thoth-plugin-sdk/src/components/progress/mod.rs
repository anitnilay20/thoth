use bon::Builder;
use serde::{Deserialize, Serialize};

/// Track height — design `.track{height:6px}`.
#[cfg(feature = "egui")]
const TRACK_HEIGHT: f32 = 6.0;
/// Fixed width of the value readout column — design `.pv{width:34px}`. Fixed so
/// stacked bars line up no matter how wide their readout text is.
#[cfg(feature = "egui")]
const READOUT_WIDTH: f32 = 34.0;
/// Gap between the track and its readout — design `.prow{gap:10px}`.
#[cfg(feature = "egui")]
const READOUT_GAP: f32 = 10.0;

/// A horizontal progress bar.
///
/// ```
/// use thoth_plugin_sdk::components::Progress;
///
/// let bar = Progress::builder().value(0.6).color("success").build();
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
#[non_exhaustive]
pub struct Progress {
    /// Completion in `0.0..=1.0`.
    pub value: f64,
    /// Optional fill colour — a semantic token (e.g. `"success"`, `"warning"`,
    /// `"info"`) or a `#rrggbb` hex. Defaults to the theme accent when unset.
    #[serde(default)]
    pub color: Option<String>,
    /// Optional fixed bar height in points.
    #[serde(default)]
    pub height: Option<f32>,
    /// Optional value readout shown to the right of the track (e.g. `"62%"` or
    /// `"done"`) — design `.pv`. The text is composed by the caller; the
    /// component only positions it.
    #[serde(default)]
    pub readout: Option<String>,
}

#[cfg(feature = "egui")]
impl egui::Widget for Progress {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        use crate::theme::{FONT_CAPTION, RADIUS_PILL, ThemeColors};

        let colors = ThemeColors::from_ctx(ui.ctx());
        // The fill colour is semantic (green for done, yellow for a warning …),
        // so it stays whatever the caller asked for; the accent is only a default.
        let fill = self
            .color
            .as_deref()
            .and_then(|token| crate::theme::resolve_color(token, &colors))
            .unwrap_or(colors.accent);
        let track_h = self.height.unwrap_or(TRACK_HEIGHT);

        // Lay the readout out first: it fixes the row's height as well as the
        // width left over for the track (design `.track{flex:1}`).
        let readout = self.readout.as_deref().filter(|t| !t.is_empty()).map(|t| {
            ui.painter().layout_no_wrap(
                t.to_owned(),
                egui::FontId::monospace(FONT_CAPTION),
                colors.fg_muted,
            )
        });
        let reserved = match &readout {
            Some(_) => READOUT_WIDTH + READOUT_GAP,
            None => 0.0,
        };
        let row_h = readout
            .as_ref()
            .map_or(track_h, |g| g.size().y.max(track_h));
        let track_w = (ui.available_width() - reserved).max(0.0);

        let (rect, response) =
            ui.allocate_exact_size(egui::vec2(track_w + reserved, row_h), egui::Sense::hover());

        if ui.is_rect_visible(rect) {
            // Fully-round track and fill — design `border-radius:999px` on both,
            // with the fill kept inside the track's bounds.
            let pill = egui::CornerRadius::same(RADIUS_PILL as u8);
            let track = egui::Rect::from_min_size(
                egui::pos2(rect.left(), rect.center().y - track_h / 2.0),
                egui::vec2(track_w, track_h),
            );
            ui.painter().rect_filled(track, pill, colors.surface_raised);
            let filled = track_w * self.value.clamp(0.0, 1.0) as f32;
            if filled > 0.0 {
                let bar = egui::Rect::from_min_size(track.min, egui::vec2(filled, track_h));
                ui.painter().rect_filled(bar, pill, fill);
            }
            if let Some(galley) = readout {
                // Right-aligned in its fixed column — design `.pv{text-align:right}`.
                let pos = egui::pos2(
                    rect.right() - galley.size().x,
                    rect.center().y - galley.size().y / 2.0,
                );
                ui.painter().galley(pos, galley, colors.fg_muted);
            }
        }

        response
    }
}
