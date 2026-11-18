use std::cell::Cell;

use crate::components::traits::StatefulComponent;
use crate::error::{ErrorHandler, ErrorRecovery, RecoveryAction, ThothError};
use eframe::egui;
use thoth_plugin_sdk::components::{
    Button, ButtonColor, ButtonType, Icon, Modal, Separator, Typography, TypographyVariant,
};
use thoth_plugin_sdk::theme::{FONT_BODY, ThemeColors, color_to_hex, with_alpha};

/// Props for the error modal
pub struct ErrorModalProps<'a> {
    pub error: &'a ThothError,
    pub open: bool,
}

/// Events emitted by the error modal
#[derive(Clone, Copy)]
pub enum ErrorModalEvent {
    Close,
    Retry,
    Reset,
}

pub struct ErrorModalOutput {
    pub events: Vec<ErrorModalEvent>,
    pub recovery_action: Option<RecoveryAction>,
}

/// Error modal component - displays errors with recovery options
#[derive(Default)]
pub struct ErrorModal {
    /// The error already written to the log, so a modal that stays open for
    /// hundreds of frames logs once instead of once per frame.
    logged: Option<String>,
}

impl StatefulComponent for ErrorModal {
    type Props<'a> = ErrorModalProps<'a>;
    type Output = ErrorModalOutput;

    fn render(&mut self, ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();
        let mut recovery_action = None;

        if !props.open {
            self.logged = None;
            return ErrorModalOutput {
                events,
                recovery_action,
            };
        }

        let colors = ThemeColors::from_ctx(ui.ctx());

        // Get user-friendly message and recovery suggestion
        let user_message = ErrorHandler::get_user_message(props.error);
        let recovery_suggestion = ErrorRecovery::get_recovery_suggestion(props.error);
        let action = ErrorRecovery::get_recovery_action(props.error);
        let recoverable = ErrorHandler::is_recoverable(props.error);
        let resettable = matches!(action, RecoveryAction::Reset);

        // Log the technical error once per error instance, not once per frame.
        let signature = props.error.to_string();
        if self.logged.as_deref() != Some(signature.as_str()) {
            ErrorHandler::log_error(props.error);
            self.logged = Some(signature);
        }

        // The card's head carries the title plus the message as its subtitle
        // (design `.m-head`: `An error occurred` over a `pre-line` detail line).
        let modal = Modal::builder()
            .id("error_modal")
            .title("An error occurred")
            .subtitle(user_message)
            .width(480.0)
            // Design leads the head with the 30px status glyph (`.glyph`).
            .glyph(egui_phosphor::regular::WARNING_CIRCLE)
            .glyph_color("error")
            .build();

        // The footer closure may borrow locals, so the pick is a plain `Cell`.
        let picked: Cell<Option<ErrorModalEvent>> = Cell::new(None);

        let closed = modal.show_with_footer(
            ui,
            |ui| {
                // Recovery hint — design `.sep` + a `.m-body` row of the yellow
                // lightbulb and the italic suggestion.
                if let Some(suggestion) = recovery_suggestion {
                    ui.spacing_mut().item_spacing.y = 0.0;
                    // `.sep`: hairline inset 20px each side — the body padding
                    // already insets us, so it spans the available width.
                    ui.add(Separator::rule(color_to_hex(with_alpha(
                        colors.surface_raised,
                        77, // surface1@30%
                    ))));
                    ui.add_space(13.0); // `.m-body{padding-top:13px}`
                    ui.horizontal_top(|ui| {
                        ui.spacing_mut().item_spacing.x = 9.0;
                        ui.add(
                            Icon::builder()
                                .glyph(egui_phosphor::regular::LIGHTBULB)
                                .color("warning")
                                .size(17.0)
                                .build(),
                        );
                        // Labels don't wrap inside a horizontal layout, so the
                        // copy gets its own strip across the remaining width.
                        ui.vertical(|ui| {
                            ui.add(
                                Typography::builder()
                                    .text(suggestion)
                                    .variant(TypographyVariant::Body)
                                    .size(FONT_BODY)
                                    .italic(true)
                                    .color("fg-muted")
                                    .build(),
                            );
                        });
                    });
                }
            },
            // `.m-foot` lays out right-to-left: the recovery actions sit at the
            // right edge, `Close` to their left.
            |ui| {
                // Only offer Retry if the error is recoverable
                if recoverable
                    && ui
                        .add(
                            Button::builder()
                                .label("Retry")
                                .button_type(ButtonType::Elevated)
                                .color(ButtonColor::Danger)
                                .build(),
                        )
                        .clicked()
                {
                    picked.set(Some(ErrorModalEvent::Retry));
                }

                // Show Reset button for specific recovery actions
                if resettable
                    && ui
                        .add(
                            Button::builder()
                                .label("Reset")
                                .button_type(ButtonType::Elevated)
                                .color(ButtonColor::Danger)
                                .build(),
                        )
                        .clicked()
                {
                    picked.set(Some(ErrorModalEvent::Reset));
                }

                // Close button (always available)
                if ui
                    .add(
                        Button::builder()
                            .label("Close")
                            .button_type(ButtonType::Elevated)
                            .color(ButtonColor::Default)
                            .build(),
                    )
                    .clicked()
                {
                    picked.set(Some(ErrorModalEvent::Close));
                }
            },
        );

        // Escape / backdrop / ✕ dismiss the card the same way `Close` does.
        let event = picked.get().or(if closed {
            Some(ErrorModalEvent::Close)
        } else {
            None
        });

        if let Some(event) = event {
            recovery_action = Some(match event {
                ErrorModalEvent::Close => RecoveryAction::ClearError,
                ErrorModalEvent::Retry => RecoveryAction::Retry,
                ErrorModalEvent::Reset => RecoveryAction::Reset,
            });
            events.push(event);
        }

        ErrorModalOutput {
            events,
            recovery_action,
        }
    }
}
