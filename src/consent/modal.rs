use eframe::egui::{self, Align2, Color32, CornerRadius, Frame, Layout, Margin, Stroke};

use crate::{
    components::traits::ContextComponent,
    theme::{ThemeColors, phosphor_font_id},
};
use thoth_plugin_sdk::components::{
    Button, ButtonColor, ButtonSize, ButtonType, Typography, TypographyVariant,
};

use super::manager::{ConsentRequest, PermissionEntry};

// ── Props ─────────────────────────────────────────────────────────────────────

pub struct ConsentModalProps<'a> {
    /// The active consent request, or `None` when nothing is pending.
    pub request: Option<ConsentRequest>,
    /// Called when the user clicks Allow.
    /// The `bool` argument is `true` when "Remember this choice" is checked.
    pub on_accept: &'a dyn Fn(bool),
    /// Called when the user clicks Cancel.
    pub on_cancel: &'a dyn Fn(),
}

// ── Component ─────────────────────────────────────────────────────────────────

#[derive(Default)]
pub struct ConsentModal {
    remember: bool,
}

impl ContextComponent for ConsentModal {
    type Props<'a> = ConsentModalProps<'a>;
    type Output = ();

    fn render(&mut self, ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let Some(request) = props.request else {
            self.remember = false;
            return;
        };

        let colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| crate::theme::Theme::default().colors())
        });

        let mut accepted = false;
        let mut cancelled = false;

        let modal = egui::Modal::new(egui::Id::new("consent_modal"))
            .backdrop_color(Color32::from_black_alpha(153));

        modal.show(ui.ctx(), |ui| {
            ui.set_width(480.0);

            Frame::new()
                .fill(colors.bg)
                .stroke(Stroke::new(1.0, colors.surface))
                .corner_radius(CornerRadius::same(8))
                .inner_margin(Margin::ZERO)
                .show(ui, |ui| {
                    render_header(ui, &request, &colors);
                    ui.add(egui::Separator::default().spacing(0.0));
                    render_body(ui, &request, &colors);
                    ui.add(egui::Separator::default().spacing(0.0));
                    render_footer(
                        ui,
                        &mut self.remember,
                        &mut accepted,
                        &mut cancelled,
                        &colors,
                    );
                });
        });

        // Backdrop clicks are intentionally absorbed — the user must make an explicit choice.

        if accepted {
            (props.on_accept)(self.remember);
            self.remember = false;
        } else if cancelled {
            (props.on_cancel)();
            self.remember = false;
        }
    }
}

// ── Section renderers ─────────────────────────────────────────────────────────

fn render_header(ui: &mut egui::Ui, request: &ConsentRequest, colors: &ThemeColors) {
    Frame::new()
        .inner_margin(Margin {
            left: 24,
            right: 24,
            top: 20,
            bottom: 16,
        })
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Plugin avatar with shield-warning badge
                let avatar_size = egui::vec2(44.0, 44.0);
                let (avatar_rect, _) = ui.allocate_exact_size(avatar_size, egui::Sense::hover());
                ui.painter()
                    .rect_filled(avatar_rect, CornerRadius::same(8), colors.surface);
                ui.painter().text(
                    avatar_rect.center(),
                    Align2::CENTER_CENTER,
                    egui_phosphor::regular::PUZZLE_PIECE,
                    phosphor_font_id(22.0),
                    colors.accent,
                );

                let badge_rect = egui::Rect::from_min_size(
                    avatar_rect.max - egui::vec2(14.0, 14.0),
                    egui::vec2(18.0, 18.0),
                );
                ui.painter()
                    .rect_filled(badge_rect, CornerRadius::same(9), colors.warning);
                ui.painter().rect_stroke(
                    badge_rect,
                    CornerRadius::same(9),
                    Stroke::new(2.0, colors.bg),
                    egui::StrokeKind::Outside,
                );
                ui.painter().text(
                    badge_rect.center(),
                    Align2::CENTER_CENTER,
                    egui_phosphor::regular::SHIELD_WARNING,
                    phosphor_font_id(10.0),
                    colors.bg,
                );

                ui.add_space(12.0);
                ui.vertical(|ui| {
                    ui.add(
                        Typography::builder()
                            .text("PERMISSION REQUESTED")
                            .variant(TypographyVariant::GroupLabel)
                            .color(thoth_plugin_sdk::theme::color_to_hex(colors.warning))
                            .build(),
                    );
                    ui.add_space(2.0);
                    Typography::heading(ui, &request.title);
                });
            });
        });
}

