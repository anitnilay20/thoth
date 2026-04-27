use crate::app::persistent_state::Bookmark;
use crate::components::common::list::{List, ListItem, ListItemAction, ListProps};
use crate::components::traits::{StatefulComponent, StatelessComponent};
use eframe::egui;

pub struct BookmarksProps<'a> {
    pub bookmarks: &'a [Bookmark],
    pub current_file_path: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub enum BookmarksEvent {
    NavigateToBookmark { file_path: String, path: String },
    RemoveBookmark(usize),
    JumpToPath(String),
}

pub struct BookmarksOutput {
    pub events: Vec<BookmarksEvent>,
}

#[derive(Default)]
pub struct Bookmarks {
    jump_input: String,
}

impl StatefulComponent for Bookmarks {
    type Props<'a> = BookmarksProps<'a>;
    type Output = BookmarksOutput;

    fn render(&mut self, ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        let mut events = Vec::new();

        if ui.available_width() < 50.0 {
            return BookmarksOutput { events };
        }

        let (header_color, muted_color) = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
                .map(|c| (c.sidebar_header, c.overlay1))
                .unwrap_or_else(|| {
                    (
                        egui::Color32::from_rgb(153, 153, 153),
                        egui::Color32::from_rgb(128, 128, 128),
                    )
                })
        });

        // Header
        ui.add_space(8.0);
        ui.label(
            egui::RichText::new("BOOKMARKS")
                .size(11.0)
                .color(header_color)
                .strong(),
        );
        ui.add_space(4.0);
        ui.separator();
        ui.add_space(8.0);

        // Jump-to-path input
        ui.horizontal(|ui| {
            ui.add_space(4.0);
            ui.label(egui::RichText::new(egui_phosphor::regular::CROSSHAIR).color(header_color));
            ui.add_space(4.0);
            let response = ui.add(
                egui::TextEdit::singleline(&mut self.jump_input)
                    .hint_text("Jump to path (e.g., 0.user.name)")
                    .desired_width(ui.available_width() - 8.0)
                    .font(egui::FontId::proportional(13.0)),
            );
            if response.lost_focus()
                && ui.input(|i| i.key_pressed(egui::Key::Enter))
                && !self.jump_input.is_empty()
            {
                events.push(BookmarksEvent::JumpToPath(self.jump_input.clone()));
                self.jump_input.clear();
            }
            if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                self.jump_input.clear();
            }
        });

        ui.add_space(8.0);
        ui.separator();
        ui.add_space(4.0);

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let show_filename: Vec<bool> = props
                    .bookmarks
                    .iter()
                    .map(|b| props.current_file_path != Some(&b.file_path))
                    .collect();

                let items: Vec<ListItem<'_>> = props
                    .bookmarks
                    .iter()
                    .enumerate()
                    .map(|(i, b)| {
                        let title = b.label.as_deref().unwrap_or(b.path.as_str());
                        let description = if show_filename[i] {
                            Some(
                                std::path::Path::new(&b.file_path)
                                    .file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or(b.file_path.as_str()),
                            )
                        } else {
                            None
                        };
                        ListItem {
                            title,
                            description,
                            icon: Some(egui_phosphor::regular::BOOKMARK_SIMPLE),
                            icon_color: Some(muted_color),
                            badge: None,
                            action: vec![ListItemAction {
                                icon: egui_phosphor::regular::X,
                                tooltip: "Remove bookmark",
                            }],
                        }
                    })
                    .collect();

                let output = List::render(
                    ui,
                    ListProps {
                        items: &items,
                        empty_label: Some("No bookmarks — press Cmd+D to add one"),
                    },
                );

                if let Some(item_idx) = output.row_clicked {
                    if let Some(b) = props.bookmarks.get(item_idx) {
                        events.push(BookmarksEvent::NavigateToBookmark {
                            file_path: b.file_path.clone(),
                            path: b.path.clone(),
                        });
                    }
                }
                if let (Some(item_idx), Some(0)) = output.action_clicked {
                    events.push(BookmarksEvent::RemoveBookmark(item_idx));
                }
            });

        BookmarksOutput { events }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bookmarks_default() {
        let bookmarks = Bookmarks::default();
        assert_eq!(bookmarks.jump_input, "");
    }

    #[test]
    fn test_bookmarks_event_navigate_debug() {
        let event = BookmarksEvent::NavigateToBookmark {
            file_path: "/test.json".to_string(),
            path: "0.user".to_string(),
        };
        assert!(format!("{:?}", event).contains("NavigateToBookmark"));
    }

    #[test]
    fn test_bookmarks_event_remove_debug() {
        let event = BookmarksEvent::RemoveBookmark(5);
        assert!(format!("{:?}", event).contains("RemoveBookmark"));
    }

    #[test]
    fn test_bookmarks_event_jump_debug() {
        let event = BookmarksEvent::JumpToPath("test.path".to_string());
        assert!(format!("{:?}", event).contains("JumpToPath"));
    }

    #[test]
    fn test_bookmarks_event_clone() {
        let event = BookmarksEvent::JumpToPath("test".to_string());
        let cloned = event.clone();
        assert!(matches!(cloned, BookmarksEvent::JumpToPath(_)));
    }
}
