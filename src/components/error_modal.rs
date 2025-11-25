use crate::components::traits::StatefulComponent;
use crate::error::{ErrorHandler, ErrorRecovery, RecoveryAction, ThothError};
use eframe::egui;

/// Props for the error modal
pub struct ErrorModalProps<'a> {
    pub error: &'a ThothError,
    pub open: bool,
}

/// Events emitted by the error modal
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
pub struct ErrorModal;

impl StatefulComponent for ErrorModal {
    type Props<'a> = ErrorModalProps<'a>;
    type Output = ErrorModalOutput;

    fn render(&mut self, ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();
        let mut recovery_action = None;

        if !props.open {
            return ErrorModalOutput {
                events,
                recovery_action,
            };
        }

        // Get user-friendly message and recovery suggestion
        let user_message = ErrorHandler::get_user_message(props.error);
        let recovery_suggestion = ErrorRecovery::get_recovery_suggestion(props.error);
        let action = ErrorRecovery::get_recovery_action(props.error);

        // Log the technical error
        ErrorHandler::log_error(props.error);

        // Create modal window
        egui::Window::new("Error")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.set_min_width(400.0);
                ui.set_max_width(600.0);

                // Error icon and message
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("⚠")
                            .size(32.0)
                            .color(egui::Color32::from_rgb(255, 100, 100)),
                    );
                    ui.add_space(12.0);
                    ui.vertical(|ui| {
                        ui.label(egui::RichText::new("An error occurred").strong().size(16.0));
                        ui.add_space(4.0);
                        ui.label(user_message);
                    });
                });

                ui.add_space(12.0);

                // Recovery suggestion if available
                if let Some(suggestion) = recovery_suggestion {
                    ui.add(egui::Separator::default());
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new("💡").size(18.0));
                        ui.label(
                            egui::RichText::new(suggestion)
                                .italics()
                                .color(ui.visuals().weak_text_color()),
                        );
                    });
                    ui.add_space(8.0);
                }

                ui.add(egui::Separator::default());
                ui.add_space(8.0);

                // Buttons based on recovery action
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // Close button (always available)
                        if ui.button("Close").clicked() {
                            events.push(ErrorModalEvent::Close);
                            recovery_action = Some(RecoveryAction::ClearError);
                        }

                        // Only show Retry button if error is recoverable
                        if ErrorHandler::is_recoverable(props.error) && ui.button("Retry").clicked()
                        {
                            events.push(ErrorModalEvent::Retry);
                            recovery_action = Some(RecoveryAction::Retry);
                        }

                        // Show Reset button for specific recovery actions
                        if matches!(action, RecoveryAction::Reset) && ui.button("Reset").clicked() {
                            events.push(ErrorModalEvent::Reset);
                            recovery_action = Some(RecoveryAction::Reset);
                        }
                    });
                });
            });

        ErrorModalOutput {
            events,
            recovery_action,
        }
    }
}
