use egui::{CursorIcon, RichText};

use crate::theme::ThemeColors;

use super::Breadcrumbs;

impl Breadcrumbs {
    /// Render the breadcrumb trail and report navigation.
    ///
    /// The returned [`egui::InnerResponse::inner`] is `Some(path)` when the user
    /// clicked a segment this frame: `Some("")` for the always-present **Root**
    /// link, or the delimiter-joined raw trail (matching the input format of
    /// [`Breadcrumbs::path`], so it round-trips) up to and including the clicked
    /// segment. The last segment is the current location — rendered bold and
    /// non-clickable. `None` when nothing was clicked.
    pub fn show(self, ui: &mut egui::Ui) -> egui::InnerResponse<Option<String>> {
        let colors = ThemeColors::from_ctx(ui.ctx());
        let delim = self.separator.as_deref().unwrap_or(".");
        let mut selected: Option<String> = None;

        let inner = ui.horizontal(|ui| {
            ui.add_space(8.0);
            match self.path.as_deref() {
                None => {
                    ui.label(
                        RichText::new("No selection")
                            .size(12.0)
                            .color(colors.fg_muted),
                    );
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
                                RichText::new(&segment.display)
                                    .size(12.0)
                                    .color(colors.fg)
                                    .strong(),
                            );
                        } else {
                            // Navigation emits the RAW path so it round-trips
                            // with `Breadcrumbs::path`, not the bracketed display.
                            let path = segments[..=i]
                                .iter()
                                .map(|s| s.raw.as_str())
                                .collect::<Vec<_>>()
                                .join(delim);
                            let resp = ui
                                .link(RichText::new(&segment.display).size(12.0).color(colors.fg))
                                .on_hover_cursor(CursorIcon::PointingHand);
                            let resp =
                                crate::theme::hover_text(resp, format!("Navigate to {path}"));
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

    /// Split `path` on `delim` into segments. Each segment keeps its `raw` token
    /// (for navigation round-tripping) and a `display` form where numeric tokens
    /// are bracketed (e.g. `"0"` -> `"[0]"`); empty tokens are dropped.
    fn parse_path(path: &str, delim: &str) -> Vec<BreadcrumbSegment> {
        path.split(delim)
            .filter(|t| !t.is_empty())
            .map(|t| BreadcrumbSegment {
                raw: t.to_owned(),
                display: if t.bytes().all(|b| b.is_ascii_digit()) {
                    format!("[{t}]")
                } else {
                    t.to_owned()
                },
            })
            .collect()
    }
}

/// A parsed breadcrumb segment: the `raw` input token and its `display` form.
struct BreadcrumbSegment {
    raw: String,
    display: String,
}

impl egui::Widget for Breadcrumbs {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.show(ui).response
    }
}
