use crate::components::button::{Button, ButtonColor, ButtonProps, ButtonType};
use crate::components::common::list::{List, ListItem, ListItemPostfix, ListProps};
use crate::components::common::typography::Typography;
use crate::components::icon_button::IconButtonProps;
use crate::components::traits::{StatefulComponent, StatelessComponent};
use eframe::egui;

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

        ui.add_space(8.0);
        Typography::panel_header(ui, "RECENT FILES");
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(4.0);

        egui::ScrollArea::vertical()
            .scroll([false, true])
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let items: Vec<ListItem<'_>> = props
                    .recent_files
                    .iter()
                    .map(|path| {
                        let filename = std::path::Path::new(path)
                            .file_name()
                            .and_then(|n| n.to_str())
                            .unwrap_or(path.as_str());
                        ListItem {
                            title: filename,
                            description: None,
                            prefix: Some(crate::components::common::list::ListItemPrefix::Icon {
                                glyph: egui_phosphor::regular::FILE,
                                color: None,
                            }),
                            badge: None,
                            tags: &[],
                            postfix: Some(ListItemPostfix::IconButton(IconButtonProps {
                                icon: egui_phosphor::regular::X,
                                frame: true,
                                tooltip: Some("Remove"),
                                badge_color: None,
                                size: None,
                                icon_size: None,
                                disabled: false,
                                selected: false,
                            })),
                            selected: false,
                            accent: None,
                        }
                    })
                    .collect();

                let output = List::render(
                    ui,
                    ListProps {
                        items: &items,
                        empty_label: Some("No recent files"),
                        shrink_to_fit: false,
                        show_separators: true,
                        compact: false,
                        max_height: None,
                    },
                );

                if let Some(item_idx) = output.postfix_clicked
                    && let Some(path) = props.recent_files.get(item_idx)
                {
                    events.push(RecentFilesEvent::RemoveFile(path.clone()));
                }

                if let Some(item_idx) = output.row_clicked
                    && let Some(path) = props.recent_files.get(item_idx)
                {
                    events.push(RecentFilesEvent::OpenFile(path.clone()));
                }
                ui.add_space(8.0);

                let button_response = Button::render(
                    ui,
                    ButtonProps {
                        label: "Open File...".to_string(),
                        button_type: ButtonType::Elevated,
                        color: ButtonColor::Default,
                        hover_text: None,
                        size: Some(13.0),
                        width: Some(ui.available_width() - 16.0),
                        height: Some(28.0),
                        icon: Some(egui_phosphor::regular::FILE_PLUS.to_string()),
                        ..Default::default()
                    },
                );

                if button_response.clicked {
                    events.push(RecentFilesEvent::OpenFilePicker);
                }
            });

        RecentFilesOutput { events }
    }
}
