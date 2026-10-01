//! The sidebar's saved-query list.
//!
//! A query names the columns of the file it was written on, so it belongs to
//! that file. That makes the list two things at once: the queries for the file
//! in front of you, and a set of bookmarks into every other file you have
//! queried. Choosing one of those is a request to open its file and arrive
//! with the query already applied, so the list shows both — the open file's
//! first, because that is the one you can use without leaving where you are.

use crate::app::persistent_state::SavedQuery;
use crate::components::traits::StatefulComponent;
use crate::theme::GUTTER_GAP;
use eframe::egui;
use thoth_plugin_sdk::components::{
    List, ListEvent, ListItem, ListItemAction, ListItemPrefix, SidebarHeader, Typography,
    TypographyVariant,
};

pub struct SavedQueriesProps<'a> {
    /// The open file's saved queries, newest first.
    pub queries: &'a [&'a SavedQuery],
    /// Every other file's, newest first.
    pub others: &'a [&'a SavedQuery],
    /// Which one is currently applied, if any.
    pub applied: Option<&'a str>,
    /// `None` when no file is open — the list is then all bookmarks.
    pub current_file_path: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub enum SavedQueriesEvent {
    /// Load this query. The host opens its file first when that file is not
    /// the one in front of the user.
    Apply(String),
    /// Forget it.
    Delete(String),
}

pub struct SavedQueriesOutput {
    pub events: Vec<SavedQueriesEvent>,
}

#[derive(Default)]
pub struct SavedQueries;

impl StatefulComponent for SavedQueries {
    type Props<'a> = SavedQueriesProps<'a>;
    type Output = SavedQueriesOutput;

    fn render(&mut self, ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        let mut events = Vec::new();

        if ui.available_width() < 50.0 {
            return SavedQueriesOutput { events };
        }

        let total = props.queries.len() + props.others.len();
        ui.add(
            SidebarHeader::builder()
                .title("SAVED QUERIES")
                .trailing_text(count_label(total))
                .build(),
        );
        ui.add_space(GUTTER_GAP);

        // Two lists need saying which is which. One does not — a heading over
        // a single list is just a second title. The open file's half earns a
        // heading even when it is empty, because "none for this file" is only
        // readable next to the ones that *are* for another.
        let split = is_split(props.current_file_path.is_some(), props.others.len());

        if split {
            group_heading(ui, "THIS FILE");
        }
        if props.current_file_path.is_some() || !props.queries.is_empty() {
            let empty = match props.current_file_path {
                Some(_) if props.others.is_empty() => nothing_saved_yet(),
                // The others are below and are the useful thing to read, so
                // this half says only that it is empty.
                Some(_) => "None for this file yet".to_string(),
                None => String::new(),
            };
            section(ui, props.queries, props.applied, false, &empty, &mut events);
        }

        if !props.others.is_empty() {
            if split {
                ui.add_space(GUTTER_GAP);
                group_heading(ui, "OTHER FILES");
            }
            // Away from their file, so each says which file it will open.
            section(ui, props.others, props.applied, true, "", &mut events);
        }

        if total == 0 && props.current_file_path.is_none() {
            ui.add(
                Typography::builder()
                    .text(nothing_saved_yet())
                    .variant(TypographyVariant::BodyMuted)
                    .build(),
            );
        }

        SavedQueriesOutput { events }
    }
}

/// Whether the pane shows two labelled lists or one unlabelled one.
///
/// Two lists need saying which is which; one does not, and a heading over a
/// single list is just a second title. The open file's half keeps its heading
/// even when it holds nothing, because "none for this file" only reads as an
/// answer beside the ones that are for another.
fn is_split(has_file: bool, others: usize) -> bool {
    has_file && others > 0
}

/// What ⌘S does, in the user's words and with this machine's key.
fn nothing_saved_yet() -> String {
    format!(
        "Nothing saved yet — {}S saves the query as it stands: table, filters, \
         grouping and columns together.",
        crate::shortcuts::marks::command()
    )
}

fn count_label(total: usize) -> String {
    match total {
        0 => String::new(),
        1 => "1".to_string(),
        n => n.to_string(),
    }
}

/// A heading over one of the two lists — design `.sgroup h3`.
fn group_heading(ui: &mut egui::Ui, text: &str) {
    ui.add(
        Typography::builder()
            .text(text)
            .variant(TypographyVariant::PanelHeader)
            .build(),
    );
    ui.add_space(GUTTER_GAP);
}

/// One list of queries. `name_file` puts the file under the name, for rows
/// whose file is not the one on screen.
fn section(
    ui: &mut egui::Ui,
    queries: &[&SavedQuery],
    applied: Option<&str>,
    name_file: bool,
    empty_label: &str,
    events: &mut Vec<SavedQueriesEvent>,
) {
    let items: Vec<ListItem> = queries
        .iter()
        .map(|q| {
            // The query read back in words, so a row says what it does and not
            // only what it was called — or, away from its own file, which file
            // choosing it will open.
            let description = if name_file {
                file_name(&q.file_path)
            } else {
                q.spec.summary()
            };
            ListItem::builder()
                .title(q.name.clone())
                .description(description)
                .selected(applied == Some(q.id.as_str()))
                .prefix(ListItemPrefix::Icon {
                    glyph: egui_phosphor::regular::BOOKMARK_SIMPLE.to_string(),
                    color: None,
                })
                .actions(vec![
                    ListItemAction::builder()
                        .icon(egui_phosphor::regular::TRASH)
                        .tooltip("Forget this query")
                        .build(),
                ])
                .build()
        })
        .collect();

    // `List` owns its own scroll area — no outer one. `shrink_to_fit` sizes it
    // to its rows and lets the sidebar's scroll area do the scrolling.
    let event = List::builder()
        .items(items)
        .empty_label(empty_label)
        .shrink_to_fit(true)
        .build()
        .show(ui);

    match event {
        Some(ListEvent::ItemClicked(index)) => {
            if let Some(q) = queries.get(index) {
                events.push(SavedQueriesEvent::Apply(q.id.clone()));
            }
        }
        Some(ListEvent::ActionClicked { item, .. }) => {
            if let Some(q) = queries.get(item) {
                events.push(SavedQueriesEvent::Delete(q.id.clone()));
            }
        }
        _ => {}
    }
}

/// The last component of a path, for naming the file a bookmark opens.
fn file_name(path: &str) -> String {
    std::path::Path::new(path)
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pane_is_split_only_when_there_are_two_lists_to_tell_apart() {
        // A file open, with queries elsewhere — the only case with two lists.
        assert!(is_split(true, 2));
        // A file open and nothing saved anywhere else: one list, no heading,
        // whether or not this file has queries of its own.
        assert!(!is_split(true, 0));
        // No file open: every query is a bookmark, so there is nothing to
        // separate them from.
        assert!(!is_split(false, 5));
        assert!(!is_split(false, 0));
    }

    #[test]
    fn a_bookmark_is_named_by_its_file() {
        assert_eq!(file_name("/tmp/logs/events.json"), "events.json");
        // A path that ends in no name is better shown whole than as nothing.
        assert_eq!(file_name("/"), "/");
    }
}
