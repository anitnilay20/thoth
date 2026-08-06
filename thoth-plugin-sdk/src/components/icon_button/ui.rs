use egui::{Color32, Response, Sense, Shadow, Stroke, Widget};

use crate::components::Size;
use crate::theme::{
    ICON_CONTROL, RADIUS_CONTROL, ThemeColors, edge_stroke, get_contrast_text_color, glow_shadow,
    phosphor_font_id, resolve_color, with_alpha,
};

use super::{IconButton, IconButtonSelectedStyle};

const DEFAULT_BUTTON_SIZE: f32 = 20.0;
const DEFAULT_ICON_SIZE: f32 = 14.0;

/// Ghost hover wash — design `.ib.ghost:hover{background:text@8%}`.
const GHOST_HOVER_ALPHA: u8 = 20; // 8% of 255
/// Disabled icon buttons drop to 42% opacity — design `.disabled{opacity:.42}`.
const DISABLED_OPACITY: f32 = 0.42;
/// Badge dot diameter — design `.ib .bdot{width:9px;height:9px}`.
const BADGE_DIAMETER: f32 = 9.0;
/// The dot hangs 2px outside the button's top-right corner — design
/// `.bdot{top:-2px;right:-2px}`, i.e. a *negative* inset.
const BADGE_INSET: f32 = -2.0;
/// …and is ringed in the panel colour so it reads against any background —
/// design `.bdot{box-shadow:0 0 0 2px var(--base)}`.
const BADGE_RING_WIDTH: f32 = 2.0;
/// Selected wash — design `.bell{background:color-mix(mauve 14%,transparent)}`.
const SELECTED_WASH_ALPHA: u8 = 36; // 14% of 255

impl IconButton {
    /// `(square dimension, default glyph size)` for this icon button's size
    /// preset. Design `.ib` is 26px square with a 15px glyph ([`ICON_CONTROL`]);
    /// `Small`/`Large` step 2px either side of that glyph size.
    fn dims(&self) -> (f32, f32) {
        // An explicit pixel override wins; its glyph scales from the 20px base.
        if let Some(px) = self.size_px {
            return (px, (px / DEFAULT_BUTTON_SIZE) * DEFAULT_ICON_SIZE);
        }
        // Square size shares the same heights as Button/Select for the same size
        // level (from `Size::metrics`), so a toolbar of mixed controls lines up.
        // The glyph size is icon-button-specific. `(square, glyph)`.
        let square = self.size.metrics().1;
        let glyph = match self.size {
            Size::Small => ICON_CONTROL - 2.0,
            Size::Medium => ICON_CONTROL,
            Size::Large => ICON_CONTROL + 2.0,
        };
        (square, glyph)
    }

    /// `(radius, offset)` of the badge dot: its radius, and how far its centre
    /// sits in from the button's top-right corner. A negative
    /// [`badge_inset`](IconButton::badge_inset) — the default — pushes the dot
    /// out past the corner; a positive one tucks it inside.
    fn badge_geometry(&self) -> (f32, f32) {
        let radius = self.badge_size.unwrap_or(BADGE_DIAMETER) / 2.0;
        (radius, radius + self.badge_inset.unwrap_or(BADGE_INSET))
    }
}

/// Resolved paint colours for one icon-button state.
struct Visual {
    fill: Option<Color32>,
    glyph: Color32,
    stroke: Stroke,
    shadow: Option<Shadow>,
}

impl Visual {
    /// Design `.ib` (framed), `.ib.ghost`, `.ib.active` and `.bell`.
    fn resolve(
        framed: bool,
        hovered: bool,
        selected: bool,
        selected_style: IconButtonSelectedStyle,
        colors: &ThemeColors,
    ) -> Self {
        match (selected, framed, hovered) {
            // `.bell` — a 14% accent wash under an `fg` glyph, no glow.
            (true, _, _) if selected_style == IconButtonSelectedStyle::Wash => Self {
                fill: Some(with_alpha(colors.accent, SELECTED_WASH_ALPHA)),
                glyph: colors.fg,
                stroke: Stroke::NONE,
                shadow: None,
            },
            // `.ib.active` — mauve fill with a matching glow, framed or not.
            (true, _, _) => Self {
                fill: Some(colors.accent),
                glyph: get_contrast_text_color(colors.accent),
                stroke: Stroke::NONE,
                shadow: Some(glow_shadow(colors.accent)),
            },
            // `.ib:hover`
            (false, true, true) => Self {
                fill: Some(colors.surface_raised),
                glyph: colors.fg,
                stroke: edge_stroke(colors),
                shadow: None,
            },
            // `.ib`
            (false, true, false) => Self {
                fill: Some(colors.surface),
                glyph: colors.fg_subtle(),
                stroke: edge_stroke(colors),
                shadow: None,
            },
            // `.ib.ghost:hover`
            (false, false, true) => Self {
                fill: Some(with_alpha(colors.fg, GHOST_HOVER_ALPHA)),
                glyph: colors.fg,
                stroke: Stroke::NONE,
                shadow: None,
            },
            // `.ib.ghost`
            (false, false, false) => Self {
                fill: None,
                glyph: colors.fg_muted,
                stroke: Stroke::NONE,
                shadow: None,
            },
        }
    }

