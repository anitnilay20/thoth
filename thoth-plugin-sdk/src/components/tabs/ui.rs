use crate::render_node::UiEvent;
use crate::theme::ThemeColors;

use super::Tabs;

impl Tabs {
    /// Render the tab header and the selected panel. Mutates the selected
    /// child (panels are stateful nodes) and collects its events.
    pub fn show(&mut self, ui: &mut egui::Ui, events: &mut Vec<UiEvent>) {
        let colors = ThemeColors::from_ctx(ui.ctx());
        let state_id = egui::Id::new(("sdk_tabs", &self.id));
        let mut selected: usize = ui.ctx().data(|d| d.get_temp(state_id).unwrap_or(0));
        selected = selected.min(self.headers.len().saturating_sub(1));

        // ── Header strip ─────────────────────────────────────────────────────
        egui::Frame::new()
            .fill(colors.bg_panel)
            .inner_margin(egui::Margin::symmetric(4, 2))
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    for (i, header) in self.headers.iter().enumerate() {
                        let is_active = i == selected;
                        let text = egui::RichText::new(header)
                            .color(if is_active { colors.fg } else { colors.fg_muted });
                        if ui.selectable_label(is_active, text).clicked() {
                            selected = i;
                        }
                    }
                });
            });

        ui.ctx().data_mut(|d| d.insert_temp(state_id, selected));

        // ── Selected panel ───────────────────────────────────────────────────
        if let Some(child) = self.children.get_mut(selected) {
            child.show(ui, events);
        }
    }
}
