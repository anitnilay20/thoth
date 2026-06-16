use egui::Widget;

use crate::theme::ThemeColors;

use super::ButtonGroups;

impl<'a> ButtonGroups<'a> {
    /// Render the segmented button group and report the user's selection.
    ///
    /// The currently-active segment is taken from `self.active`. The returned
    /// [`egui::InnerResponse::inner`] is `Some(index)` when the user clicked a
    /// *different* segment this frame, and `None` otherwise. Write that index
    /// back into your own state and pass it in as `active` next frame so the
    /// new selection renders (standard immediate-mode flow).
    pub fn show(&self, ui: &mut egui::Ui) -> egui::InnerResponse<Option<usize>> {
        let colors = ThemeColors::from_ctx(ui.ctx());

        let mut selected: Option<usize> = None;

        let frame = egui::Frame::new()
            .fill(colors.bg_panel)
            .corner_radius(6)
            .inner_margin(egui::Margin::same(2))
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing.x = 2.0;
                ui.horizontal(|ui| {
                    for (i, item) in self.items.iter().enumerate() {
                        let is_active = i == self.active;
                        let response = item.clone().ui(ui);

                        if response.clicked() && !is_active {
                            selected = Some(i);
                        }
                    }
                });
            });

        egui::InnerResponse::new(selected, frame.response)
    }
}

impl<'a> Widget for ButtonGroups<'a> {
    /// Convenience for `ui.add(group)`. Renders the group but **discards** the
    /// selection — use [`ButtonGroups::show`] when you need to know which
    /// segment was clicked.
    fn ui(self, ui: &mut egui::Ui) -> egui::Response {
        self.show(ui).response
    }
}
