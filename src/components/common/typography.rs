use eframe::egui;
use egui::text::{LayoutJob, TextFormat};

use crate::{components::traits::StatelessComponent, theme::ThemeColors};

/// Visual scale — maps to a specific size + weight + default color token.
#[derive(Default)]
pub enum TypographyVariant {
    /// Sidebar panel section titles: 11 px · semi-bold · `sidebar_header`.
    /// Use for "SEARCH", "RECENT FILES", etc.
    PanelHeader,
    /// Settings group card labels: 11 px · semi-bold · `fg_muted`.
    /// Use for group headings above setting cards.
    GroupLabel,
    /// Dialog / window titles: 14 px · bold · `fg`.
    Title,
    /// Section headings and card titles: 16 px · bold · `fg`.
    Heading,
    /// Large body — setting row labels, toolbar labels: 13 px · `fg`.
    BodyLarge,
    /// Standard body copy: 12 px · `fg`.
    #[default]
    Body,
    /// Secondary / muted body: 12 px · `fg_muted`.
    BodyMuted,
    /// Subtitle under a heading: 13 px · `fg_muted`.
    Subtitle,
    /// Small metadata, hints, counts: 11 px · `fg_muted`.
    Caption,
    /// Tiny badge / inline tag text: 10 px · `fg_muted`.
    Label,
    /// Monospace code / path text: 12 px · monospace · `fg`.
    Mono,
}

pub struct TypographyProps<'a> {
    pub text: &'a str,
    pub variant: TypographyVariant,
    /// Override the default color for this variant.
    pub color: Option<egui::Color32>,
    /// Override the variant's default font size in points.
    pub size_override: Option<f32>,
    /// Apply bold weight on top of the variant's default weight.
    pub bold: bool,
    /// Apply italic style.
    pub italic: bool,
    /// Apply underline decoration.
    pub underline: bool,
}

impl<'a> Default for TypographyProps<'a> {
    fn default() -> Self {
        Self {
            text: "",
            variant: TypographyVariant::Body,
            color: None,
            size_override: None,
            bold: false,
            italic: false,
            underline: false,
        }
    }
}

pub struct Typography;

impl Typography {
    /// Returns `(size, default_color, variant_is_bold, is_mono)`.
    fn variant_style(
        variant: &TypographyVariant,
        colors: &ThemeColors,
    ) -> (f32, egui::Color32, bool, bool) {
        match variant {
            TypographyVariant::PanelHeader => (11.0, colors.sidebar_header, true, false),
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

    /// Renders text with a 0.5 px horizontal second pass, thickening vertical
    /// strokes to simulate CSS font-weight 700 without loading a separate font.
    fn faux_bold_label(
        ui: &mut egui::Ui,
        text: &str,
        font_id: egui::FontId,
        color: egui::Color32,
        italic: bool,
        underline: bool,
    ) -> egui::Response {
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
                    egui::Stroke::new(1.0, color)
                } else {
                    egui::Stroke::NONE
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

    // ── Convenience shorthands ────────────────────────────────────────────────

    pub fn panel_header(ui: &mut egui::Ui, text: &str) -> egui::Response {
        Self::render(
            ui,
            TypographyProps {
                text,
                variant: TypographyVariant::PanelHeader,
                ..Default::default()
            },
        )
    }

    pub fn group_label(ui: &mut egui::Ui, text: &str) -> egui::Response {
        Self::render(
            ui,
            TypographyProps {
                text,
                variant: TypographyVariant::GroupLabel,
                ..Default::default()
            },
        )
    }

    pub fn title(ui: &mut egui::Ui, text: &str) -> egui::Response {
        Self::render(
            ui,
            TypographyProps {
                text,
                variant: TypographyVariant::Title,
                ..Default::default()
            },
        )
    }

    pub fn heading(ui: &mut egui::Ui, text: &str) -> egui::Response {
        Self::render(
            ui,
            TypographyProps {
                text,
                variant: TypographyVariant::Heading,
                ..Default::default()
            },
        )
    }

    pub fn body_large(ui: &mut egui::Ui, text: &str) -> egui::Response {
        Self::render(
            ui,
            TypographyProps {
                text,
                variant: TypographyVariant::BodyLarge,
                ..Default::default()
            },
        )
    }

    pub fn body(ui: &mut egui::Ui, text: &str) -> egui::Response {
        Self::render(
            ui,
            TypographyProps {
                text,
                ..Default::default()
            },
        )
    }

    pub fn body_muted(ui: &mut egui::Ui, text: &str) -> egui::Response {
        Self::render(
            ui,
            TypographyProps {
                text,
                variant: TypographyVariant::BodyMuted,
                ..Default::default()
            },
        )
    }

    pub fn subtitle(ui: &mut egui::Ui, text: &str) -> egui::Response {
        Self::render(
            ui,
            TypographyProps {
                text,
                variant: TypographyVariant::Subtitle,
                ..Default::default()
            },
        )
    }

    pub fn caption(ui: &mut egui::Ui, text: &str) -> egui::Response {
        Self::render(
            ui,
            TypographyProps {
                text,
                variant: TypographyVariant::Caption,
                ..Default::default()
            },
        )
    }

    pub fn label(ui: &mut egui::Ui, text: &str) -> egui::Response {
        Self::render(
            ui,
            TypographyProps {
                text,
                variant: TypographyVariant::Label,
                ..Default::default()
            },
        )
    }

    pub fn mono(ui: &mut egui::Ui, text: &str) -> egui::Response {
        Self::render(
            ui,
            TypographyProps {
                text,
                variant: TypographyVariant::Mono,
                ..Default::default()
            },
        )
    }

    pub fn bold(ui: &mut egui::Ui, text: &str) -> egui::Response {
        Self::render(
            ui,
            TypographyProps {
                text,
                bold: true,
                ..Default::default()
            },
        )
    }
}

impl StatelessComponent for Typography {
    type Props<'a> = TypographyProps<'a>;
    type Output = egui::Response;

    fn render(ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
                .unwrap_or_else(|| crate::theme::Theme::default().colors())
        });

        let (variant_size, default_color, variant_bold, is_mono) =
            Self::variant_style(&props.variant, &colors);
        let size = props.size_override.unwrap_or(variant_size);
        let color = props.color.unwrap_or(default_color);
        let is_bold = variant_bold || props.bold;

        if is_bold {
            let font_id = if is_mono {
                egui::FontId::monospace(size)
            } else {
                egui::FontId::proportional(size)
            };
            Self::faux_bold_label(
                ui,
                props.text,
                font_id,
                color,
                props.italic,
                props.underline,
            )
        } else {
            let mut rt = if is_mono {
                egui::RichText::new(props.text).size(size).monospace()
            } else {
                egui::RichText::new(props.text).size(size)
            };
            if props.italic {
                rt = rt.italics();
            }
            if props.underline {
                rt = rt.underline();
            }
            ui.label(rt.color(color))
        }
    }
}
