use egui::{Align2, Color32, CursorIcon, FontId, Margin, Sense, Widget};

use crate::theme::ThemeColors;

use super::ButtonGroups;

impl ButtonGroups {
    /// Render the segmented control and report the user's selection.
    ///
    /// The active segment is `self.active`. The returned
    /// [`egui::InnerResponse::inner`] is `Some(value)` when the user clicked a
    /// *different* segment this frame, and `None` otherwise. Write that value
    /// back into your own state and pass it in as `active` next frame.
    pub fn show(&self, ui: &mut egui::Ui) -> egui::InnerResponse<Option<String>> {
        let colors = ThemeColors::from_ctx(ui.ctx());
        let mut selected: Option<String> = None;

        let frame = egui::Frame::new()
            .fill(colors.bg_panel)
            .corner_radius(6)
            .inner_margin(Margin::same(2))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.horizontal(|ui| {
                    for item in &self.items {
                        let is_active = item.value == self.active;
                        let response = render_segment(ui, &item.label, is_active, &colors);
                        if response.clicked() && !is_active {
                            selected = Some(item.value.clone());
                        }
                    }
                });
            });

        egui::InnerResponse::new(selected, frame.response)
    }
}

fn render_segment(
    ui: &mut egui::Ui,
    label: &str,
    is_active: bool,
    colors: &ThemeColors,
) -> egui::Response {
    let font_size = 12.5_f32;
    let padding = egui::vec2(10.0, 4.0);
    // Approximate width: proportional fonts average ~0.6× font_size per char.
    let text_w = label.len() as f32 * font_size * 0.6;
    let desired = egui::vec2(text_w + padding.x * 2.0, font_size + padding.y * 2.0);

    let (rect, response) = ui.allocate_exact_size(desired, Sense::click());

    if ui.is_rect_visible(rect) {
        if is_active {
            ui.painter().rect_filled(rect, 4.0, colors.surface_active);
        } else if response.hovered() {
            ui.painter().rect_filled(
                rect,
                4.0,
                Color32::from_rgba_premultiplied(
                    colors.surface_raised.r(),
                    colors.surface_raised.g(),
                    colors.surface_raised.b(),
                    60,
                ),
            );
        }

        let text_color = if is_active || response.hovered() {
            colors.fg
        } else {
            colors.fg_muted
        };
        ui.painter().text(
            rect.center(),
            Align2::CENTER_CENTER,
            label,
            FontId::proportional(font_size),
            text_color,
        );
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }

    response
}

impl Widget for ButtonGroups {
    /// Convenience for `ui.add(group)`. Renders the group but **discards** the
    /// selection — use [`ButtonGroups::show`] when you need it.
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.show(ui).response
    }
}
