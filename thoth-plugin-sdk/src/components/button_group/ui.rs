use egui::{Color32, CursorIcon, Margin, Sense, TextFormat, Widget, text::LayoutJob};

use crate::theme::{FONT_CONTROL, RADIUS_CHIP, ThemeColors, phosphor_font_id, thumb_shadow};

use super::ButtonGroups;

/// Corner radius of the outer track — design `.seg{border-radius:9px}`. One step
/// above the segment radius so the selected thumb nests inside it.
const TRACK_RADIUS: u8 = 9;
/// Inset between the track and its segments — design `.seg{padding:2px}`.
const TRACK_PADDING: i8 = 2;
/// Gap between segments — design `.seg{gap:2px}`.
const SEGMENT_GAP: f32 = 2.0;
/// Track height — design `.seg{height:23px}` and `.seg.mini{height:20px}`.
///
/// The *track*, not the segment: the segments sit inside it, inset by the
/// track's padding. Sizing the segment at 23 made the control 27 tall, which
/// is what put the Builder/SQL switch out of scale with the strip it sits in.
const TRACK_HEIGHT: f32 = 23.0;
const TRACK_HEIGHT_MINI: f32 = 20.0;
/// Its padding — design `.seg{padding:2px}` / `.seg.mini{padding:1px}`.
const TRACK_PADDING_MINI: i8 = 1;
/// Segment horizontal padding — design `.seg .s{padding:0 11px}` and
/// `.seg.mini button{padding:0 7px}`.
const SEGMENT_PADDING_X: f32 = 11.0;
const SEGMENT_PADDING_X_MINI: f32 = 7.0;
/// Label size — design `.seg.mini button{font-size:10.5px}`.
const FONT_MINI: f32 = 10.5;
/// Gap between a segment's icon and its label — design `.seg .s{gap:6px}`.
const ICON_GAP: f32 = 6.0;

impl ButtonGroups {
    /// Render the segmented control and report the user's selection.
    ///
    /// The active segment is `self.active`. The returned
    /// [`egui::InnerResponse::inner`] is `Some(value)` when the user clicked a
    /// *different* segment this frame, and `None` otherwise. Write that value
    /// back into your own state and pass it in as `active` next frame.
    pub fn show(&self, ui: &mut egui::Ui) -> egui::InnerResponse<Option<String>> {
        let colors = ThemeColors::from_ctx(ui.ctx());
        let mut selected: Option<String> = None;
        let mini = matches!(self.size, crate::components::Size::Small);
        let (track_pad, track_h, pad_x, font) = if mini {
            (
                TRACK_PADDING_MINI,
                TRACK_HEIGHT_MINI,
                SEGMENT_PADDING_X_MINI,
                FONT_MINI,
            )
        } else {
            (TRACK_PADDING, TRACK_HEIGHT, SEGMENT_PADDING_X, FONT_CONTROL)
        };
        // The segments sit inside the track, so their height is what is left
        // of it once its padding is taken off both edges.
        let segment_h = track_h - f32::from(track_pad) * 2.0;

        // Design `.seg` — the track is the deepest background, so the selected
        // segment reads as a raised thumb sitting in a groove.
        let frame = egui::Frame::new()
            .fill(colors.bg_sunken)
            .corner_radius(TRACK_RADIUS)
            .inner_margin(Margin::same(track_pad))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = SEGMENT_GAP;
                ui.horizontal(|ui| {
                    for item in &self.items {
                        let is_active = item.value == self.active;
                        let response =
                            render_segment(ui, item, is_active, &colors, segment_h, pad_x, font);
                        if response.clicked() && !is_active {
                            selected = Some(item.value.clone());
                        }
                    }
                });
            });

        egui::InnerResponse::new(selected, frame.response)
    }
}

