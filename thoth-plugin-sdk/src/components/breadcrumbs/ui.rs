use egui::{CursorIcon, RichText};

use crate::theme::ThemeColors;

use super::Breadcrumbs;

impl Breadcrumbs {
    /// Render the breadcrumb trail and report navigation.
    ///
    /// The returned [`egui::InnerResponse::inner`] is `Some(path)` when the user
    /// clicked a segment this frame: `Some("")` for the always-present **Root**
    /// link, or the delimiter-joined trail (in display form, with numeric indices
    /// bracketed) up to and including the clicked segment. The last segment is the
    /// current location — rendered bold and non-clickable. `None` when nothing
    /// was clicked.
    pub fn show(self, ui: &mut egui::Ui) -> egui::InnerResponse<Option<String>> {
        let colors = ThemeColors::from_ctx(ui.ctx());
        let delim = self.separator.as_deref().unwrap_or(".");
        let mut selected: Option<String> = None;

        let inner = ui.horizontal(|ui| {
            ui.add_space(8.0);
            match self.path.as_deref() {
                None => {
                    ui.label(RichText::new("No selection").size(12.0).color(colors.fg_muted));
                }
                Some("") => {
                    ui.label(RichText::new("Root").size(12.0).color(colors.fg));
                }
                Some(p) => {
                    let segments = Self::parse_path(p, delim);

                    // Root is always clickable.
                    if ui
                        .link(RichText::new("Root").size(12.0).color(colors.fg))
                        .on_hover_cursor(CursorIcon::PointingHand)
                        .clicked()
                    {
                        selected = Some(String::new());
                    }

                    let last = segments.len().saturating_sub(1);
                    for (i, segment) in segments.iter().enumerate() {
                        ui.label(
                            RichText::new(egui_phosphor::regular::CARET_RIGHT)
                                .size(10.0)
                                .color(colors.fg_muted),
                        );
                        if i == last {
                            ui.label(
                                RichText::new(segment).size(12.0).color(colors.fg).strong(),
                            );
                        } else {
                            let path = segments[..=i].join(delim);
                            let resp = ui
                                .link(RichText::new(segment).size(12.0).color(colors.fg))
                                .on_hover_cursor(CursorIcon::PointingHand)
                                .on_hover_text(format!("Navigate to {path}"));
                            if resp.clicked() {
                                selected = Some(path);
                            }
                        }
                    }
                    ui.add_space(8.0);
                }
            }
        });

        egui::InnerResponse::new(selected, inner.response)
    }

    /// Split `path` on `delim` into displayable segments. Numeric tokens are
    /// bracketed (e.g. `"0"` -> `"[0]"`); empty tokens are dropped.
    fn parse_path(path: &str, delim: &str) -> Vec<String> {
        path.split(delim)
            .filter(|t| !t.is_empty())
            .map(|t| {
                if t.bytes().all(|b| b.is_ascii_digit()) {
                    format!("[{t}]")
                } else {
                    t.to_owned()
                }
            })
            .collect()
    }
}

impl egui::Widget for Breadcrumbs {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.show(ui).response
    }
}
