use eframe::egui::{self, Color32, CornerRadius, Sense, Vec2};

#[inline]
fn lerp_u8(a: u8, b: u8, t: f32) -> u8 {
    (a as f32 + (b as f32 - a as f32) * t.clamp(0.0, 1.0)) as u8
}

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
        let mut response = Self::toggle_switch(
            ui,
            props.enabled,
            colors.accent,
            colors.surface_active,
            colors.bg,
            colors.fg,
        );
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
        on_pill_color: Color32,
        off_pill_color: Color32,
    ) -> egui::Response {
        let (rect, response) = ui.allocate_exact_size(Vec2::new(36.0, 20.0), Sense::click());

        if ui.is_rect_visible(rect) {
            // Animate the toggle position
            let animation_id = egui::Id::new("toggle_switch_animation").with(response.id);
            let animation_progress = ui.ctx().animate_bool(animation_id, enabled);

            let bg = egui::Color32::from_rgba_unmultiplied(
                lerp_u8(off_color.r(), on_color.r(), animation_progress),
                lerp_u8(off_color.g(), on_color.g(), animation_progress),
                lerp_u8(off_color.b(), on_color.b(), animation_progress),
                lerp_u8(off_color.a(), on_color.a(), animation_progress),
            );
            ui.painter().rect_filled(rect, CornerRadius::same(10), bg);

            // Smoothly interpolate knob position
            let knob_x_off = rect.left() + 10.0;
            let knob_x_on = rect.right() - 10.0;
            let knob_x = egui::lerp(knob_x_off..=knob_x_on, animation_progress);
            let pill_color = egui::Color32::from_rgba_unmultiplied(
                lerp_u8(off_pill_color.r(), on_pill_color.r(), animation_progress),
                lerp_u8(off_pill_color.g(), on_pill_color.g(), animation_progress),
                lerp_u8(off_pill_color.b(), on_pill_color.b(), animation_progress),
                lerp_u8(off_pill_color.a(), on_pill_color.a(), animation_progress),
            );

            ui.painter()
                .circle_filled(egui::pos2(knob_x, rect.center().y), 8.0, pill_color);
        }

        response
    }
}
