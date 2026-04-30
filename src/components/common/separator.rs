use eframe::egui;

use crate::components::traits::StatelessComponent;

pub struct Separator;

pub struct SeparatorProps {
    margin_top: f32,
    margin_bot: f32,
}

impl Default for SeparatorProps {
    fn default() -> Self {
        Self {
            margin_top: 0.0,
            margin_bot: 0.0,
        }
    }
}

pub struct SeparatorOutput;

impl StatelessComponent for Separator {
    type Props<'a> = SeparatorProps;

    type Output = SeparatorOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        if props.margin_top > 0.0 {
            ui.add_space(props.margin_top);
        }

        ui.separator();

        if props.margin_bot > 0.0 {
            ui.add_space(props.margin_bot);
        }

        SeparatorOutput
    }
}

impl Separator {
    pub fn default(ui: &mut egui::Ui) -> SeparatorOutput {
        Self::render(ui, SeparatorProps::default())
    }

    pub fn with_margin(ui: &mut egui::Ui, margin: f32) -> SeparatorOutput {
        Self::render(
            ui,
            SeparatorProps {
                margin_top: margin,
                margin_bot: margin,
            },
        )
    }

    pub fn with_margins(ui: &mut egui::Ui, margin: (f32, f32)) -> SeparatorOutput {
        Self::render(
            ui,
            SeparatorProps {
                margin_top: margin.0,
                margin_bot: margin.1,
            },
        )
    }
}
