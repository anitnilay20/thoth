use eframe::egui::{self, Color32};
use serde::{Deserialize, Serialize};

use crate::{
    components::traits::StatelessComponent,
    theme::{Theme, ThemeColors, get_contrast_text_color},
};

pub struct Button;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub enum ButtonType {
    #[default]
    Elevated,
    Text,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
pub enum ButtonColor {
    #[default]
    Default,
    Primary,
    Secondary,
    Danger,
    Success,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ButtonProps {
    pub label: String,
    #[serde(rename = "button-type", default)]
    pub button_type: ButtonType,
    #[serde(default)]
    pub color: ButtonColor,
    #[serde(default)]
    pub hover_text: Option<String>,
    #[serde(default)]
    pub size: Option<f32>,
    #[serde(default)]
    pub width: Option<f32>,
    #[serde(default)]
    pub height: Option<f32>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub icon: Option<String>,
}

fn default_enabled() -> bool {
    true
}

impl Default for ButtonProps {
    fn default() -> Self {
        Self {
            label: String::new(),
            button_type: ButtonType::Elevated,
            color: ButtonColor::Default,
            hover_text: None,
            size: None,
            width: None,
            height: None,
            enabled: true,
            icon: None,
        }
    }
}

pub struct ButtonOutput {
    pub clicked: bool,
    pub response: egui::Response,
}

impl StatelessComponent for Button {
    type Props<'a> = ButtonProps;

    type Output = ButtonOutput;

    fn render(ui: &mut eframe::egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| Theme::default().colors())
        });

        let label = if let Some(icon) = props.icon {
            format!("{} {}", icon, props.label)
        } else {
            props.label
        };

        let size = props.size.unwrap_or(14.0);
        let text = egui::RichText::new(&label).size(size);

        let bg_color = match props.color {
            ButtonColor::Default => colors.surface2,
            ButtonColor::Primary => colors.primary,
            ButtonColor::Danger => colors.error,
            ButtonColor::Success => colors.success,
            ButtonColor::Secondary => colors.secondary,
        };

        let mut response = ui
            .add_enabled_ui(props.enabled, |ui| match props.button_type {
                ButtonType::Elevated => {
                    let text_color = get_contrast_text_color(bg_color);
                    Self::elevated_button(
                        ui,
                        text.color(text_color),
                        bg_color,
                        props.width,
                        props.height,
                    )
                }
                ButtonType::Text => Self::text_button(
                    ui,
                    &label,
                    props.size.unwrap_or(14.0),
                    bg_color,
                    colors.text,
                    props.width,
                    props.height,
                ),
            })
            .inner;

        if let Some(hover_text) = props.hover_text {
            response = response.on_hover_text(hover_text);
        }

        response = response.on_hover_cursor(egui::CursorIcon::PointingHand);

        ButtonOutput {
            clicked: response.clicked(),
            response,
        }
    }
}

impl Button {
    fn elevated_button(
        ui: &mut egui::Ui,
        text: egui::RichText,
        bg_color: Color32,
        width: Option<f32>,
        height: Option<f32>,
    ) -> egui::Response {
        let button = egui::Button::new(text)
            .stroke(egui::Stroke::NONE)
            .corner_radius(4);

        // Zero out expansion so the painted background never overflows the
        // allocated rect on hover (+1 px default) or shrinks on press (-1 px
        // default), which would visually shift neighbouring widgets.
        // Set weak_bg_fill per state so the hover/active darkening is handled
        // by egui's state machine rather than a static override.
        ui.scope(|ui| {
            {
                let w = &mut ui.visuals_mut().widgets;
                w.inactive.weak_bg_fill = bg_color;
                w.inactive.expansion = 0.0;
                w.inactive.bg_stroke = egui::Stroke::NONE;
                w.hovered.weak_bg_fill = bg_color.linear_multiply(0.85);
                w.hovered.expansion = 0.0;
                w.hovered.bg_stroke = egui::Stroke::NONE;
                w.active.weak_bg_fill = bg_color.linear_multiply(0.75);
                w.active.expansion = 0.0;
                w.active.bg_stroke = egui::Stroke::NONE;
            }
            if let (Some(w), Some(h)) = (width, height) {
                ui.add_sized(egui::vec2(w, h), button)
            } else if let Some(w) = width {
                ui.add_sized(egui::vec2(w, 0.0), button)
            } else if let Some(h) = height {
                ui.add_sized(egui::vec2(0.0, h), button)
            } else {
                ui.add(button)
            }
        })
        .inner
    }

    fn text_button(
        ui: &mut egui::Ui,
        label: &str,
        size: f32,
        normal_color: Color32,
        hover_color: Color32,
        width: Option<f32>,
        height: Option<f32>,
    ) -> egui::Response {
        // Render the Button with invisible text so it handles sizing and
        // interaction rect allocation — then paint the visible text ourselves
        // using response.hovered() which is current-frame accurate and
        // per-widget (no shared ID issues).
        let invisible = egui::RichText::new(label)
            .size(size)
            .color(Color32::TRANSPARENT);
        let button = egui::Button::new(invisible)
            .fill(Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE);

        let response = ui
            .scope(|ui| {
                let w = &mut ui.visuals_mut().widgets;
                w.inactive.weak_bg_fill = Color32::TRANSPARENT;
                w.inactive.expansion = 0.0;
                w.inactive.bg_stroke = egui::Stroke::NONE;
                w.hovered.weak_bg_fill = Color32::TRANSPARENT;
                w.hovered.expansion = 0.0;
                w.hovered.bg_stroke = egui::Stroke::NONE;
                w.active.weak_bg_fill = Color32::TRANSPARENT;
                w.active.expansion = 0.0;
                w.active.bg_stroke = egui::Stroke::NONE;

                if let (Some(w), Some(h)) = (width, height) {
                    ui.add_sized(egui::vec2(w, h), button)
                } else if let Some(w) = width {
                    ui.add_sized(egui::vec2(w, 0.0), button)
                } else if let Some(h) = height {
                    ui.add_sized(egui::vec2(0.0, h), button)
                } else {
                    ui.add(button)
                }
            })
            .inner;

        if ui.is_rect_visible(response.rect) {
            let color = if response.is_pointer_button_down_on() || response.hovered() {
                hover_color
            } else {
                normal_color
            };
            ui.painter().text(
                response.rect.center(),
                egui::Align2::CENTER_CENTER,
                label,
                egui::FontId::proportional(size),
                color,
            );
        }

        response
    }
}
