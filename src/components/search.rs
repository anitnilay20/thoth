use crate::components::common::input::{Input, InputProps};
use crate::components::common::list::{List, ListItem, ListProps};
use crate::components::common::typography::Typography;
use crate::components::icon_button::{IconButton, IconButtonProps};
use crate::components::traits::{StatefulComponent, StatelessComponent};
use crate::search::{QueryMode, Search as SearchState, SearchMessage, decode_history_entry};
use eframe::egui;

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
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            Typography::panel_header(ui, "SEARCH");

            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                // Clear button
                let clear_output = IconButton::render(
                    ui,
                    IconButtonProps {
                        icon: egui_phosphor::regular::X,
                        frame: false,
                        tooltip: Some("Clear search"),
                        badge_color: None,
                        size: None,
                        disabled: false,
                        icon_size: None,
                        selected: false,
                    },
                );
                if clear_output.clicked {
                    self.search_query.clear();
                    let query_mode = detect_query_mode("");
                    if let Some(msg) =
                        SearchMessage::create_search(String::new(), self.match_case, query_mode)
                    {
                        events.push(SearchEvent::Search(msg));
                    }
                }

                // Search button
                let search_output = IconButton::render(
                    ui,
                    IconButtonProps {
                        icon: egui_phosphor::regular::MAGNIFYING_GLASS,
                        frame: false,
                        tooltip: Some("Search"),
                        badge_color: None,
                        size: None,
                        disabled: false,
                        icon_size: None,
                        selected: false,
                    },
                );
                if search_output.clicked && !self.search_query.is_empty() {
                    let query_mode = detect_query_mode(&self.search_query);
                    if let Some(msg) = SearchMessage::create_search(
                        self.search_query.clone(),
                        self.match_case,
                        query_mode,
                    ) {
                        events.push(SearchEvent::Search(msg));
                    }
                }
            });
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(8.0);

        let search_out = Input::render(
            ui,
            InputProps {
                value: &mut self.search_query,
                placeholder: "Search… ($ prefix for JSONPath, e.g. $.user.name = \"alice\")",
                icon: Some(egui_phosphor::regular::MAGNIFYING_GLASS),
                password: false,
                disabled: false,
                multiline: false,
                rows: 1,
                desired_width: None,
                id_salt: None,
            },
        );
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

        ui.add_space(8.0);

        // Match case checkbox with accessibility info
        let checkbox_response = ui.checkbox(&mut self.match_case, "Match case");
        checkbox_response.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Checkbox,
                ui.is_enabled(),
                self.match_case,
                "Match case",
            )
        });

        ui.add_space(8.0);

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
                ui.separator();
                ui.add_space(8.0);

                ui.horizontal(|ui| {
                    Typography::panel_header(ui, "RECENT SEARCHES");
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        let clear_output = IconButton::render(
                            ui,
                            IconButtonProps {
                                icon: egui_phosphor::regular::X,
                                frame: false,
                                tooltip: Some("Clear search history"),
                                badge_color: None,
                                size: Some(egui::vec2(16.0, 16.0)),
                                disabled: false,
                                icon_size: None,
                                selected: false,
                            },
                        );
                        if clear_output.clicked {
                            events.push(SearchEvent::ClearHistory);
                        }
                    });
                });
                ui.add_space(4.0);

                let items: Vec<ListItem<'_>> = queries
                    .iter()
                    .map(|q| ListItem {
                        title: q.as_str(),
                        description: None,
                        prefix: Some(crate::components::common::list::ListItemPrefix::Icon {
                            glyph: egui_phosphor::regular::CLOCK_COUNTER_CLOCKWISE,
                            color: None,
                        }),
                        badge: None,
                        postfix: None,
                        selected: false,
                        accent: None,
                        tags: &[],
                    })
                    .collect();

                let output = List::render(
                    ui,
                    ListProps {
                        items: &items,
                        empty_label: None,
                        shrink_to_fit: false,
                        show_separators: true,
                        compact: false,
                        max_height: Some(300.0),
                    },
                );

                if let Some(idx) = output.row_clicked
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

        ui.separator();
        ui.add_space(8.0);

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
                ui.add_space(4.0);

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
                let items: Vec<ListItem<'_>> = titles
                    .iter()
                    .zip(descriptions.iter())
                    .map(|(title, desc): (&String, &Option<String>)| ListItem {
                        title: title.as_str(),
                        description: desc.as_deref(),
                        prefix: Some(crate::components::common::list::ListItemPrefix::Icon {
                            glyph: egui_phosphor::regular::MAGNIFYING_GLASS,
                            color: None,
                        }),
                        badge: None,
                        postfix: None,
                        selected: false,
                        accent: None,
                        tags: &[],
                    })
                    .collect();

                egui::ScrollArea::vertical()
                    .id_salt("search_results_scroll")
                    .auto_shrink([false, false])
                    .show(ui, |ui| {
                        let output = List::render(
                            ui,
                            ListProps {
                                items: &items,
                                empty_label: None,
                                shrink_to_fit: false,
                                show_separators: true,
                                compact: false,
                                max_height: Some(300.0),
                            },
                        );
                        if let Some(idx) = output.row_clicked
                            && let Some(hit) = props.search_state.results.hits().get(idx)
                        {
                            events.push(SearchEvent::NavigateToResult {
                                record_index: hit.record_index,
                            });
                        }
                    });
            } else {
                Typography::body_muted(ui, "No results found");
            }
        }

        SearchOutput { events }
    }
}
