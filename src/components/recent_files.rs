use crate::components::traits::StatefulComponent;
use eframe::egui;
use thoth_plugin_sdk::components::{
    Button, ButtonColor, ButtonType, IconButton, List, ListEvent, ListItem, ListItemPostfix,
    ListItemPrefix, SidebarHeader,
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
        ui.add_space(4.0);

        egui::ScrollArea::vertical()
            .scroll([false, true])
            .auto_shrink([false, false])
            .show(ui, |ui| {
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
                            .postfix(ListItemPostfix::IconButton(
                                IconButton::builder()
                                    .icon(egui_phosphor::regular::X)
                                    .frame(true)
                                    .tooltip("Remove")
                                    .build(),
                            ))
                            .build()
                    })
                    .collect();

                match List::builder()
                    .items(items)
                    .empty_label("No recent files")
                    .build()
                    .show(ui)
                {
                    Some(ListEvent::PostfixClicked(i)) => {
                        if let Some(path) = props.recent_files.get(i) {
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
                ui.add_space(8.0);

                let avail = ui.available_width();
                let clicked = ui
                    .add(
                        Button::builder()
                            .label("Open File...")
                            .button_type(ButtonType::Elevated)
                            .color(ButtonColor::Default)
                            .size(13.0)
                            .width(avail - 16.0)
                            .height(28.0)
                            .icon(egui_phosphor::regular::FILE_PLUS)
                            .build(),
                    )
                    .clicked();
                if clicked {
                    events.push(RecentFilesEvent::OpenFilePicker);
                }
            });

        RecentFilesOutput { events }
    }
}
