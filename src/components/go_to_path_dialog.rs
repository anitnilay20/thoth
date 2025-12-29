use crate::components::traits::StatefulComponent;
use eframe::egui;

/// Props for the GoToPathDialog component
pub struct GoToPathDialogProps<'a> {
    /// Whether the dialog should be open
    pub open: bool,
    /// Theme colors for styling
    pub theme_colors: &'a crate::theme::ThemeColors,
}

/// Events emitted by the GoToPathDialog
#[derive(Debug, Clone)]
pub enum GoToPathDialogEvent {
    /// User wants to navigate to a path
    NavigateToPath(String),
    /// User wants to close the dialog
    Close,
}

/// Output from the GoToPathDialog
pub struct GoToPathDialogOutput {
    pub events: Vec<GoToPathDialogEvent>,
}

/// Go-to-path dialog component
/// Allows users to type a JSON path and jump directly to it
#[derive(Default)]
pub struct GoToPathDialog {
    /// Current input text
    input: String,
    /// Whether to request focus on the text input
    request_focus: bool,
}

impl StatefulComponent for GoToPathDialog {
    type Props<'a> = GoToPathDialogProps<'a>;
    type Output = GoToPathDialogOutput;

    fn render(&mut self, ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();

        if !props.open {
            // Reset state when closed
            self.input.clear();
            self.request_focus = false;
            return GoToPathDialogOutput { events };
        }

        // Request focus when just opened
        if props.open && !self.request_focus {
            self.request_focus = true;
        }

        // Render modal overlay
        let screen_rect = ui.ctx().screen_rect();
        let modal_width = 500.0;
        let modal_height = 120.0;

        let modal_rect = egui::Rect::from_center_size(
            screen_rect.center(),
            egui::vec2(modal_width, modal_height),
        );

        // Dark overlay background
        ui.painter()
            .rect_filled(screen_rect, 0.0, egui::Color32::from_black_alpha(180));

        // Modal window
        let mut modal_ui = ui.child_ui(modal_rect, egui::Layout::top_down(egui::Align::Min), None);

        egui::Frame::window(&modal_ui.style())
            .fill(props.theme_colors.base)
            .stroke(egui::Stroke::new(1.0, props.theme_colors.surface1))
            .rounding(8.0)
            .inner_margin(16.0)
            .show(&mut modal_ui, |ui| {
                // Title
                ui.label(
                    egui::RichText::new("Go to Path")
                        .size(16.0)
                        .color(props.theme_colors.text)
                        .strong(),
                );

                ui.add_space(12.0);

                // Text input
                let text_edit = egui::TextEdit::singleline(&mut self.input)
                    .hint_text("Enter JSON path (e.g., 0.user.name)")
                    .font(egui::FontId::proportional(14.0))
                    .desired_width(modal_width - 48.0);

                let response = ui.add(text_edit);

                // Auto-focus on open
                if self.request_focus {
                    response.request_focus();
                    self.request_focus = false;
                }

                // Handle Enter key
                if response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)) {
                    if !self.input.trim().is_empty() {
                        events.push(GoToPathDialogEvent::NavigateToPath(
                            self.input.trim().to_string(),
                        ));
                        self.input.clear();
                        events.push(GoToPathDialogEvent::Close);
                    }
                }

                // Handle Escape key
                if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                    self.input.clear();
                    events.push(GoToPathDialogEvent::Close);
                }

                ui.add_space(12.0);

                // Help text
                ui.label(
                    egui::RichText::new("Press Enter to navigate, Esc to cancel")
                        .size(12.0)
                        .color(props.theme_colors.overlay1),
                );
            });

        GoToPathDialogOutput { events }
    }
}
