use super::{Button, ButtonColor, ButtonSize, ButtonType};
use crate::theme::{
    RADIUS_CONTROL, ThemeColors, edge_stroke, get_contrast_text_color, glow_shadow,
    phosphor_font_id, with_alpha,
};
use egui::{Color32, Shadow, Stroke, TextFormat, text::LayoutJob};

/// Gap between a leading icon and the label — design `.btn{gap:6px}`.
const ICON_GAP: f32 = 6.0;
/// Horizontal padding inside a button — design `.btn{padding:0 11px}`.
const PADDING_X: f32 = 11.0;
/// …and its compact variant — design `.btn.sm{padding:0 9px}`.
const PADDING_X_SMALL: f32 = 9.0;
/// Solid fills brighten on hover — design `.btn.primary:hover{filter:brightness(1.06)}`.
const HOVER_BRIGHTEN: f32 = 1.06;
/// …and dim by the same step while held.
const PRESSED_DIM: f32 = 0.94;
/// Disabled buttons drop to 42% opacity — design `.btn.disabled{opacity:.42}`.
const DISABLED_OPACITY: f32 = 0.42;
/// A soft fill lays 18% of its semantic colour over the resting surface — design
/// `.btn.dsoft{background:color-mix(in oklab,var(--red) 18%,var(--surface0))}`.
const SOFT_TINT_ALPHA: u8 = 46; // 18% of 255

/// Background fill of a filled button in each interaction state.
struct Fill {
    resting: Color32,
    hovered: Color32,
    pressed: Color32,
}

impl Fill {
    /// The neutral surface ladder — design `.btn.secondary` and its `:hover`.
    fn surface(colors: &ThemeColors) -> Self {
        Self {
            resting: colors.surface,
            hovered: colors.surface_raised,
            pressed: colors.surface_active,
        }
    }

    /// A solid semantic fill, brightened on hover and dimmed while pressed.
    fn solid(color: Color32) -> Self {
        Self {
            resting: color,
            hovered: color.gamma_multiply(HOVER_BRIGHTEN),
            pressed: color.gamma_multiply(PRESSED_DIM),
        }
    }
}

/// Everything needed to paint one filled button, resolved from the palette.
struct Visual {
    fill: Fill,
    /// Translucent wash laid over `fill` — soft variants only.
    tint: Option<Color32>,
    text: Color32,
    stroke: Stroke,
    shadow: Option<Shadow>,
}

impl Visual {
    /// Paint recipe for `color`, either solid or as a soft tint over the surface.
    fn resolve(color: ButtonColor, soft: bool, colors: &ThemeColors) -> Self {
        // `Default` has no semantic hue: it *is* the neutral surface button.
        let semantic = match color {
            ButtonColor::Default => None,
            // The design's primary button is lavender, not mauve; `Secondary`
            // shares it so text buttons keep their lavender label colour.
            ButtonColor::Primary | ButtonColor::Secondary => Some(colors.accent_secondary),
            ButtonColor::Danger => Some(colors.error),
            ButtonColor::Success => Some(colors.success),
            ButtonColor::Warning => Some(colors.warning),
        };

        match (semantic, soft) {
            // `.btn.dsoft` — the hue reads in the label, the fill only hints at it.
            (Some(hue), true) => Self {
                fill: Fill::surface(colors),
                tint: Some(with_alpha(hue, SOFT_TINT_ALPHA)),
                text: hue,
                stroke: Stroke::NONE,
                shadow: None,
            },
            // Solid semantic fill with a matching glow — `.btn.primary` et al.
            (Some(hue), false) => Self {
                fill: Fill::solid(hue),
                tint: None,
                text: get_contrast_text_color(hue),
                stroke: Stroke::NONE,
                shadow: Some(glow_shadow(hue)),
            },
            // `.btn.secondary` — surface fill, hairline edge, no glow. A soft
            // neutral button is the same thing minus the edge.
            (None, soft) => Self {
                fill: Fill::surface(colors),
                tint: None,
                text: colors.fg,
                stroke: if soft {
                    Stroke::NONE
                } else {
                    edge_stroke(colors)
                },
                shadow: None,
            },
        }
    }
}