    /// Design `.disabled{opacity:.42;box-shadow:none}`.
    fn dimmed(self) -> Self {
        Self {
            fill: self.fill.map(|c| c.gamma_multiply(DISABLED_OPACITY)),
            glyph: self.glyph.gamma_multiply(DISABLED_OPACITY),
            stroke: Stroke::new(
                self.stroke.width,
                self.stroke.color.gamma_multiply(DISABLED_OPACITY),
            ),
            shadow: None,
        }
    }
}

impl Widget for IconButton {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let colors = ThemeColors::from_ctx(ui.ctx());

        let (dim, default_icon) = self.dims();
        let size = egui::vec2(dim, dim);
        let icon_size = self.icon_size.unwrap_or(default_icon);

        let sense = if self.disabled {
            Sense::hover()
        } else {
            Sense::click()
        };
        let (rect, response) = ui.allocate_exact_size(size, sense);

        if ui.is_rect_visible(rect) {
            let hovered = response.hovered() && !self.disabled;
            let mut visual = Visual::resolve(
                self.frame,
                hovered,
                self.selected,
                self.selected_style,
                &colors,
            );
            // An explicit glyph colour outranks the state's — design `.bell`
            // keeps its `--text` glyph washed, hovered, or idle.
            if let Some(c) = self
                .glyph_color
                .as_deref()
                .and_then(|c| resolve_color(c, &colors))
            {
                visual.glyph = c;
            }
            let visual = if self.disabled {
                visual.dimmed()
            } else {
                visual
            };

            if let Some(shadow) = visual.shadow {
                ui.painter().add(shadow.as_shape(rect, RADIUS_CONTROL));
            }
            if let Some(fill) = visual.fill {
                ui.painter().rect_filled(rect, RADIUS_CONTROL, fill);
            }
            if visual.stroke != Stroke::NONE {
                ui.painter().rect_stroke(
                    rect,
                    RADIUS_CONTROL,
                    visual.stroke,
                    egui::StrokeKind::Inside,
                );
            }

            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &self.icon,
                phosphor_font_id(icon_size),
                visual.glyph,
            );

            if let Some(badge_color) = self
                .badge_color
                .as_deref()
                .and_then(|c| resolve_color(c, &colors))
            {
                // The dot straddles the top-right corner: by default it overhangs
                // by 2px, but a tucked-in inset pulls it inside the box instead.
                let (radius, offset) = self.badge_geometry();
                let badge_center = egui::pos2(rect.right() - offset, rect.top() + offset);
                let ring = self
                    .badge_ring_color
                    .as_deref()
                    .and_then(|c| resolve_color(c, &colors))
                    .unwrap_or(colors.bg);
                // Ring first, sitting just outside the dot, then the dot itself.
                ui.painter().circle_stroke(
                    badge_center,
                    radius + BADGE_RING_WIDTH / 2.0,
                    Stroke::new(BADGE_RING_WIDTH, ring),
                );
                ui.painter()
                    .circle_filled(badge_center, radius, badge_color);
            }
        }

        if response.hovered() {
            let cursor = if self.disabled {
                egui::CursorIcon::NotAllowed
            } else {
                egui::CursorIcon::PointingHand
            };
            ui.ctx().set_cursor_icon(cursor);
        }

        let tooltip = self.tooltip;
        let response = match tooltip.as_deref() {
            Some(t) => crate::theme::hover_text(response, t.to_owned()),
            None => response,
        };

        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                ui.is_enabled(),
                tooltip.as_deref().unwrap_or("Button"),
            )
        });

        response
    }
}

#[cfg(test)]
mod tests {
    use super::{BADGE_DIAMETER, BADGE_INSET};
    use crate::components::{IconButton, IconButtonSelectedStyle};

    #[test]
    fn defaults_keep_the_original_badge_and_selected_chrome() {
        let b = IconButton::builder().icon("x").build();
        assert_eq!(b.selected_style, IconButtonSelectedStyle::Solid);
        assert!(b.badge_size.is_none());
        assert!(b.badge_inset.is_none());
        assert!(b.badge_ring_color.is_none());
        assert!(b.glyph_color.is_none());
        // Design `.ib .bdot`: a 9px dot overhanging the corner by 2px, i.e. a
        // centre 2.5px in from it.
        let (radius, offset) = b.badge_geometry();
        assert_eq!(radius, BADGE_DIAMETER / 2.0);
        assert_eq!(offset, BADGE_DIAMETER / 2.0 + BADGE_INSET);
        assert_eq!(offset, 2.5);
    }

    #[test]
    fn the_bells_dot_tucks_inside_the_corner() {
        // Design `.bell .bd{width:8px;top:3px;right:3px}` — a centre 7px in.
        let b = IconButton::builder()
            .icon("x")
            .badge_size(8.0)
            .badge_inset(3.0)
            .build();
        assert_eq!(b.badge_geometry(), (4.0, 7.0));
    }
}
