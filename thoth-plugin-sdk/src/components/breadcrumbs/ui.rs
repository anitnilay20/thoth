use egui::Widget;

use crate::{
    components::{Button, ButtonSize, ButtonType},
    theme::ThemeColors,
};

use super::Breadcrumbs;

/// One parsed breadcrumb segment.
struct Segment<'a> {
    /// The original token from the input path, used to rebuild a canonical
    /// navigation path (e.g. `"42"`).
    raw: &'a str,
    /// How the segment is shown to the user — numeric indices are bracketed
    /// (e.g. `"[42]"`), everything else matches `raw`.
    display: String,
}

impl Breadcrumbs {
    /// Render the breadcrumb trail and report navigation.
    ///
    /// The returned [`egui::InnerResponse::inner`] is `Some(path)` when the user
    /// clicked a segment this frame, where `path` is the separator-joined trail
    /// of the original (raw) tokens up to and including that segment — i.e. it
    /// matches the input syntax, not the bracketed display form. Clicking
    /// `settings` in `users.42.settings` yields `"users.42.settings"`. It is
    /// `None` when nothing was clicked.
    pub fn show(self, ui: &mut egui::Ui) -> egui::InnerResponse<Option<String>> {
        let colors = ThemeColors::from_ctx(ui.ctx());
        let segments = self.parse_path();
        let separator = self.separator.as_deref().unwrap_or(".");

        let mut selected: Option<String> = None;

        let inner = ui.horizontal(|ui| {
            // Tighten the gap between segment buttons and separators.
            ui.spacing_mut().item_spacing.x = 2.0;

            for (i, segment) in segments.iter().enumerate() {
                if i > 0 {
                    ui.colored_label(colors.fg_muted, separator);
                }

                let hover = format!("Navigate to {}", segment.display);
                let response = Button::builder()
                    .label(segment.display.as_str())
                    .button_type(ButtonType::Text)
                    .button_size(ButtonSize::Small)
                    .hover_text(hover.as_str())
                    .build()
                    .ui(ui);

                if response.clicked() {
                    selected = Some(
                        segments[..=i]
                            .iter()
                            .map(|s| s.raw)
                            .collect::<Vec<_>>()
                            .join(separator),
                    );
                }
            }
        });

        egui::InnerResponse::new(selected, inner.response)
    }
}

impl Widget for Breadcrumbs {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.show(ui).response
    }
}

impl Breadcrumbs {
    /// Split the path into segments, pairing each raw token with its display
    /// form. Numeric tokens are bracketed for display while their raw value is
    /// preserved for navigation.
    ///
    /// Examples (raw -> display):
    /// - `"0.user.name"`    -> `["0" → "[0]", "user", "name"]`
    /// - `"users.42.title"` -> `["users", "42" → "[42]", "title"]`
    fn parse_path(&self) -> Vec<Segment<'_>> {
        let Some(path) = self.path.as_deref() else {
            return vec![];
        };

        path.split('.')
            .filter(|token| !token.is_empty())
            .map(|raw| {
                let display = if !raw.is_empty() && raw.bytes().all(|b| b.is_ascii_digit()) {
                    format!("[{raw}]")
                } else {
                    raw.to_owned()
                };
                Segment { raw, display }
            })
            .collect()
    }
}
