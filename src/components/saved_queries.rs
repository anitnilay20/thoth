//! The sidebar's saved-query list.
//!
//! A query is saved against the file it was written on, because it names that
//! file's columns — a query offered for a file without them is one that fails
//! the moment it is applied. So this lists the open file's queries and says so
//! when there is no file open to have any.

use crate::app::persistent_state::SavedQuery;
use crate::components::traits::StatefulComponent;
use crate::theme::GUTTER_GAP;
use eframe::egui;
use thoth_plugin_sdk::components::{
    List, ListEvent, ListItem, ListItemAction, ListItemPrefix, SidebarHeader,
};

pub struct SavedQueriesProps<'a> {
    /// The open file's saved queries, newest first.
    pub queries: &'a [&'a SavedQuery],
    /// Which one is currently applied, if any.
    pub applied: Option<&'a str>,
    /// `None` when no file is open — the list then has nothing to be about.
    pub current_file_path: Option<&'a str>,
}

#[derive(Debug, Clone)]
pub enum SavedQueriesEvent {
    /// Load this query into the open file's builder.
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

        ui.add(SidebarHeader::builder().title("SAVED QUERIES").build());
        ui.add_space(GUTTER_GAP);

        let items: Vec<ListItem> = props
            .queries
            .iter()
            .map(|q| {
                ListItem::builder()
                    .title(q.name.clone())
                    // The query read back in words, so a row says what it does
                    // and not only what it was called.
                    .description(q.spec.summary())
                    .selected(props.applied == Some(q.id.as_str()))
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

        // `List` owns its own scroll area — no outer one. `shrink_to_fit` sizes
        // it to its rows and lets the sidebar's scroll area do the scrolling.
        // The key, written the way this machine writes it. A hardcoded "Cmd+S"
        // is wrong on every platform but one.
        let nothing_yet = format!(
            "Nothing saved yet — {}S saves the query as it stands: table, \
             filters, grouping and columns together.",
            crate::shortcuts::marks::command()
        );
        let empty = match props.current_file_path {
            Some(_) => nothing_yet.as_str(),
            None => "Open a file to see the queries saved for it",
        };
        let event = List::builder()
            .items(items)
            .empty_label(empty)
            .shrink_to_fit(true)
            .build()
            .show(ui);

        match event {
            Some(ListEvent::ItemClicked(index)) => {
                if let Some(q) = props.queries.get(index) {
                    events.push(SavedQueriesEvent::Apply(q.id.clone()));
                }
            }
            Some(ListEvent::ActionClicked { item, .. }) => {
                if let Some(q) = props.queries.get(item) {
                    events.push(SavedQueriesEvent::Delete(q.id.clone()));
                }
            }
            _ => {}
        }

        SavedQueriesOutput { events }
    }
}
