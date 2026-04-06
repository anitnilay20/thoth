use eframe::egui::{self, Color32};

use crate::{components::traits::StatelessComponent, theme::{Theme, ThemeColors, get_contrast_text_color}};

pub struct Button;

#[derive(Clone, Copy, Debug)]
pub enum ButtonType {
    Elevated,
    Text,
}

#[derive(Clone, Copy, Debug)]
pub enum ButtonColor {
    Default,
    Danger,
    Success,
}

pub struct ButtonProps {
    pub label: String,
    pub button_type: ButtonType,
    pub color: ButtonColor,
    pub hover_text: Option<String>,
    pub size: Option<f32>,
    /// Optional custom width in pixels
    pub width: Option<f32>,
    /// Optional custom height in pixels
    pub height: Option<f32>,
}

pub struct ButtonOutput {
    pub clicked: bool,
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

        let size = props.size.unwrap_or(14.0);
        let text = egui::RichText::new(&props.label).size(size);

        let bg_color = match props.color {
            ButtonColor::Default => colors.surface1,
            ButtonColor::Danger => colors.error,
            ButtonColor::Success => colors.success,
        };

        let mut response = match props.button_type {
            ButtonType::Elevated => {
                let text_color = get_contrast_text_color(bg_color);
                Self::elevated_button(ui, text.color(text_color), bg_color, props.width, props.height)
            }
            ButtonType::Text => {
                // Text buttons use the color as the text color, not the background.
                Self::text_button(ui, text.color(bg_color), colors.surface1, props.width, props.height)
            }
        };

        if let Some(hover_text) = props.hover_text {
            response = response.on_hover_text(hover_text);
        }

        response = response.on_hover_cursor(egui::CursorIcon::PointingHand);

        ButtonOutput {
            clicked: response.clicked(),
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
            .fill(bg_color)
            .stroke(egui::Stroke::NONE)
            .corner_radius(4);

        if let (Some(w), Some(h)) = (width, height) {
            ui.add_sized(egui::vec2(w, h), button)
        } else if let Some(w) = width {
            ui.add_sized(egui::vec2(w, 0.0), button)
        } else if let Some(h) = height {
            ui.add_sized(egui::vec2(0.0, h), button)
        } else {
            ui.add(button)
        }
    }

    fn text_button(
        ui: &mut egui::Ui,
        text: egui::RichText,
        hover_bg_color: Color32,
        width: Option<f32>,
        height: Option<f32>,
    ) -> egui::Response {
        let button = egui::Button::new(text)
            .fill(Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE);

        let response = if let (Some(w), Some(h)) = (width, height) {
            ui.add_sized(egui::vec2(w, h), button)
        } else if let Some(w) = width {
            ui.add_sized(egui::vec2(w, 0.0), button)
        } else if let Some(h) = height {
            ui.add_sized(egui::vec2(0.0, h), button)
        } else {
            ui.add(button)
        };

        // Apply hover background effect
        if response.hovered() && ui.is_rect_visible(response.rect) {
            // Draw a subtle background on hover with reduced opacity
            let hover_color = Color32::from_rgba_premultiplied(
                hover_bg_color.r(),
                hover_bg_color.g(),
                hover_bg_color.b(),
                40, // Low alpha for subtle effect
            );
            ui.painter().rect_filled(response.rect, 4.0, hover_color);
        }

        response
    }
}
