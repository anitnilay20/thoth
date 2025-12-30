use crate::components::traits::StatelessComponent;
use eframe::egui;

// Default button and icon sizes
const DEFAULT_BUTTON_SIZE: f32 = 20.0;
const DEFAULT_ICON_SIZE: f32 = 14.0;

/// Props for the IconButton component
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
    #[allow(dead_code)]
    pub response: egui::Response,
}

/// Icon button component with consistent padding and styling
pub struct IconButton;

impl StatelessComponent for IconButton {
    type Props<'a> = IconButtonProps<'a>;
    type Output = IconButtonOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        // Get theme colors
        let base_color = if props.disabled {
            ui.style().visuals.weak_text_color()
        } else {
            ui.style().visuals.text_color()
        };
        let hover_color = ui.style().visuals.strong_text_color();

        // Create button with custom styling
        let size = props
            .size
            .unwrap_or(egui::vec2(DEFAULT_BUTTON_SIZE, DEFAULT_BUTTON_SIZE));
        // Scale icon size proportionally to button size
        let icon_size = (size.y / DEFAULT_BUTTON_SIZE) * DEFAULT_ICON_SIZE;

        let button = egui::Button::new(
            egui::RichText::new(props.icon)
                .size(icon_size)
                .color(base_color),
        )
        .frame(props.frame)
        .min_size(size);

        let response = ui.add_enabled(!props.disabled, button);

        // Change cursor based on state
        if response.hovered() {
            if props.disabled {
                ui.ctx().set_cursor_icon(egui::CursorIcon::NotAllowed);
            } else {
                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
            }
        }

        // Apply hover color by redrawing the text
        if response.hovered() {
            let painter = ui.painter();
            painter.text(
                response.rect.center(),
                egui::Align2::CENTER_CENTER,
                props.icon,
                egui::FontId::proportional(icon_size),
                hover_color,
            );
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

        // Draw badge if provided
        if let Some(badge_color) = props.badge_color {
            let button_rect = response.rect;
            let badge_center = egui::pos2(button_rect.right() - 6.0, button_rect.top() + 6.0);
            let badge_radius = 2.0;

            ui.painter()
                .circle_filled(badge_center, badge_radius, badge_color);
            ui.painter().circle_stroke(
                badge_center,
                badge_radius,
                egui::Stroke::new(1.5, egui::Color32::WHITE),
            );
        }

        IconButtonOutput {
            clicked: response.clicked(),
            response,
        }
    }
}
