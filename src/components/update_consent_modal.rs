use eframe::egui::{self, Frame, Layout, Margin, RichText};

use crate::{
    components::{
        button::{Button, ButtonColor, ButtonProps, ButtonSize, ButtonType},
        traits::StatelessComponent,
        typography::{Typography, TypographyProps, TypographyVariant},
    },
    theme::{ThemeColors, phosphor_font_id},
};

pub struct UpdateConsentModal;

pub struct UpdateConsentModalProps<'a> {
    pub current_version: &'a str,
    pub latest_version: &'a str,
}

pub struct UpdateConsentModalOutput {
    pub update_now: bool,
    pub remind_later: bool,
}

impl StatelessComponent for UpdateConsentModal {
    type Props<'a> = UpdateConsentModalProps<'a>;
    type Output = UpdateConsentModalOutput;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| crate::theme::Theme::default().colors())
        });

        let mut output = UpdateConsentModalOutput {
            update_now: false,
            remind_later: false,
        };

        let modal = egui::Modal::new(egui::Id::new("update_consent_modal"));
        modal.show(ui.ctx(), |ui| {
            ui.set_width(400.0);

            // Backdrop color is handled by egui::Modal internally.

            // ── Header ────────────────────────────────────────────────────────
            Frame::new()
                .inner_margin(Margin {
                    left: 24,
                    right: 24,
                    top: 24,
                    bottom: 16,
                })
                .show(ui, |ui| {
                    ui.horizontal(|ui| {
                        ui.label(
                            RichText::new(egui_phosphor::regular::ARROW_CIRCLE_UP)
                                .font(phosphor_font_id(28.0))
                                .color(colors.info),
                        );
                        ui.add_space(10.0);
                        ui.vertical(|ui| {
                            Typography::render(
                                ui,
                                TypographyProps {
                                    text: "Update Available",
                                    variant: TypographyVariant::BodyLarge,
                                    bold: true,
                                    ..Default::default()
                                },
                            );
                            Typography::body_muted(
                                ui,
                                &format!(
                                    "v{} → v{}",
                                    props.current_version, props.latest_version
                                ),
                            );
                        });
                    });
                });

            ui.add(egui::Separator::default().spacing(0.0));

            // ── Body ──────────────────────────────────────────────────────────
            Frame::new()
                .inner_margin(Margin {
                    left: 24,
                    right: 24,
                    top: 16,
                    bottom: 16,
                })
                .show(ui, |ui| {
                    Typography::body(
                        ui,
                        "A new version of Thoth is ready to install. Update now for the latest features and improvements.",
                    );
                });

            ui.add(egui::Separator::default().spacing(0.0));

            // ── Footer ────────────────────────────────────────────────────────
            Frame::new()
                .fill(colors.bg_sunken)
                .inner_margin(Margin {
                    left: 24,
                    right: 24,
                    top: 12,
                    bottom: 12,
                })
                .show(ui, |ui| {
                    ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                        if Button::render(
                            ui,
                            ButtonProps {
                                label: "Update Now".to_string(),
                                button_type: ButtonType::Elevated,
                                color: ButtonColor::Primary,
                                button_size: ButtonSize::Medium,
                                ..Default::default()
                            },
                        )
                        .clicked
                        {
                            output.update_now = true;
                        }

                        ui.add_space(8.0);

                        if Button::render(
                            ui,
                            ButtonProps {
                                label: "Remind Later".to_string(),
                                button_type: ButtonType::Text,
                                color: ButtonColor::Default,
                                button_size: ButtonSize::Medium,
                                ..Default::default()
                            },
                        )
                        .clicked
                        {
                            output.remind_later = true;
                        }
                    });
                });
        });

        output
    }
}
