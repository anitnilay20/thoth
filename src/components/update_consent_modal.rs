use std::cell::Cell;

use eframe::egui;

use crate::components::traits::StatelessComponent;
use thoth_plugin_sdk::components::{
    Button, ButtonColor, ButtonSize, ButtonType, Modal, Typography, TypographyVariant,
};
use thoth_plugin_sdk::theme::{FONT_BODY, ThemeColors, color_to_hex};

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
        let colors = ThemeColors::from_ctx(ui.ctx());

        // Card chrome, head and footer strip all come from the base modal
        // (design `.card.w400`): the head shows the title over the version delta.
        let modal = Modal::builder()
            .id("update_consent_modal")
            .title("Update Available")
            .subtitle(format!(
                "v{} → v{}",
                props.current_version, props.latest_version
            ))
            .width(400.0)
            // Design leads the head with the update glyph.
            .glyph(egui_phosphor::regular::ARROW_CIRCLE_UP)
            .glyph_color("info")
            // The design's update card has no ✕ — the user answers it either way.
            .dismissible(false)
            .build();

        // The footer closure borrows locals, so plain cells suffice.
        let update_now = Cell::new(false);
        let remind_later = Cell::new(false);

        let closed = modal.show_with_footer(
            ui,
            // ── Body — one paragraph of 13px `subtext0` copy ──────────────────
            |ui| {
                ui.add(
                    Typography::builder()
                        .text("A new version of Thoth is ready to install. Update now for the latest features and improvements.")
                        .variant(TypographyVariant::Body)
                        .size(FONT_BODY)
                        .color(color_to_hex(colors.fg_subtle()))
                        .build(),
                );
            },
            // ── Footer — laid out right-to-left, so the primary goes first ────
            |ui| {
                if ui
                    .add(
                        Button::builder()
                            .label("Update Now")
                            .button_type(ButtonType::Elevated)
                            .color(ButtonColor::Primary)
                            .button_size(ButtonSize::Medium)
                            .build(),
                    )
                    .clicked()
                {
                    update_now.set(true);
                }

                if ui
                    .add(
                        Button::builder()
                            .label("Remind Later")
                            .button_type(ButtonType::Text)
                            .color(ButtonColor::Default)
                            .button_size(ButtonSize::Medium)
                            .build(),
                    )
                    .clicked()
                {
                    remind_later.set(true);
                }
            },
        );

        UpdateConsentModalOutput {
            update_now: update_now.get(),
            // Escape / backdrop / ✕ defer the update rather than dismissing it
            // silently, so the prompt comes back on the next check.
            remind_later: remind_later.get() || (closed && !update_now.get()),
        }
    }
}
