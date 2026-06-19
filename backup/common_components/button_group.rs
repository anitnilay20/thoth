use eframe::egui;

use crate::components::common::traits::StatelessComponent;
use crate::theme::{Theme, ThemeColors};

/// A single option in a ButtonGroup.
pub struct ButtonGroupItem<'a> {
    /// The value emitted when this item is selected.
    pub value: &'a str,
    /// The label displayed on the button.
    pub label: &'a str,
}

pub struct ButtonGroupProps<'a> {
    pub items: &'a [ButtonGroupItem<'a>],
    /// The currently active value.
    pub active: &'a str,
}

pub struct ButtonGroupOutput {
    /// The value of the item the user clicked, if any.
    pub selected: Option<String>,
}

/// A pill-style segmented button group — one selection at a time.
///
/// The active item is filled with `colors.surface_active`; inactive items are
/// transparent text buttons that highlight on hover.
pub struct ButtonGroup;

impl StatelessComponent for ButtonGroup {
    type Props<'a> = ButtonGroupProps<'a>;
    type Output = ButtonGroupOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| Theme::default().colors())
        });

        let mut selected: Option<String> = None;

        egui::Frame::new()
            .fill(colors.bg_panel)
            .corner_radius(6)
            .inner_margin(egui::Margin::same(2))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.horizontal(|ui| {
                    for item in props.items {
                        let is_active = item.value == props.active;
                        let response = render_segment(ui, item.label, is_active, &colors);
                        if response.clicked() && !is_active {
                            selected = Some(item.value.to_string());
                        }
                    }
                });
            });

        ButtonGroupOutput { selected }
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

    // Approximate width: proportional fonts average ~0.6× font_size per character.
    let text_w = label.len() as f32 * font_size * 0.6;
    let desired = egui::vec2(text_w + padding.x * 2.0, font_size + padding.y * 2.0);

    let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());

    if ui.is_rect_visible(rect) {
        if is_active {
            ui.painter().rect_filled(rect, 4.0, colors.surface_active);
        } else if response.hovered() {
            ui.painter().rect_filled(
                rect,
                4.0,
                egui::Color32::from_rgba_premultiplied(
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
            egui::Align2::CENTER_CENTER,
            label,
            egui::FontId::proportional(font_size),
            text_color,
        );
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
    }

    response
}
