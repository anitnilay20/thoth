use crate::app::persistent_state::Bookmark;
use crate::components::traits::StatefulComponent;
use eframe::egui;
use thoth_plugin_sdk::components::{
    Input, List, ListEvent, ListItem, ListItemPrefix, Separator, SidebarHeader,
};

pub struct BookmarksProps<'a> {
    pub bookmarks: &'a [Bookmark],
    pub current_file_path: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub enum BookmarksEvent {
    NavigateToBookmark { file_path: String, path: String },
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

        // Header
        ui.add(SidebarHeader::builder().title("BOOKMARKS").build());
        ui.add_space(8.0);

        // Jump-to-path input
        {
            let mut input = Input::builder()
                .id("jump_input")
                .value(self.jump_input.clone())
                .placeholder("Jump to path (e.g., 0.user.name)")
                .icon(egui_phosphor::regular::CROSSHAIR)
                .build();
            let r = input.show(ui);
            if r.inner {
                self.jump_input = input.value.clone();
            }
            let response = r.response;
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
        }

        ui.add(Separator::with_margins(8.0, 4.0));

        egui::ScrollArea::vertical()
            .auto_shrink([false, false])
            .show(ui, |ui| {
                let show_filename: Vec<bool> = props
                    .bookmarks
                    .iter()
                    .map(|b| props.current_file_path != Some(&b.file_path))
                    .collect();

                let items: Vec<ListItem> = props
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
                                    .unwrap_or(b.file_path.as_str())
                                    .to_string(),
                            )
                        } else {
                            None
                        };
                        ListItem::builder()
                            .title(title.to_string())
                            .maybe_description(description)
                            .prefix(ListItemPrefix::Icon {
                                glyph: egui_phosphor::regular::BOOKMARK_SIMPLE.to_string(),
                                color: None,
                            })
                            .build()
                    })
                    .collect();

                if let Some(ListEvent::ItemClicked(item_idx)) = List::builder()
                    .items(items)
                    .empty_label("No bookmarks — press Cmd+D to add one")
                    .build()
                    .show(ui)
                    && let Some(b) = props.bookmarks.get(item_idx)
                {
                    events.push(BookmarksEvent::NavigateToBookmark {
                        file_path: b.file_path.clone(),
                        path: b.path.clone(),
                    });
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
