use crate::components::traits::StatefulComponent;
use crate::search::{QueryMode, Search as SearchState, SearchMessage, decode_history_entry};
use crate::theme::GUTTER_GAP;
use eframe::egui;
use thoth_plugin_sdk::components::{
    Checkbox, Input, List, ListEvent, ListItem, ListItemPrefix, Separator, SidebarHeader,
    SidebarHeaderAction, Typography,
};

/// Height cap on the recent-searches list, so it can't crowd out the results.
const HISTORY_MAX_H: f32 = 300.0;
/// Height cap on the results list. A cap is what keeps the list's own scroll
/// area (and therefore its row virtualisation) alive inside the sidebar's.
const RESULTS_MAX_H: f32 = 300.0;

/// Detect query mode based on whether the query starts with '$'
fn detect_query_mode(query: &str) -> QueryMode {
    if query.trim_start().starts_with('$') {
        QueryMode::JsonPath
    } else {
        QueryMode::Text
    }
}

/// Props passed to the Search panel (immutable, one-way binding)
pub struct SearchProps<'a> {
    /// Whether this is the first render since the panel was opened
    pub just_opened: bool,
    /// Current search state with results
    pub search_state: &'a SearchState,
    /// Search history for the current file
    pub search_history: Option<&'a Vec<String>>,
}

/// Events emitted by the Search panel
pub enum SearchEvent {
    Search(SearchMessage),
    /// User clicked on a search result to navigate to it
    NavigateToResult {
        record_index: usize,
    },
    /// User clicked to clear search history
    ClearHistory,
}

pub struct SearchOutput {
    pub events: Vec<SearchEvent>,
}

/// Stateful search panel component for sidebar
#[derive(Default)]
pub struct Search {
    search_query: String,
    match_case: bool,
}

impl StatefulComponent for Search {
    type Props<'a> = SearchProps<'a>;
    type Output = SearchOutput;

    fn render(&mut self, ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();

        // Header with buttons
        let action_clicked = SidebarHeader::builder()
            .title("SEARCH")
            .actions(vec![
                SidebarHeaderAction::builder()
                    .icon(egui_phosphor::regular::MAGNIFYING_GLASS)
                    .tooltip("Search")
                    .build(),
                SidebarHeaderAction::builder()
                    .icon(egui_phosphor::regular::X)
                    .tooltip("Clear search")
                    .build(),
            ])
            .build()
            .show(ui)
            .inner;
        match action_clicked {
            // Search
            Some(0) if !self.search_query.is_empty() => {
                let query_mode = detect_query_mode(&self.search_query);
                if let Some(msg) = SearchMessage::create_search(
                    self.search_query.clone(),
                    self.match_case,
                    query_mode,
                ) {
                    events.push(SearchEvent::Search(msg));
                }
            }
            // Clear
            Some(1) => {
                self.search_query.clear();
                let query_mode = detect_query_mode("");
                if let Some(msg) =
                    SearchMessage::create_search(String::new(), self.match_case, query_mode)
                {
                    events.push(SearchEvent::Search(msg));
                }
            }
            _ => {}
        }
        ui.add_space(GUTTER_GAP);

        // The query field and the match-case box sit in the panel's content
        // gutter, aligned with the row cards below (design `.cscroll` padding).
        let (search_out, edited_query) = egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(GUTTER_GAP as i8, 0))
            .show(ui, |ui| {
                let mut search_input = Input::builder()
                    .id("search_query")
                    .value(self.search_query.clone())
                    .placeholder("Search… ($ prefix for JSONPath, e.g. $.user.name = \"alice\")")
                    .icon(egui_phosphor::regular::MAGNIFYING_GLASS)
                    .build();
                let out = search_input.show(ui);
                (out, search_input.value)
            })
            .inner;
        if search_out.inner {
            self.search_query = edited_query;
        }
        let response = search_out.response;

        if props.just_opened {
            response.request_focus();
        }

        response.widget_info(|| {
            egui::WidgetInfo::text_edit(
                ui.is_enabled(),
                &self.search_query,
                &self.search_query,
                "Search...",
            )
        });

