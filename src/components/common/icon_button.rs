use crate::components::traits::StatelessComponent;
use crate::theme::ThemeColors;
use eframe::egui;
use egui::{Color32, Sense};

// Default button and icon sizes
const DEFAULT_BUTTON_SIZE: f32 = 20.0;
const DEFAULT_ICON_SIZE: f32 = 14.0;

/// Props for the IconButton component
#[derive(Default)]
pub struct IconButtonProps<'a> {
    /// The icon to display (e.g., from egui_phosphor)
    pub icon: &'a str,
    /// Whether to show the button frame
    pub frame: bool,
    /// Optional tooltip text
    pub tooltip: Option<&'a str>,
    /// Optional badge color (draws a small circle in top-right)
    pub badge_color: Option<egui::Color32>,
    /// Optional custom size (defaults to 20.0 x 20.0)
    pub size: Option<egui::Vec2>,
    /// Whether the button is disabled
    pub disabled: bool,
}

/// Output from the IconButton component
pub struct IconButtonOutput {
    pub clicked: bool,
    pub response: egui::Response,
}

/// Icon button component with consistent padding and styling
pub struct IconButton;

impl StatelessComponent for IconButton {
    type Props<'a> = IconButtonProps<'a>;
    type Output = IconButtonOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        // Get theme colors
        let colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| crate::theme::Theme::default().colors())
        });

        let base_color = if props.disabled {
            ui.style().visuals.weak_text_color()
        } else {
            ui.style().visuals.text_color()
        };

        let size = props
            .size
            .unwrap_or(egui::vec2(DEFAULT_BUTTON_SIZE, DEFAULT_BUTTON_SIZE));
        let icon_size = (size.y / DEFAULT_BUTTON_SIZE) * DEFAULT_ICON_SIZE;

        // Allocate the button rect FIRST so we can paint the hover background
        // before placing the icon widget (correct z-order: bg behind glyph).
        let sense = if props.disabled {
            Sense::hover()
        } else {
            Sense::click()
        };
        let (rect, response) = ui.allocate_exact_size(size, sense);

        if ui.is_rect_visible(rect) {
            // Paint frame background if requested
            if props.frame {
                ui.painter().rect_filled(rect, 4.0, colors.surface1);
            }

            // Paint hover background before the icon so it sits behind the glyph
            if response.hovered() && !props.disabled {
                let hover_bg = Color32::from_rgba_premultiplied(
                    colors.surface1.r(),
                    colors.surface1.g(),
                    colors.surface1.b(),
                    40, // Low alpha for subtle effect
                );
                ui.painter().rect_filled(rect, 4.0, hover_bg);
            }

            // Paint the icon glyph on top of the background
            // Paint the icon glyph centred in the allocated rect.
            ui.painter().text(
                rect.center(),
                egui::Align2::CENTER_CENTER,
                props.icon,
                egui::FontId::proportional(icon_size),
                base_color,
            );

            // Draw badge if provided (on top of everything)
            if let Some(badge_color) = props.badge_color {
                let badge_center = egui::pos2(rect.right() - 6.0, rect.top() + 6.0);
                ui.painter().circle_filled(badge_center, 2.0, badge_color);
                ui.painter().circle_stroke(
                    badge_center,
                    2.0,
                    egui::Stroke::new(1.5, egui::Color32::WHITE),
                );
            }
        }

        // Change cursor based on state
        if response.hovered() {
            if props.disabled {
                ui.ctx().set_cursor_icon(egui::CursorIcon::NotAllowed);
            } else {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        }

        let response = if let Some(tooltip) = props.tooltip {
            response.on_hover_text(tooltip)
        } else {
            response
        };

        // Add accessibility info for screen readers
        response.widget_info(|| {
            let label = props.tooltip.unwrap_or("Button");
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
        });

        let clicked = response.clicked() && !props.disabled;

        IconButtonOutput { clicked, response }
    }
}
