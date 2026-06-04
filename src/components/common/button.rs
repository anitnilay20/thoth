use eframe::egui::{self, Color32};
use egui::text::{LayoutJob, TextFormat};
use serde::{Deserialize, Serialize};

use crate::{
    components::traits::StatelessComponent,
    theme::{Theme, ThemeColors, get_contrast_text_color, phosphor_font_id},
};

pub struct Button;

#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub enum ButtonType {
    #[default]
    Elevated,
    Text,
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Serialize)]
pub enum ButtonSize {
    Small,
    #[default]
    Medium,
    Large,
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
    pub button_size: ButtonSize,
    #[serde(default)]
    pub hover_text: Option<String>,
    /// Font size override — if unset, derived from `button_size`.
    #[serde(default)]
    pub size: Option<f32>,
    #[serde(default)]
    pub width: Option<f32>,
    /// Height override — if unset, derived from `button_size`.
    #[serde(default)]
    pub height: Option<f32>,
    #[serde(default = "default_enabled")]
    pub enabled: bool,
    #[serde(default)]
    pub icon: Option<String>,
    /// Stretch the button to the full available width of its container.
    #[serde(rename = "full-width", default)]
    pub full_width: bool,
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
            button_size: ButtonSize::Medium,
            hover_text: None,
            size: None,
            width: None,
            height: None,
            enabled: true,
            icon: None,
            full_width: false,
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

        let (default_font, default_h) = match props.button_size {
            ButtonSize::Small => (11.0_f32, 24.0_f32),
            ButtonSize::Medium => (13.0_f32, 28.0_f32),
            ButtonSize::Large => (15.0_f32, 32.0_f32),
        };
        let size = props.size.unwrap_or(default_font);
        let height = Some(props.height.unwrap_or(default_h));
        let icon = props.icon.as_deref();
        // Full-width buttons stretch to the container's available width.
        let width = if props.full_width {
            Some(ui.available_width())
        } else {
            props.width
        };

        let bg_color = match props.color {
            ButtonColor::Default => colors.surface_active,
            ButtonColor::Primary => colors.accent,
            ButtonColor::Danger => colors.error,
            ButtonColor::Success => colors.success,
            ButtonColor::Secondary => colors.accent_secondary,
        };

        let mut response = ui
            .add_enabled_ui(props.enabled, |ui| match props.button_type {
                ButtonType::Elevated => {
                    let text_color = get_contrast_text_color(bg_color);
                    Self::elevated_button(
                        ui,
                        Self::make_layout_job(icon, &props.label, size, text_color),
                        bg_color,
                        width,
                        height,
                    )
                }
                ButtonType::Text => Self::text_button(
                    ui,
                    &props.label,
                    icon,
                    size,
                    bg_color,
                    colors.fg,
                    width,
                    height,
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
    fn make_layout_job(icon: Option<&str>, label: &str, size: f32, color: Color32) -> LayoutJob {
        let mut job = LayoutJob::default();
        if let Some(ic) = icon {
            job.append(
                ic,
                0.0,
                TextFormat {
                    font_id: phosphor_font_id(size),
                    color,
                    valign: egui::Align::Center,
                    ..Default::default()
                },
            );
            job.append(
                " ",
                0.0,
                TextFormat {
                    font_id: egui::FontId::proportional(size),
                    color,
                    valign: egui::Align::Center,
                    ..Default::default()
                },
            );
        }
        job.append(
            label,
            0.0,
            TextFormat {
                font_id: egui::FontId::proportional(size),
                color,
                valign: egui::Align::Center,
                ..Default::default()
            },
        );
        job
    }

    fn elevated_button(
        ui: &mut egui::Ui,
        job: LayoutJob,
        bg_color: Color32,
        width: Option<f32>,
        height: Option<f32>,
    ) -> egui::Response {
        let galley = ui.painter().layout_job(job);
        let desired = egui::vec2(
            width.unwrap_or(galley.size().x + 20.0),
            height.unwrap_or(galley.size().y + 10.0),
        );
        let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());

        if ui.is_rect_visible(rect) {
            let bg = if !ui.is_enabled() {
                bg_color.linear_multiply(0.35)
            } else if response.is_pointer_button_down_on() {
                bg_color.linear_multiply(0.75)
            } else if response.hovered() {
                bg_color.linear_multiply(0.85)
            } else {
                bg_color
            };
            ui.painter().rect_filled(rect, 4.0, bg);

            let text_color = get_contrast_text_color(bg_color);
            let pos = rect.center() - galley.rect.center().to_vec2();
            // Faux bold: second pass shifted 0.5 px right thickens vertical strokes.
            ui.painter()
                .galley(pos + egui::vec2(0.5, 0.0), galley.clone(), text_color);
            ui.painter().galley(pos, galley, text_color);
        }

        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    #[allow(clippy::too_many_arguments)]
    fn text_button(
        ui: &mut egui::Ui,
        label: &str,
        icon: Option<&str>,
        size: f32,
        normal_color: Color32,
        hover_color: Color32,
        width: Option<f32>,
        height: Option<f32>,
    ) -> egui::Response {
        // Transparent sizing job — allocates correct space for icon+label.
        let sizing_job = Self::make_layout_job(icon, label, size, Color32::TRANSPARENT);
        let button = egui::Button::new(sizing_job)
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

                if let Some(w) = width {
                    let h = height.unwrap_or(0.0);
                    ui.add_sized(egui::vec2(w, h), button)
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
            let paint_job = Self::make_layout_job(icon, label, size, color);
            let galley = ui.painter().layout_job(paint_job);
            let pos = response.rect.center() - galley.rect.center().to_vec2();
            ui.painter()
                .galley(pos + egui::vec2(0.5, 0.0), galley.clone(), color);
            ui.painter().galley(pos, galley, color);
        }

        response
    }
}
