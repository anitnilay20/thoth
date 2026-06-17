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
        let mut changed = false;
        let mut remove: Option<usize> = None;

        ui.add_enabled_ui(!self.disabled, |ui| {
            ui.vertical(|ui| {
                if !self.label.is_empty() {
                    ui.label(&self.label);
                }
                for (i, entry) in self.entries.iter_mut().enumerate() {
                    ui.horizontal(|ui| {
                        changed |= ui.checkbox(&mut entry.enabled, "").changed();
                        changed |= ui
                            .add(egui::TextEdit::singleline(&mut entry.key).hint_text("key"))
                            .changed();
                        changed |= ui
                            .add(egui::TextEdit::singleline(&mut entry.value).hint_text("value"))
                            .changed();
                        if ui.button(egui_phosphor::regular::X).clicked() {
                            remove = Some(i);
                        }
                    });
                }
                if ui.button(&self.add_label).clicked() {
                    self.entries.push(KvEntry::builder().enabled(true).build());
                    changed = true;
                }
            });
        });

        if let Some(i) = remove {
            self.entries.remove(i);
            changed = true;
        }
        changed
    }
}
