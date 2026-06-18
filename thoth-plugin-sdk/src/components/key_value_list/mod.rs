use bon::Builder;
use serde::{Deserialize, Serialize};

fn default_add_label() -> String {
    "Add".to_string()
}

/// One editable key/value row in a [`KeyValueList`].
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct KvEntry {
    /// The key text.
    #[builder(default)]
    #[serde(default)]
    pub key: String,
    /// The value text.
    #[builder(default)]
    #[serde(default)]
    pub value: String,
    /// Whether the row is active; disabled rows are dimmed. Defaults to `true`.
    #[builder(default = true)]
    #[serde(default = "crate::components::key_value_list::default_true")]
    pub enabled: bool,
}

pub(crate) fn default_true() -> bool {
    true
}

/// An editable list of key/value pairs with per-row enable + add/remove. Owns
/// its `entries`; [`KeyValueList::show`] mutates them in place.
///
/// ```
/// use thoth_plugin_sdk::components::KeyValueList;
///
/// let mut kv = KeyValueList::builder().label("Headers").build();
/// ```
#[derive(Clone, Debug, Default, Serialize, Deserialize, Builder)]
#[builder(on(String, into))]
pub struct KeyValueList {
    /// Widget id used for event routing.
    #[builder(default)]
    #[serde(default)]
    pub id: String,
    /// Optional label shown above the rows.
    #[builder(default)]
    #[serde(default)]
    pub label: String,
    /// The current rows.
    #[builder(default)]
    #[serde(default)]
    pub entries: Vec<KvEntry>,
    /// Label for the "add row" button. Defaults to `"Add"`.
    #[builder(default = default_add_label())]
    #[serde(default = "default_add_label", rename = "add-label")]
    pub add_label: String,
    /// Disable interaction.
    #[builder(default)]
    #[serde(default)]
    pub disabled: bool,
}

#[cfg(feature = "egui")]
impl KeyValueList {
    /// Render the editable rows, mutating [`entries`](KeyValueList::entries) in
    /// place. Returns `true` if anything changed this frame.
    pub fn show(&mut self, ui: &mut egui::Ui) -> bool {
        use super::{Button, ButtonType, IconButton};
        use crate::theme::ThemeColors;
        let colors = ThemeColors::from_ctx(ui.ctx());

        let mut changed = false;
        let mut to_remove: Option<usize> = None;
        let disabled = self.disabled;

        if !self.label.is_empty() {
            ui.label(&self.label);
        }

        let toggle_col_w = 22.0;
        let delete_col_w = 24.0;
        let available = ui.available_width();
        let input_w = ((available - toggle_col_w - delete_col_w - 12.0) / 2.0).max(40.0);

        // ── Header row ────────────────────────────────────────────────
        // TextEdit has ~4px internal left padding; match it so header
        // labels align with the placeholder/input text below.
        let text_edit_pad = 4.0;
        let header_rect = ui
            .horizontal(|ui| {
                ui.set_width(available);
                ui.spacing_mut().item_spacing.x = 4.0;
                let label_color = colors.fg_muted;
                let font = egui::FontId::proportional(11.0);
                ui.allocate_exact_size(egui::vec2(toggle_col_w, 24.0), egui::Sense::hover());
                ui.painter().text(
                    ui.cursor().min + egui::vec2(text_edit_pad, 8.0),
                    egui::Align2::LEFT_TOP,
                    "KEY",
                    font.clone(),
                    label_color,
                );
                ui.allocate_exact_size(egui::vec2(input_w, 24.0), egui::Sense::hover());
                ui.painter().text(
                    ui.cursor().min + egui::vec2(text_edit_pad, 8.0),
                    egui::Align2::LEFT_TOP,
                    "VALUE",
                    font,
                    label_color,
                );
                ui.allocate_exact_size(egui::vec2(input_w, 24.0), egui::Sense::hover());
            })
            .response
            .rect;

        // Bottom border under header.
        let border_y = header_rect.bottom();
        ui.painter().line_segment(
            [
                egui::pos2(header_rect.left(), border_y),
                egui::pos2(header_rect.right(), border_y),
            ],
            egui::Stroke::new(1.0, colors.surface_raised),
        );

        // ── Data rows ─────────────────────────────────────────────────
        for (i, entry) in self.entries.iter_mut().enumerate() {
            ui.horizontal(|ui| {
                ui.set_width(available);
                ui.spacing_mut().item_spacing.x = 4.0;
                if ui
                    .add_sized(
                        egui::vec2(toggle_col_w, 24.0),
                        egui::Checkbox::without_text(&mut entry.enabled),
                    )
                    .changed()
                {
                    changed = true;
                }
                // Dim key/value text when the row is disabled.
                let text_color = if entry.enabled { colors.fg } else { colors.fg_muted };
                if ui
                    .add_sized(
                        egui::vec2(input_w, 24.0),
                        egui::TextEdit::singleline(&mut entry.key)
                            .frame(egui::Frame::NONE)
                            .hint_text("key")
                            .text_color(text_color)
                            .background_color(egui::Color32::TRANSPARENT),
                    )
                    .changed()
                {
                    changed = true;
                }
                if ui
                    .add_sized(
                        egui::vec2(input_w, 24.0),
                        egui::TextEdit::singleline(&mut entry.value)
                            .frame(egui::Frame::NONE)
                            .hint_text("value")
                            .text_color(text_color)
                            .background_color(egui::Color32::TRANSPARENT),
                    )
                    .changed()
                {
                    changed = true;
                }
                if !disabled
                    && ui
                        .add(
                            IconButton::builder()
                                .icon(egui_phosphor::regular::X)
                                .frame(false)
                                .tooltip("Remove")
                                .size(14.0)
                                .build(),
                        )
                        .clicked()
                {
                    to_remove = Some(i);
                }
            });

            // Row separator.
            let row_rect = ui.min_rect();
            ui.painter().line_segment(
                [
                    egui::pos2(row_rect.left(), row_rect.bottom()),
                    egui::pos2(row_rect.right(), row_rect.bottom()),
                ],
                egui::Stroke::new(1.0, colors.surface),
            );
        }

        if let Some(idx) = to_remove {
            self.entries.remove(idx);
            changed = true;
        }

        // ── Add row button ────────────────────────────────────────────
        if !disabled {
            ui.add_space(4.0);
            if ui
                .add(
                    Button::builder()
                        .label(&self.add_label)
                        .button_type(ButtonType::Text)
                        .icon(egui_phosphor::regular::PLUS)
                        .build(),
                )
                .clicked()
            {
                self.entries.push(KvEntry::builder().enabled(true).build());
                changed = true;
            }
        }

        changed
    }
}