impl Button {
    fn make_layout_job(icon: Option<&str>, label: &str, size: f32, color: Color32) -> LayoutJob {
        let mut job = LayoutJob::default();
        let mut gap = 0.0;
        if let Some(ic) = icon {
            job.append(
                ic,
                0.0,
                TextFormat {
                    font_id: phosphor_font_id(size),
                    color,
                    valign: egui::Align::Center,
                    ..Default::default()
                },
            );
            // Design `.btn{gap:6px}` — a real gap, not a space glyph.
            gap = ICON_GAP;
        }
        job.append(
            label,
            gap,
            TextFormat {
                font_id: egui::FontId::proportional(size),
                color,
                valign: egui::Align::Center,
                ..Default::default()
            },
        );
        job
    }

    /// Paint a filled button: optional glow, fill (+ tint), inset edge, label.
    ///
    /// `job` must be laid out with [`Color32::PLACEHOLDER`] so the final text
    /// colour — which depends on the enabled state — can be applied at paint time.
    fn filled_button(
        ui: &mut egui::Ui,
        job: LayoutJob,
        visual: Visual,
        padding_x: f32,
        width: Option<f32>,
        height: Option<f32>,
    ) -> egui::Response {
        let galley = ui.painter().layout_job(job);
        let desired = egui::vec2(
            width.unwrap_or(galley.size().x + padding_x * 2.0),
            height.unwrap_or(galley.size().y + 10.0),
        );
        let (rect, response) = ui.allocate_exact_size(desired, egui::Sense::click());

        if ui.is_rect_visible(rect) {
            let enabled = ui.is_enabled();
            let fill = if !enabled {
                visual.fill.resting
            } else if response.is_pointer_button_down_on() {
                visual.fill.pressed
            } else if response.hovered() {
                visual.fill.hovered
            } else {
                visual.fill.resting
            };

            // Design `.btn.disabled{box-shadow:none}` — the glow drops out; the
            // 42% fade itself is applied by egui through `disabled_alpha`.
            if let Some(shadow) = visual.shadow.filter(|_| enabled) {
                ui.painter().add(shadow.as_shape(rect, RADIUS_CONTROL));
            }
            ui.painter().rect_filled(rect, RADIUS_CONTROL, fill);
            if let Some(tint) = visual.tint {
                ui.painter().rect_filled(rect, RADIUS_CONTROL, tint);
            }
            if visual.stroke != Stroke::NONE {
                ui.painter().rect_stroke(
                    rect,
                    RADIUS_CONTROL,
                    visual.stroke,
                    egui::StrokeKind::Inside,
                );
            }

            let pos = rect.center() - galley.rect.center().to_vec2();
            // Faux semibold (design `font-weight:600`): a second pass shifted
            // 0.5 px right thickens vertical strokes.
            ui.painter()
                .galley(pos + egui::vec2(0.5, 0.0), galley.clone(), visual.text);
            ui.painter().galley(pos, galley, visual.text);
        }

        response.on_hover_cursor(egui::CursorIcon::PointingHand)
    }

