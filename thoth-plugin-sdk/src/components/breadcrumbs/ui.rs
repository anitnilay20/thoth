use egui::{CursorIcon, RichText};

use crate::theme::{RADIUS_CHECK, ThemeColors, with_alpha};

use super::Breadcrumbs;

/// Row height — design `.crumbs{height:26px}`.
const ROW_HEIGHT: f32 = 26.0;
/// Gap between segments and separators — design `.crumbs{gap:5px}`.
const GAP: f32 = 5.0;
/// Horizontal padding of the trail — design `.crumbs{padding:0 8px}`.
const PADDING_X: i8 = 8;
/// Segment text size — design `.crumbs{font-size:12px}`.
const FONT_SIZE: f32 = 12.0;
/// Separator glyph size — design `.crumbs .sepc{font-size:10px}`.
const SEPARATOR_SIZE: f32 = 10.0;
/// Padding inside a segment's hit area — design `.crumbs a{padding:1px 5px}`.
const SEGMENT_PADDING: egui::Vec2 = egui::vec2(5.0, 1.0);
/// Hover wash behind a clickable segment — design `text 8%`.
const HOVER_ALPHA: u8 = 20;

impl Breadcrumbs {
    /// Paint one segment: `label` inside a small chip-shaped hit area. Clickable
    /// segments fill on hover (design `.crumbs a:hover`); the `current` one — the
    /// last — is bold and inert (design `.crumbs .cur`).
    fn segment(
        ui: &mut egui::Ui,
        label: &str,
        current: bool,
        colors: &ThemeColors,
    ) -> egui::Response {
        let galley = ui.painter().layout_no_wrap(
            label.to_owned(),
            egui::FontId::proportional(FONT_SIZE),
            colors.fg,
        );
        let size = galley.size() + SEGMENT_PADDING * 2.0;
        let sense = if current {
            egui::Sense::hover()
        } else {
            egui::Sense::click()
        };
        let (rect, response) = ui.allocate_exact_size(size, sense);

        if ui.is_rect_visible(rect) {
            if !current && response.hovered() {
                ui.painter()
                    .rect_filled(rect, RADIUS_CHECK, with_alpha(colors.fg, HOVER_ALPHA));
            }
            let pos = rect.center() - galley.rect.center().to_vec2();
            if current {
                // Faux bold (design `.cur{font-weight:700}`): a second pass shifted
                // half a pixel right thickens the vertical strokes.
                ui.painter()
                    .galley(pos + egui::vec2(0.5, 0.0), galley.clone(), colors.fg);
            }
            ui.painter().galley(pos, galley, colors.fg);
        }

        response
    }

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

        let inner = egui::Frame::new()
            .inner_margin(egui::Margin {
                left: PADDING_X,
                right: PADDING_X,
                top: 0,
                bottom: 0,
            })
            .show(ui, |ui| {
                ui.set_min_height(ROW_HEIGHT);
                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = GAP;
                    match self.path.as_deref() {
                        None => {
                            ui.label(
                                RichText::new("No selection")
                                    .size(FONT_SIZE)
                                    .color(colors.fg_muted),
                            );
                        }
                        Some("") => {
                            Self::segment(ui, "Root", true, &colors);
                        }
                        Some(p) => {
                            let segments = Self::parse_path(p, delim);

                            // Root is always clickable.
                            if Self::segment(ui, "Root", false, &colors)
                                .on_hover_cursor(CursorIcon::PointingHand)
                                .clicked()
                            {
                                selected = Some(String::new());
                            }

                            let last = segments.len().saturating_sub(1);
                            for (i, segment) in segments.iter().enumerate() {
                                ui.label(
                                    RichText::new(egui_phosphor::regular::CARET_RIGHT)
                                        .size(SEPARATOR_SIZE)
                                        .color(colors.fg_muted),
                                );
                                if i == last {
                                    Self::segment(ui, &segment.display, true, &colors);
                                } else {
                                    // Navigation emits the RAW path so it round-trips
                                    // with `Breadcrumbs::path`, not the bracketed display.
                                    let path = segments[..=i]
                                        .iter()
                                        .map(|s| s.raw.as_str())
                                        .collect::<Vec<_>>()
                                        .join(delim);
                                    let resp = Self::segment(ui, &segment.display, false, &colors)
                                        .on_hover_cursor(CursorIcon::PointingHand);
                                    let resp = crate::theme::hover_text(
                                        resp,
                                        format!("Navigate to {path}"),
                                    );
                                    if resp.clicked() {
                                        selected = Some(path);
                                    }
                                }
                            }
                        }
                    }
                });
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
