use eframe::egui::{self, Color32, CornerRadius, Sense, Vec2};

use crate::{
    components::traits::StatelessComponent,
    theme::{Theme, ThemeColors},
};

pub struct ToggleSwitch;

pub struct ToggleSwitchProps {
    pub hover_text: Option<String>,
    pub enabled: bool,
}

pub enum ToggleSwitchEvent {
    Toggled(bool),
}

pub struct ToggleSwitchOutput {
    pub events: Vec<ToggleSwitchEvent>,
}

impl StatelessComponent for ToggleSwitch {
    type Props<'a> = ToggleSwitchProps;

    type Output = ToggleSwitchOutput;

    fn render(ui: &mut eframe::egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| Theme::default().colors())
        });
        let mut response = Self::toggle_switch(ui, props.enabled, colors.success, colors.surface2);
        let mut output = ToggleSwitchOutput { events: Vec::new() };

        response = response.on_hover_cursor(egui::CursorIcon::PointingHand);
        if let Some(hover_text) = props.hover_text {
            response = response.on_hover_text(hover_text);
        }

        if response.clicked() {
            output
                .events
                .push(ToggleSwitchEvent::Toggled(!props.enabled));
        }

        output
    }
}

impl ToggleSwitch {
    fn toggle_switch(
        ui: &mut egui::Ui,
        enabled: bool,
        on_color: Color32,
        off_color: Color32,
    ) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(36.0, 20.0), Sense::click());

        if ui.is_rect_visible(rect) {
            // Animate the toggle position
            let animation_id = egui::Id::new("toggle_switch_animation").with(response.id);
            let animation_progress = ui.ctx().animate_bool(animation_id, enabled);

            let bg = if enabled { on_color } else { off_color };
            ui.painter().rect_filled(rect, CornerRadius::same(10), bg);

            // Smoothly interpolate knob position
            let knob_x_off = rect.left() + 10.0;
            let knob_x_on = rect.right() - 10.0;
            let knob_x = egui::lerp(knob_x_off..=knob_x_on, animation_progress);

            ui.painter()
                .circle_filled(egui::pos2(knob_x, rect.center().y), 8.0, Color32::WHITE);
        }

        response
    }
}
