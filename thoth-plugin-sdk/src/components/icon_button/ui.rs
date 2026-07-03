use egui::{Color32, Response, Sense, Widget};

use crate::components::Size;
use crate::theme::{ThemeColors, phosphor_font_id, resolve_color};

use super::IconButton;

const DEFAULT_BUTTON_SIZE: f32 = 20.0;
const DEFAULT_ICON_SIZE: f32 = 14.0;

impl IconButton {
    /// `(square dimension, default glyph size)` for this icon button's size
    /// preset. Icon buttons stay compact, so `Medium` keeps the historical 20px
    /// default; `Small`/`Large` step around it.
    fn dims(&self) -> (f32, f32) {
        // An explicit pixel override wins; its glyph scales from the 20px base.
        if let Some(px) = self.size_px {
            return (px, (px / DEFAULT_BUTTON_SIZE) * DEFAULT_ICON_SIZE);
        }
        // Square size shares the same heights as Button/Select for the same size
        // level, so a toolbar of mixed controls lines up. `(square, glyph)`.
        match self.size {
            Size::Small => (24.0, 14.0),
            Size::Medium => (28.0, 16.0),
            Size::Large => (32.0, 18.0),
        }
    }
}

impl Widget for IconButton {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let colors = ThemeColors::from_ctx(ui.ctx());

        let base_color = if self.disabled {
            ui.style().visuals.weak_text_color()
        } else {
            ui.style().visuals.text_color()
        };

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
            if self.frame {
                ui.painter().rect_filled(rect, 4.0, colors.surface_raised);
            }

            if response.hovered() && !self.disabled {
                let hover_bg = Color32::from_rgba_premultiplied(
                    colors.surface_raised.r(),
                    colors.surface_raised.g(),
                    colors.surface_raised.b(),
                    40,
                );
                ui.painter().rect_filled(rect, 4.0, hover_bg);
            }

            let icon_color = if (response.hovered() && !self.disabled) || self.selected {
                colors.accent
            } else {
                base_color
            };
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                &self.icon,
                phosphor_font_id(icon_size),
                icon_color,
            );

            if let Some(badge_color) = self
                .badge_color
                .as_deref()
                .and_then(|c| resolve_color(c, &colors))
            {
                let badge_center = egui::pos2(rect.right() - 6.0, rect.top() + 6.0);
                ui.painter().circle_filled(badge_center, 2.0, badge_color);
                ui.painter().circle_stroke(
                    badge_center,
                    2.0,
                    egui::Stroke::new(1.5, Color32::WHITE),
                );
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
            Some(t) => response.on_hover_text(t.to_owned()),
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
