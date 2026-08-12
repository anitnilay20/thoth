use egui::{InnerResponse, RichText, Stroke, Widget};

use crate::components::Size;
use crate::theme::{
    FONT_CAPTION, ICON_CONTROL, RADIUS_CONTROL, ThemeColors, edge_stroke, focus_stroke,
    phosphor_font_id,
};

use super::Input;

/// Horizontal padding inside the field — design `.field{padding:0 10px}`.
const PAD_X: f32 = 10.0;
/// Gap between the text and the leading icon / trailing eye — design
/// `.field{gap:7px}`.
const GAP: f32 = 7.0;

impl Input {
    /// Render the input, mutating [`value`](Input::value) in place.
    ///
    /// The returned [`InnerResponse::inner`] is `true` when the text changed
    /// this frame; [`InnerResponse::response`] is the underlying `TextEdit`
    /// response (use it for focus / lost-focus checks).
    pub fn show(&mut self, ui: &mut egui::Ui) -> InnerResponse<bool> {
        let colors = ThemeColors::from_ctx(ui.ctx());
        if !self.label.is_empty() {
            let text = if self.required {
                format!("{} *", self.label)
            } else {
                self.label.clone()
            };
            ui.label(
                RichText::new(text)
                    .color(colors.fg_muted)
                    .size(FONT_CAPTION),
            );
        }

        // Text-entry metrics — 12.5px text in a 28px box at Medium (design `.field`).
        let (font_size, height) = self.size.field_metrics();
        let font_id = if self.mono {
            egui::FontId::monospace(font_size)
        } else {
            egui::FontId::proportional(font_size)
        };

        // An errored field swaps the hairline for a red one and never draws the
        // focus ring (the design's `.err` never also gains `.foc`).
        let is_error = self.error.is_some();
        let stroke = if is_error {
            Stroke::new(1.0, colors.error)
        } else {
            edge_stroke(&colors)
        };

        // Whether a password field is currently revealed is view-only state, so
        // it lives in egui memory rather than on the serialisable struct. An
        // id-less field falls back to this `ui`'s auto id (as `CodeEditor` does),
        // so two anonymous fields in one container don't share reveal state.
        let id_source: String = if self.id.is_empty() {
            format!("sdk_input_{:?}", ui.next_auto_id())
        } else {
            self.id.clone()
        };
        let reveal_id = ui.make_persistent_id((id_source.as_str(), "reveal"));
        let mut reveal: bool = ui.ctx().data(|d| d.get_temp(reveal_id).unwrap_or(false));

        // `grow` (and the default) fill the available width; `desired_width`
        // pins it.
        let outer_w = if self.grow {
            ui.available_width()
        } else {
            self.desired_width.unwrap_or_else(|| ui.available_width())
        };

        let mut changed = false;
        let mut inner_response: Option<egui::Response> = None;
        // Rect of the field box itself, used for the focus ring below.
        let mut field_rect = egui::Rect::NOTHING;

        ui.add_enabled_ui(!self.disabled, |ui| {
            if self.multiline {
                // No design spec for text areas; they reuse the field chrome with
                // a vertical padding scaled to the size preset.
                let pad_y: i8 = match self.size {
                    Size::Small => 3,
                    Size::Medium => 4,
                    Size::Large => 6,
                };
                let frame = egui::Frame::new()
                    .fill(colors.surface)
                    .stroke(stroke)
                    .corner_radius(RADIUS_CONTROL)
                    .inner_margin(egui::Margin::symmetric(PAD_X as i8, pad_y))
                    .show(ui, |ui| {
                        ui.visuals_mut().weak_text_color = Some(colors.fg_muted);
                        let row_count = self.rows.max(1) as f32;
                        let row_height = ui.text_style_height(&egui::TextStyle::Body);
                        let fixed_h = row_height * row_count
                            + ui.spacing().item_spacing.y * (row_count - 1.0)
                            + ui.spacing().button_padding.y * 2.0;
                        let scroll_out = egui::ScrollArea::vertical()
                            .id_salt(ui.next_auto_id())
                            .max_height(fixed_h)
                            .min_scrolled_height(fixed_h)
                            .show(ui, |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut self.value)
                                        .hint_text(&self.placeholder)
                                        .desired_rows(self.rows.max(1))
                                        .desired_width(outer_w - 2.0 * PAD_X)
                                        .font(font_id.clone())
                                        .text_color(colors.fg)
                                        .margin(egui::Margin::ZERO)
                                        .frame(egui::Frame::NONE),
                                )
                            });
                        changed = scroll_out.inner.changed();
                        inner_response = Some(scroll_out.inner);
                    });
                field_rect = frame.response.rect;
            } else {
                // The box is painted by hand so it is exactly `height` tall and so
                // its click area (allocated *before* the text edit, hence below it)
                // focuses the field when the padding is clicked.
                let (rect, bg_resp) =
                    ui.allocate_exact_size(egui::vec2(outer_w, height), egui::Sense::click());
                field_rect = rect;
                if ui.is_rect_visible(rect) {
                    ui.painter()
                        .rect_filled(rect, RADIUS_CONTROL, colors.surface);
                    ui.painter().rect_stroke(
                        rect,
                        RADIUS_CONTROL,
                        stroke,
                        egui::StrokeKind::Inside,
                    );
                }

                let content = egui::UiBuilder::new()
                    .max_rect(rect.shrink2(egui::vec2(PAD_X, 0.0)))
                    .layout(egui::Layout::left_to_right(egui::Align::Center));
                ui.scope_builder(content, |ui| {
                    // Gaps are placed explicitly to hit the design's 7px.
                    ui.spacing_mut().item_spacing.x = 0.0;
                    ui.visuals_mut().weak_text_color = Some(colors.fg_muted);

                    if let Some(glyph) = &self.icon {
                        ui.label(
                            RichText::new(glyph)
                                .font(phosphor_font_id(ICON_CONTROL))
                                .color(colors.fg_muted),
                        );
                        ui.add_space(GAP);
                    }

                    // Reserve the reveal eye up front so the text never runs under it.
                    let trailing = if self.password {
                        ICON_CONTROL + GAP
                    } else {
                        0.0
                    };
                    let text_w = (ui.available_width() - trailing).max(0.0);
                    let mut edit = egui::TextEdit::singleline(&mut self.value)
                        .hint_text(&self.placeholder)
                        .desired_width(text_w)
                        .font(font_id.clone())
                        .text_color(colors.fg)
                        .vertical_align(egui::Align::Center)
                        .margin(egui::Margin::ZERO)
                        .frame(egui::Frame::NONE);
                    if self.password && !reveal {
                        edit = edit.password(true);
                    }
                    let r = ui.add(edit);
                    changed = r.changed();

                    if self.password {
                        ui.add_space(GAP);
                        let glyph = if reveal {
                            egui_phosphor::regular::EYE_SLASH
                        } else {
                            egui_phosphor::regular::EYE
                        };
                        let eye = ui.add(
                            egui::Label::new(
                                RichText::new(glyph)
                                    .font(phosphor_font_id(ICON_CONTROL))
                                    .color(colors.fg_muted),
                            )
                            .sense(egui::Sense::click()),
                        );
                        if eye.clicked() {
                            reveal = !reveal;
                            ui.ctx().data_mut(|d| d.insert_temp(reveal_id, reveal));
                        }
                        if eye.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }
                    }

                    // Clicking the padding around the text behaves like clicking
                    // the text itself.
                    if bg_resp.clicked() {
                        ui.memory_mut(|m| m.request_focus(r.id));
                    }
                    if bg_resp.hovered() {
                        ui.ctx().set_cursor_icon(egui::CursorIcon::Text);
                    }
                    inner_response = Some(r);
                });
            }
        });

        // Focus ring sits *outside* the hairline — design
        // `.field.foc{box-shadow:var(--edge),var(--focus)}`.
        let focused = inner_response.as_ref().is_some_and(|r| r.has_focus());
        if focused && !is_error && ui.is_rect_visible(field_rect) {
            ui.painter().rect_stroke(
                field_rect,
                RADIUS_CONTROL,
                focus_stroke(&colors),
                egui::StrokeKind::Outside,
            );
        }

        if let Some(message) = &self.error {
            ui.label(
                RichText::new(message)
                    .color(colors.error)
                    .size(FONT_CAPTION),
            );
        }

        let response = inner_response.unwrap_or_else(|| {
            ui.interact(
                field_rect,
                ui.make_persistent_id((id_source.as_str(), "field")),
                egui::Sense::hover(),
            )
        });
        InnerResponse::new(changed, response)
    }
}

impl Widget for Input {
    /// Convenience for `ui.add(input)` — renders read-only-style and **discards**
    /// edits and the changed flag. Use [`Input::show`] to capture input.
    fn ui(mut self, ui: &mut egui::Ui) -> egui::Response {
        self.show(ui).response
    }
}
