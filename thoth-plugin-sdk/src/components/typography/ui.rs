use egui::text::{LayoutJob, TextFormat};
use egui::{Color32, FontId, Response, RichText, Stroke, Widget};

use crate::theme::{ThemeColors, resolve_color};

use super::{Typography, TypographyVariant};

impl TypographyVariant {
    /// Returns `(size, default_color, variant_is_bold, is_mono)` for this
    /// variant against the active palette.
    fn style(self, colors: &ThemeColors) -> (f32, Color32, bool, bool) {
        match self {
            TypographyVariant::PanelHeader => (11.0, colors.sidebar_header, true, false),
            TypographyVariant::SectionHeader => (12.0, colors.fg, true, false),
            TypographyVariant::GroupLabel => (11.0, colors.fg_muted, true, false),
            TypographyVariant::Title => (14.0, colors.fg, true, false),
            TypographyVariant::Heading => (16.0, colors.fg, true, false),
            TypographyVariant::BodyLarge => (13.0, colors.fg, false, false),
            TypographyVariant::Body => (12.0, colors.fg, false, false),
            TypographyVariant::BodyMuted => (12.0, colors.fg_muted, false, false),
            TypographyVariant::Subtitle => (13.0, colors.fg_muted, false, false),
            TypographyVariant::Caption => (11.0, colors.fg_muted, false, false),
            TypographyVariant::Label => (10.0, colors.fg_muted, false, false),
            TypographyVariant::Mono => (12.0, colors.fg, false, true),
        }
    }
}

impl Typography {
    /// Renders text with a 0.5px horizontal second pass, thickening vertical
    /// strokes to simulate font-weight 700 without loading a separate font.
    fn faux_bold_label(
        ui: &mut egui::Ui,
        text: &str,
        font_id: FontId,
        color: Color32,
        italic: bool,
        underline: bool,
    ) -> Response {
        let mut job = LayoutJob::default();
        job.wrap.max_width = ui.available_width();
        job.append(
            text,
            0.0,
            TextFormat {
                font_id,
                color,
                italics: italic,
                underline: if underline {
                    Stroke::new(1.0, color)
                } else {
                    Stroke::NONE
                },
                ..Default::default()
            },
        );
        let galley = ui.painter().layout_job(job);
        let (rect, response) = ui.allocate_exact_size(galley.size(), egui::Sense::hover());
        if ui.is_rect_visible(rect) {
            ui.painter()
                .galley(rect.min + egui::vec2(0.5, 0.0), galley.clone(), color);
            ui.painter().galley(rect.min, galley, color);
        }
        response
    }
}

impl Widget for Typography {
    fn ui(self, ui: &mut egui::Ui) -> Response {
        let colors = ThemeColors::from_ctx(ui.ctx());
        let (variant_size, default_color, variant_bold, is_mono) = self.variant.style(&colors);

        let size = self.size.unwrap_or(variant_size);
        let color = self
            .color
            .as_deref()
            .and_then(|c| resolve_color(c, &colors))
            .unwrap_or(default_color);
        let is_bold = variant_bold || self.bold;

        if is_bold {
            let font_id = if is_mono {
                FontId::monospace(size)
            } else {
                FontId::proportional(size)
            };
            Self::faux_bold_label(ui, &self.text, font_id, color, self.italic, self.underline)
        } else {
            let mut rt = if is_mono {
                RichText::new(&self.text).size(size).monospace()
            } else {
                RichText::new(&self.text).size(size)
            };
            if self.italic {
                rt = rt.italics();
            }
            if self.underline {
                rt = rt.underline();
            }
            ui.label(rt.color(color))
        }
    }
}
