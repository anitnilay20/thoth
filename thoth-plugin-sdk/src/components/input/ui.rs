use egui::{InnerResponse, RichText, Stroke, Widget};

use crate::components::Size;
use crate::theme::{ThemeColors, phosphor_font_id};

use super::Input;

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
            ui.label(RichText::new(text).color(colors.fg_muted).size(11.0));
        }
        let width = if self.grow {
            f32::INFINITY
        } else {
            self.desired_width.unwrap_or(f32::INFINITY)
        };

        // Medium keeps the default look (no explicit font); Small/Large adjust the
        // font and vertical padding so the field matches small/large controls.
        let (font_size, pad_y): (Option<f32>, i8) = match self.size {
            Size::Small => (Some(11.0), 3),
            Size::Medium => (None, 4),
            Size::Large => (Some(14.0), 6),
        };

        let mut changed = false;
        let mut inner_response: Option<egui::Response> = None;

        let frame = egui::Frame::new()
            .fill(colors.surface)
            .stroke(Stroke::new(1.0, colors.surface_raised))
            .corner_radius(4)
            .inner_margin(egui::Margin::symmetric(8, pad_y))
            .show(ui, |ui| {
                ui.add_enabled_ui(!self.disabled, |ui| {
                    if self.multiline {
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
                                        .desired_width(width)
                                        .frame(egui::Frame::NONE),
                                )
                            });
                        changed = scroll_out.inner.changed();
                        inner_response = Some(scroll_out.inner);
                    } else {
                        ui.horizontal(|ui| {
                            if let Some(glyph) = &self.icon {
                                ui.label(
                                    RichText::new(glyph)
                                        .font(phosphor_font_id(13.0))
                                        .color(colors.fg_muted),
                                );
                                ui.add_space(4.0);
                            }
                            let mut edit = egui::TextEdit::singleline(&mut self.value)
                                .hint_text(&self.placeholder)
                                .desired_width(width)
                                .vertical_align(egui::Align::Center)
                                .frame(egui::Frame::NONE);
                            if let Some(fs) = font_size {
                                edit = edit.font(egui::FontId::proportional(fs));
                            }
                            if self.password {
                                edit = edit.password(true);
                            }
                            let r = ui.add(edit);
                            changed = r.changed();
                            inner_response = Some(r);
                        });
                    }
                });
            });

        let response = inner_response.unwrap_or(frame.response);
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