        let should_search = (response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));

        if should_search && !self.search_query.is_empty() {
            let query_mode = detect_query_mode(&self.search_query);
            if let Some(msg) =
                SearchMessage::create_search(self.search_query.clone(), self.match_case, query_mode)
            {
                events.push(SearchEvent::Search(msg));
            }
        }

        ui.add_space(GUTTER_GAP);

        // Match case — the SDK checkbox (design `.cb`), not egui's default.
        let checkbox_response = egui::Frame::new()
            .inner_margin(egui::Margin::symmetric(GUTTER_GAP as i8, 0))
            .show(ui, |ui| {
                let mut checkbox = Checkbox::builder()
                    .id("search_match_case")
                    .label("Match case")
                    .checked(self.match_case)
                    .build();
                let response = checkbox.show(ui);
                self.match_case = checkbox.checked;
                response
            })
            .inner;
        checkbox_response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Checkbox,
                ui.is_enabled(),
                self.match_case,
                "Match case",
            )
        });

        ui.add_space(GUTTER_GAP);

        // Display search history if no active search and history exists
        if props.search_state.query.is_empty()
            && let Some(history) = props.search_history
        {
            let queries: Vec<String> = history
                .iter()
                .map(|e| decode_history_entry(e).1)
                .filter(|q| !q.trim().is_empty())
                .collect();

            if !queries.is_empty() {
                ui.add(Separator::with_margins(0.0, GUTTER_GAP));

                // A titled section with a trailing action *is* SidebarHeader
                // (design `.sh` / `.shacts`) — no need to hand-roll one.
                let cleared = SidebarHeader::builder()
                    .title("RECENT SEARCHES")
                    .actions(vec![
                        SidebarHeaderAction::builder()
                            .icon(egui_phosphor::regular::X)
                            .tooltip("Clear search history")
                            .build(),
                    ])
                    .build()
                    .show(ui)
                    .inner;
                if cleared == Some(0) {
                    events.push(SearchEvent::ClearHistory);
                }
                ui.add_space(GUTTER_GAP / 2.0);

                let items: Vec<ListItem> = queries
                    .iter()
                    .map(|q| {
                        ListItem::builder()
                            .title(q.clone())
                            .prefix(ListItemPrefix::Icon {
                                glyph: egui_phosphor::regular::CLOCK_COUNTER_CLOCKWISE.to_string(),
                                color: None,
                            })
                            .build()
                    })
                    .collect();

                if let Some(ListEvent::ItemClicked(idx)) = List::builder()
                    .items(items)
                    // Size to the rows, but cap the box so a long history can't
                    // push the results out of the panel.
                    .shrink_to_fit(true)
                    .max_height(HISTORY_MAX_H)
                    .build()
                    .show(ui)
                    && let Some(q) = queries.get(idx)
                {
                    self.search_query = q.clone();
                    let query_mode = detect_query_mode(q);
                    if let Some(msg) =
                        SearchMessage::create_search(q.clone(), self.match_case, query_mode)
                    {
                        events.push(SearchEvent::Search(msg));
                    }
                }
            }
        }

        ui.add(Separator::with_margins(0.0, 8.0));

        // Display search results list
        if !props.search_state.query.is_empty() {
            let result_count = props.search_state.results.len();

            if props.search_state.scanning {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(14.0));
                    ui.label("Searching...");
                });
            } else if result_count > 0 {
                Typography::caption(ui, &format!("{} result(s)", result_count));
                ui.add_space(GUTTER_GAP / 2.0);

                let hits = props.search_state.results.hits();
                let titles: Vec<String> = hits
                    .iter()
                    .map(|hit| format!("Record #{}", hit.record_index))
                    .collect();
                let descriptions: Vec<Option<String>> = hits
                    .iter()
                    .map(|hit| {
                        hit.preview.as_ref().map(|p| {
                            format!(
                                "{}{}{}",
                                p.before.trim(),
                                p.highlight.trim(),
                                p.after.trim()
                            )
                        })
                    })
                    .collect();
                let items: Vec<ListItem> = titles
                    .iter()
                    .zip(descriptions.iter())
                    .map(|(title, desc): (&String, &Option<String>)| {
                        ListItem::builder()
                            .title(title.clone())
                            .maybe_description(desc.clone())
                            .prefix(ListItemPrefix::Icon {
                                glyph: egui_phosphor::regular::MAGNIFYING_GLASS.to_string(),
                                color: None,
                            })
                            .build()
                    })
                    .collect();

                // `List` scrolls and virtualises its own rows — the results pane
                // must not nest another scroll area around it. The cap keeps that
                // virtualisation alive inside the sidebar's own scroll area.
                if let Some(ListEvent::ItemClicked(idx)) = List::builder()
                    .items(items)
                    .shrink_to_fit(true)
                    .max_height(RESULTS_MAX_H)
                    .build()
                    .show(ui)
                    && let Some(hit) = props.search_state.results.hits().get(idx)
                {
                    events.push(SearchEvent::NavigateToResult {
                        record_index: hit.record_index,
                    });
                }
            } else {
                Typography::body_muted(ui, "No results found");
            }
        }

        SearchOutput { events }
    }
}
