use egui::{Color32, Response, Sense, Widget};

use crate::theme::{ThemeColors, parse_hex_color, phosphor_font_id};

use super::IconButton;

const DEFAULT_BUTTON_SIZE: f32 = 20.0;
const DEFAULT_ICON_SIZE: f32 = 14.0;

impl Widget for IconButton {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let colors = ThemeColors::from_ctx(ui.ctx());

        let base_color = if self.disabled {
            ui.style().visuals.weak_text_color()
        } else {
            ui.style().visuals.text_color()
        };

        let dim = self.size.unwrap_or(DEFAULT_BUTTON_SIZE);
        let size = egui::vec2(dim, dim);
        let icon_size = self
            .icon_size
            .unwrap_or((dim / DEFAULT_BUTTON_SIZE) * DEFAULT_ICON_SIZE);

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

            if let Some(badge_color) = self.badge_color.as_deref().and_then(parse_hex_color) {
                let badge_center = egui::pos2(rect.right() - 6.0, rect.top() + 6.0);
                ui.painter().circle_filled(badge_center, 2.0, badge_color);
                ui.painter()
                    .circle_stroke(badge_center, 2.0, egui::Stroke::new(1.5, Color32::WHITE));
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
