use eframe::egui;

use crate::components::button::{Button, ButtonColor, ButtonProps, ButtonSize, ButtonType};
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
            .fill(colors.bg_panel)
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
                ui.set_height(40.0);

                let frame_bottom = ui.max_rect().max.y;

                ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
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
                                button_size: ButtonSize::Medium,
                                ..Default::default()
                            },
                        );

                        let resp = btn.response;

                        // Draw active underline pinned to frame bottom
                        if is_active {
                            let bar_rect = egui::Rect::from_min_max(
                                egui::pos2(resp.rect.left(), frame_bottom - 2.0),
                                egui::pos2(resp.rect.right(), frame_bottom),
                            );
                            ui.painter().rect_filled(bar_rect, 0.0, colors.accent);
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