    #[allow(clippy::too_many_arguments)]
    fn text_button(
        ui: &mut egui::Ui,
        label: &str,
        icon: Option<&str>,
        size: f32,
        normal_color: Color32,
        hover_color: Color32,
        width: Option<f32>,
        height: Option<f32>,
    ) -> egui::Response {
        // Transparent sizing job — allocates correct space for icon+label.
        let sizing_job = Self::make_layout_job(icon, label, size, Color32::TRANSPARENT);
        let button = egui::Button::new(sizing_job)
            .fill(Color32::TRANSPARENT)
            .stroke(egui::Stroke::NONE);

        let response = ui
            .scope(|ui| {
                let w = &mut ui.visuals_mut().widgets;
                w.inactive.weak_bg_fill = Color32::TRANSPARENT;
                w.inactive.expansion = 0.0;
                w.inactive.bg_stroke = egui::Stroke::NONE;
                w.hovered.weak_bg_fill = Color32::TRANSPARENT;
                w.hovered.expansion = 0.0;
                w.hovered.bg_stroke = egui::Stroke::NONE;
                w.active.weak_bg_fill = Color32::TRANSPARENT;
                w.active.expansion = 0.0;
                w.active.bg_stroke = egui::Stroke::NONE;

                if let Some(w) = width {
                    let h = height.unwrap_or(0.0);
                    ui.add_sized(egui::vec2(w, h), button)
                } else {
                    ui.add(button)
                }
            })
            .inner;

        if ui.is_rect_visible(response.rect) {
            let color = if response.is_pointer_button_down_on() || response.hovered() {
                hover_color
            } else {
                normal_color
            };
            let paint_job = Self::make_layout_job(icon, label, size, color);
            let galley = ui.painter().layout_job(paint_job);
            let pos = response.rect.center() - galley.rect.center().to_vec2();
            ui.painter()
                .galley(pos + egui::vec2(0.5, 0.0), galley.clone(), color);
            ui.painter().galley(pos, galley, color);
        }

        response
    }
}

#[cfg(feature = "egui")]
impl egui::Widget for Button {
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        let colors = ThemeColors::from_ctx(ui.ctx());

        let (default_font, default_h) = self.button_size.metrics();
        let size = self.size.unwrap_or(default_font);
        let height = Some(self.height.unwrap_or(default_h));
        let icon = self.icon.as_deref();
        // Design `.btn.sm` tightens its horizontal padding along with its height.
        let padding_x = match self.button_size {
            ButtonSize::Small => PADDING_X_SMALL,
            _ => PADDING_X,
        };
        // Full-width buttons stretch to the container's available width.
        let width = if self.full_width {
            Some(ui.available_width())
        } else {
            self.width
        };

        let mut response = ui
            .scope(|ui| {
                // Design `.btn.disabled{opacity:.42}`. egui fades disabled widgets
                // by `disabled_alpha`, so set the design value here instead of
                // dimming a second time by hand.
                ui.visuals_mut().disabled_alpha = DISABLED_OPACITY;
                ui.add_enabled_ui(self.enabled, |ui| match self.button_type {
                    ButtonType::Elevated | ButtonType::Soft => {
                        let soft = self.button_type == ButtonType::Soft;
                        Self::filled_button(
                            ui,
                            // Colour is applied at paint time, not baked in.
                            Self::make_layout_job(icon, &self.label, size, Color32::PLACEHOLDER),
                            Visual::resolve(self.color, soft, &colors),
                            padding_x,
                            width,
                            height,
                        )
                    }
                    ButtonType::Text => {
                        // Text buttons paint with their semantic color; preserve it on
                        // hover — brightened by the same step as a solid fill — instead
                        // of falling back to the default foreground.
                        let semantic = match self.color {
                            ButtonColor::Default => None,
                            ButtonColor::Primary | ButtonColor::Secondary => {
                                Some(colors.accent_secondary)
                            }
                            ButtonColor::Danger => Some(colors.error),
                            ButtonColor::Success => Some(colors.success),
                            ButtonColor::Warning => Some(colors.warning),
                        };
                        let (normal_color, hover_color) = match semantic {
                            Some(hue) => (hue, hue.gamma_multiply(HOVER_BRIGHTEN)),
                            None => (colors.fg_muted, colors.fg),
                        };
                        Self::text_button(
                            ui,
                            &self.label,
                            icon,
                            size,
                            normal_color,
                            hover_color,
                            width,
                            height,
                        )
                    }
                })
                .inner
            })
            .inner;

        if let Some(hover_text) = self.hover_text {
            response = crate::theme::hover_text(response, hover_text);
        }

        // Copy-to-clipboard on click, handled in-widget.
        if let Some(text) = &self.copy
            && response.clicked()
        {
            ui.ctx().copy_text(text.clone());
        }

        response = response.on_hover_cursor(egui::CursorIcon::PointingHand);

        response
    }
}