fn render_body(ui: &mut egui::Ui, request: &ConsentRequest, colors: &ThemeColors) {
    Frame::new()
        .inner_margin(Margin {
            left: 24,
            right: 24,
            top: 16,
            bottom: 12,
        })
        .show(ui, |ui| {
            if !request.message.is_empty() {
                Typography::body_large(ui, &request.message);
                ui.add_space(12.0);
            }

            Typography::group_label(ui, "THIS WILL ALLOW THE PLUGIN TO");
            ui.add_space(6.0);

            Frame::new()
                .fill(colors.bg_sunken)
                .stroke(Stroke::new(1.0, colors.surface))
                .corner_radius(CornerRadius::same(4))
                .inner_margin(Margin::same(12))
                .show(ui, |ui| {
                    for (i, entry) in request.permissions.iter().enumerate() {
                        if i > 0 {
                            ui.add_space(8.0);
                        }
                        render_permission_row(ui, entry, colors);
                    }
                });
        });
}

fn render_footer(
    ui: &mut egui::Ui,
    remember: &mut bool,
    accepted: &mut bool,
    cancelled: &mut bool,
    colors: &ThemeColors,
) {
    Frame::new()
        .inner_margin(Margin {
            left: 24,
            right: 24,
            top: 12,
            bottom: 16,
        })
        .fill(colors.bg_panel)
        .show(ui, |ui| {
            ui.horizontal(|ui| {
                // Custom checkbox — distinct from ToggleSwitch shape
                let cb_size = egui::vec2(14.0, 14.0);
                let (cb_rect, cb_resp) = ui.allocate_exact_size(cb_size, egui::Sense::click());
                if cb_resp.clicked() {
                    *remember = !*remember;
                }
                cb_resp.on_hover_cursor(egui::CursorIcon::PointingHand);
                ui.painter().rect(
                    cb_rect,
                    CornerRadius::same(3),
                    if *remember {
                        colors.accent
                    } else {
                        Color32::TRANSPARENT
                    },
                    Stroke::new(
                        1.0,
                        if *remember {
                            colors.accent
                        } else {
                            colors.surface_active
                        },
                    ),
                    egui::StrokeKind::Outside,
                );
                if *remember {
                    ui.painter().text(
                        cb_rect.center(),
                        Align2::CENTER_CENTER,
                        egui_phosphor::regular::CHECK,
                        phosphor_font_id(10.0),
                        colors.bg,
                    );
                }
                ui.add_space(6.0);
                ui.add(
                    Typography::builder()
                        .text("Remember this choice")
                        .variant(TypographyVariant::Subtitle)
                        .build(),
                );

                ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.spacing_mut().item_spacing.x = 8.0;

                    if ui
                        .add(
                            Button::builder()
                                .label("Allow")
                                .button_type(ButtonType::Elevated)
                                .color(ButtonColor::Primary)
                                .button_size(ButtonSize::Medium)
                                .height(32.0)
                                .width(100.0)
                                .build(),
                        )
                        .clicked()
                    {
                        *accepted = true;
                    }

                    if ui
                        .add(
                            Button::builder()
                                .label("Cancel")
                                .button_type(ButtonType::Elevated)
                                .color(ButtonColor::Default)
                                .button_size(ButtonSize::Medium)
                                .height(32.0)
                                .width(100.0)
                                .build(),
                        )
                        .clicked()
                    {
                        *cancelled = true;
                    }
                });
            });
        });
}

fn render_permission_row(ui: &mut egui::Ui, entry: &PermissionEntry, colors: &ThemeColors) {
    ui.horizontal(|ui| {
        let icon_color = if entry.sensitive {
            colors.warning
        } else {
            colors.accent
        };
        ui.label(
            egui::RichText::new(entry.icon)
                .font(phosphor_font_id(16.0))
                .color(icon_color),
        );
        ui.add_space(8.0);
        Typography::body_large(ui, &entry.label);
        ui.add_space(6.0);
        ui.add(
            Typography::builder()
                .text(&entry.scope)
                .variant(TypographyVariant::Mono)
                .size(11.0)
                .color(thoth_plugin_sdk::theme::color_to_hex(colors.fg_muted))
                .build(),
        );
        if entry.sensitive {
            ui.with_layout(Layout::right_to_left(egui::Align::Center), |ui| {
                Frame::new()
                    .fill(Color32::from_rgba_premultiplied(249, 226, 175, 26))
                    .corner_radius(CornerRadius::same(3))
                    .inner_margin(Margin {
                        left: 5,
                        right: 5,
                        top: 2,
                        bottom: 2,
                    })
                    .show(ui, |ui| {
                        ui.add(
                            Typography::builder()
                                .text("SENSITIVE")
                                .variant(TypographyVariant::Label)
                                .bold(true)
                                .color(thoth_plugin_sdk::theme::color_to_hex(colors.warning))
                                .build(),
                        );
                    });
            });
        }
    });
}
