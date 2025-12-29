use crate::components::traits::ContextComponent;
use eframe::egui;

/// Props for the GoToPathDialog component
pub struct GoToPathDialogProps<'a> {
    /// Whether the panel should be open
    pub open: bool,
    /// Theme colors for styling
    pub theme_colors: &'a crate::theme::ThemeColors,
}

/// Events emitted by the GoToPathDialog
#[derive(Debug, Clone)]
pub enum GoToPathDialogEvent {
    /// User wants to navigate to a path
    NavigateToPath(String),
    /// User wants to close the panel
    Close,
}

/// Output from the GoToPathDialog
pub struct GoToPathDialogOutput {
    pub events: Vec<GoToPathDialogEvent>,
}

/// Go-to-path panel component (top banner style)
/// Allows users to type a JSON path and jump directly to it
#[derive(Default)]
pub struct GoToPathDialog {
    /// Current input text
    input: String,
    /// Whether to request focus on the text input
    request_focus: bool,
}

impl ContextComponent for GoToPathDialog {
    type Props<'a> = GoToPathDialogProps<'a>;
    type Output = GoToPathDialogOutput;

    fn render(&mut self, ctx: &egui::Context, props: Self::Props<'_>) -> Self::Output {
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

        // Render as a top banner/panel (like search bar style)
        egui::TopBottomPanel::top("go_to_path_panel")
            .frame(
                egui::Frame::none()
                    .fill(props.theme_colors.mantle)
                    .inner_margin(12.0),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    // Icon
                    ui.label(
                        egui::RichText::new(egui_phosphor::regular::CROSSHAIR)
                            .size(18.0)
                            .color(props.theme_colors.info),
                    );

                    ui.add_space(8.0);

                    // Text input
                    let text_edit = egui::TextEdit::singleline(&mut self.input)
                        .hint_text("Jump to path (e.g., 0.user.name)")
                        .font(egui::FontId::proportional(14.0))
                        .desired_width(ui.available_width() - 100.0);

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

                    ui.add_space(8.0);

                    // Help text
                    ui.label(
                        egui::RichText::new("Enter ↵")
                            .size(11.0)
                            .color(props.theme_colors.overlay1),
                    );
                });
            });

        GoToPathDialogOutput { events }
    }
}