fn render_segment(
    ui: &mut egui::Ui,
    item: &super::ButtonGroupItem,
    is_active: bool,
    colors: &ThemeColors,
    height: f32,
    padding_x: f32,
    font: f32,
) -> egui::Response {
    // Lay the label out with a placeholder colour so the real one — which depends
    // on hover, only known after allocation — can be applied at paint time.
    let mut job = LayoutJob::default();
    let mut gap = 0.0;
    if let Some(icon) = item.icon.as_deref() {
        job.append(
            icon,
            0.0,
            TextFormat {
                font_id: phosphor_font_id(font),
                color: Color32::PLACEHOLDER,
                valign: egui::Align::Center,
                ..Default::default()
            },
        );
        gap = ICON_GAP;
    }
    job.append(
        &item.label,
        gap,
        TextFormat {
            // Design `.seg .s{font-weight:500}` — a real medium face. egui has no
            // weight axis, so the host registers weight 500 as its own family.
            font_id: crate::theme::medium_font_id(ui.ctx(), font),
            color: Color32::PLACEHOLDER,
            valign: egui::Align::Center,
            ..Default::default()
        },
    );
    let galley = ui.painter().layout_job(job);

    let desired = egui::vec2(galley.size().x + padding_x * 2.0, height);
    let (rect, response) = ui.allocate_exact_size(desired, Sense::click());

    if ui.is_rect_visible(rect) {
        // Design `.seg .s.on` — surface fill lifted off the track by a tight shadow.
        if is_active {
            ui.painter().add(thumb_shadow().as_shape(rect, RADIUS_CHIP));
            ui.painter().rect_filled(rect, RADIUS_CHIP, colors.surface);
        }

        let text_color = if is_active || response.hovered() {
            colors.fg
        } else {
            colors.fg_muted
        };
        let pos = rect.center() - galley.rect.center().to_vec2();
        ui.painter().galley(pos, galley, text_color);
    }

    if response.hovered() {
        ui.ctx().set_cursor_icon(CursorIcon::PointingHand);
    }
    crate::theme::paint_focus_ring(ui, &response, RADIUS_CHIP);

    response
}

impl Widget for ButtonGroups {
    /// Convenience for `ui.add(group)`. Renders the group but **discards** the
    /// selection — use [`ButtonGroups::show`] when you need it.
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.show(ui).response
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::components::{ButtonGroupItem, ButtonGroups, Size};

    fn with_ui<R>(f: impl FnOnce(&mut egui::Ui) -> R) -> R {
        let ctx = egui::Context::default();
        let mut fonts = egui::FontDefinitions::default();
        crate::theme::register_phosphor(&mut fonts);
        ctx.set_fonts(fonts);
        let mut f = Some(f);
        let mut out = None;
        let _ = ctx.run_ui(Default::default(), |ui| {
            if let Some(f) = f.take() {
                out = Some(f(ui));
            }
        });
        out.expect("the test frame ran")
    }

    fn track_height(size: Size) -> f32 {
        with_ui(|ui| {
            ButtonGroups::builder()
                .id("seg")
                .active("a")
                .size(size)
                .items(vec![
                    ButtonGroupItem::builder()
                        .value("a")
                        .label("Builder")
                        .build(),
                    ButtonGroupItem::builder().value("b").label("SQL").build(),
                ])
                .build()
                .show(ui)
                .response
                .rect
                .height()
        })
    }

    #[test]
    fn the_track_is_the_height_the_design_gives_it() {
        // The *track*, not the segment. Sizing the segment at 23 and then
        // padding the track by 2 on each edge made the control 27 tall, which
        // is what put the Builder/SQL switch out of scale with its strip.
        assert_eq!(track_height(Size::Medium), TRACK_HEIGHT);
        assert_eq!(track_height(Size::Small), TRACK_HEIGHT_MINI);
    }

    #[test]
    fn mini_is_smaller_than_the_standard_track() {
        assert!(track_height(Size::Small) < track_height(Size::Medium));
    }
}
