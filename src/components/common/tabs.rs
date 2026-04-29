use eframe::egui;

use crate::components::button::{Button, ButtonColor, ButtonProps, ButtonType};
use crate::components::traits::StatelessComponent;
use crate::theme::ThemeColors;

pub struct TabItem<'a> {
    pub value: &'a str,
    pub label: &'a str,
}

pub struct TabProps<'a> {
    pub id: egui::Id,
    pub items: &'a [TabItem<'a>],
    pub active: &'a str,
}

pub struct TabOutput {
    /// The `value` of the tab the user clicked, if any.
    pub selected: Option<String>,
}

pub struct Tabs;

impl StatelessComponent for Tabs {
    type Props<'a> = TabProps<'a>;
    type Output = TabOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| crate::theme::Theme::default().colors())
        });

        let mut selected: Option<String> = None;

        egui::Frame::new()
            .fill(colors.mantle)
            .outer_margin(egui::Margin {
                left: 0,
                right: 0,
                top: 0,
                bottom: 10,
            })
            .inner_margin(egui::Margin {
                left: 8,
                right: 8,
                top: 0,
                bottom: 0,
            })
            .show(ui, |ui| {
                ui.set_width(ui.available_width());

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = 3.0;

                    for item in props.items {
                        let is_active = item.value == props.active;
                        let btn = Button::render(
                            ui,
                            ButtonProps {
                                label: item.label.to_string(),
                                button_type: ButtonType::Text,
                                color: if is_active {
                                    ButtonColor::Primary
                                } else {
                                    ButtonColor::Default
                                },
                                hover_text: None,
                                size: None,
                                width: None,
                                height: Some(40.0),
                                enabled: true,
                                ..Default::default()
                            },
                        );

                        let resp = btn.response;

                        // Draw active underline
                        if is_active {
                            let bar_rect = egui::Rect::from_min_max(
                                egui::pos2(resp.rect.left(), resp.rect.bottom() - 2.0),
                                egui::pos2(resp.rect.right(), resp.rect.bottom()),
                            );
                            ui.painter().rect_filled(bar_rect, 0.0, colors.primary);
                        }

                        if resp.clicked() && !is_active {
                            selected = Some(item.value.to_string());
                        }
                    }
                });
            });

        TabOutput { selected }
    }
}
