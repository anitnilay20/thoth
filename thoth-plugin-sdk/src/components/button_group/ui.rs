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
/// Segment height — design `.seg .s{height:23px}`.
const SEGMENT_HEIGHT: f32 = 23.0;
/// Segment horizontal padding — design `.seg .s{padding:0 11px}`.
const SEGMENT_PADDING_X: f32 = 11.0;
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

        // Design `.seg` — the track is the deepest background, so the selected
        // segment reads as a raised thumb sitting in a groove.
        let frame = egui::Frame::new()
            .fill(colors.bg_sunken)
            .corner_radius(TRACK_RADIUS)
            .inner_margin(Margin::same(TRACK_PADDING))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = SEGMENT_GAP;
                ui.horizontal(|ui| {
                    for item in &self.items {
                        let is_active = item.value == self.active;
                        let response = render_segment(ui, item, is_active, &colors);
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
                font_id: phosphor_font_id(FONT_CONTROL),
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
            font_id: crate::theme::medium_font_id(ui.ctx(), FONT_CONTROL),
            color: Color32::PLACEHOLDER,
            valign: egui::Align::Center,
            ..Default::default()
        },
    );
    let galley = ui.painter().layout_job(job);

    let desired = egui::vec2(galley.size().x + SEGMENT_PADDING_X * 2.0, SEGMENT_HEIGHT);
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

    response
}

impl Widget for ButtonGroups {
    /// Convenience for `ui.add(group)`. Renders the group but **discards** the
    /// selection — use [`ButtonGroups::show`] when you need it.
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.show(ui).response
    }
}
