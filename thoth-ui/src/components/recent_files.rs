use crate::components::traits::StatefulComponent;
use crate::theme::{FIELD_HEIGHT, FONT_BODY, GUTTER_GAP};
use eframe::egui;
use thoth_plugin_sdk::components::{
    Button, ButtonColor, ButtonType, List, ListEvent, ListItem, ListItemAction, ListItemPrefix,
    SidebarHeader,
};

pub struct RecentFilesProps<'a> {
    pub recent_files: &'a [String],
}

#[derive(Debug, Clone)]
pub enum RecentFilesEvent {
    OpenFile(String),
    RemoveFile(String),
    OpenFilePicker,
}

pub struct RecentFilesOutput {
    pub events: Vec<RecentFilesEvent>,
}

#[derive(Default)]
pub struct RecentFiles;

impl StatefulComponent for RecentFiles {
    type Props<'a> = RecentFilesProps<'a>;
    type Output = RecentFilesOutput;

    fn render(&mut self, ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        let mut events = Vec::new();

        if ui.available_width() < 50.0 {
            return RecentFilesOutput { events };
        }

        ui.add(SidebarHeader::builder().title("RECENT FILES").build());

        // Design (app-mockup `.conns`): the panel's primary action sits directly
        // under the header, above the scrolling rows — not appended below them.
        ui.add_space(GUTTER_GAP / 2.0);
        ui.horizontal(|ui| {
            ui.add_space(GUTTER_GAP);
            let avail = ui.available_width() - GUTTER_GAP;
            let clicked = ui
                .add(
                    Button::builder()
                        .label("Open File...")
                        .button_type(ButtonType::Elevated)
                        .color(ButtonColor::Default)
                        .size(FONT_BODY)
                        .width(avail)
                        .height(FIELD_HEIGHT)
                        .icon(egui_phosphor::regular::FILE_PLUS)
                        .build(),
                )
                .clicked();
            if clicked {
                events.push(RecentFilesEvent::OpenFilePicker);
            }
        });
        ui.add_space(GUTTER_GAP);

        let items: Vec<ListItem> = props
            .recent_files
            .iter()
            .map(|path| {
                let filename = std::path::Path::new(path)
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or(path.as_str());
                ListItem::builder()
                    .title(filename.to_string())
                    .prefix(ListItemPrefix::Icon {
                        glyph: egui_phosphor::regular::FILE.to_string(),
                        color: None,
                    })
                    // Design `.card .cact` / `.li .lacts`: row affordances are
                    // ghost icons revealed on hover, not a framed button riding
                    // in the row's postfix slot.
                    .actions(vec![
                        ListItemAction::builder()
                            .icon(egui_phosphor::regular::X)
                            .tooltip("Remove")
                            .build(),
                    ])
                    .build()
            })
            .collect();

        // `List` owns its own scroll area, so this panel must not nest another
        // one around it. `shrink_to_fit` keeps the rows sized to their content —
        // the sidebar's scroll area is the one that scrolls.
        match List::builder()
            .items(items)
            .empty_label("No recent files")
            .shrink_to_fit(true)
            .build()
            .show(ui)
        {
            Some(ListEvent::ActionClicked { item, .. }) => {
                if let Some(path) = props.recent_files.get(item) {
                    events.push(RecentFilesEvent::RemoveFile(path.clone()));
                }
            }
            Some(ListEvent::ItemClicked(i)) => {
                if let Some(path) = props.recent_files.get(i) {
                    events.push(RecentFilesEvent::OpenFile(path.clone()));
                }
            }
            _ => {}
        }

        RecentFilesOutput { events }
    }
}
