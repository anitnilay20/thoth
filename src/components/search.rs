use std::sync::Arc;

use crate::components::icon_button::{IconButton, IconButtonProps};
use crate::components::traits::{StatefulComponent, StatelessComponent};
use crate::search::results::MatchPreview;
use crate::search::{Search as SearchState, SearchMessage};
use eframe::egui::{self, WidgetText, text::LayoutJob};

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

        // Get theme colors for header
        let theme_colors = ui.ctx().memory(|mem| {
            mem.data
                .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
        });

        let (header_color, _, input_bg) = if let Some(colors) = theme_colors {
            (colors.sidebar_header, colors.text, colors.surface0)
        } else {
            (
                egui::Color32::from_rgb(153, 153, 153),
                egui::Color32::from_rgb(204, 204, 204),
                egui::Color32::from_rgb(49, 50, 68),
            )
        };

        // Header with buttons
        ui.add_space(8.0);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("SEARCH")
                    .size(11.0)
                    .color(header_color)
                    .strong(),
            );

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
                    },
                );
                if clear_output.clicked {
                    self.search_query.clear();
                    if let Some(msg) = SearchMessage::create_search(String::new(), self.match_case)
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
                    },
                );
                if search_output.clicked && !self.search_query.is_empty() {
                    if let Some(msg) =
                        SearchMessage::create_search(self.search_query.clone(), self.match_case)
                    {
                        events.push(SearchEvent::Search(msg));
                    }
                }
            });
        });

        ui.add_space(4.0);
        ui.separator();
        ui.add_space(8.0);

        // Search input field with custom background
        let text_edit = egui::TextEdit::singleline(&mut self.search_query)
            .desired_width(f32::INFINITY)
            .hint_text("Search...");

        // Apply custom background color to make the input more visible
        let response = ui.add(text_edit.background_color(input_bg));

        // Auto-focus only when the panel is just opened
        if props.just_opened {
            response.request_focus();
        }

        // Add accessibility info for screen readers
        response.widget_info(|| {
            egui::WidgetInfo::text_edit(
                ui.is_enabled(),
                &self.search_query,
                &self.search_query,
                "Search...",
            )
        });

        // Handle Enter key to trigger search
        // Check for Enter key press while focused OR when losing focus with Enter
        let should_search = (response.has_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)))
            || (response.lost_focus() && ui.input(|i| i.key_pressed(egui::Key::Enter)));

        if should_search && !self.search_query.is_empty() {
            if let Some(msg) =
                SearchMessage::create_search(self.search_query.clone(), self.match_case)
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
        if props.search_state.query.is_empty() {
            if let Some(history) = props.search_history {
                if !history.is_empty() {
                    ui.separator();
                    ui.add_space(8.0);

                    // Header with clear button
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new("RECENT SEARCHES")
                                .size(11.0)
                                .color(header_color)
                                .strong(),
                        );

                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            // Clear history button
                            let clear_output = IconButton::render(
                                ui,
                                IconButtonProps {
                                    icon: egui_phosphor::regular::X,
                                    frame: false,
                                    tooltip: Some("Clear search history"),
                                    badge_color: None,
                                    size: Some(egui::vec2(16.0, 16.0)),
                                },
                            );
                            if clear_output.clicked {
                                events.push(SearchEvent::ClearHistory);
                            }
                        });
                    });
                    ui.add_space(4.0);

                    for query in history {
                        let button = egui::Button::new(egui::RichText::new(query).size(12.0))
                            .frame(true)
                            .min_size(egui::vec2(ui.available_width(), 24.0));

                        let response = ui.add(button);

                        if response.hovered() {
                            ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                        }

                        if response.clicked() {
                            // Set the query and trigger search
                            self.search_query = query.clone();
                            if let Some(msg) =
                                SearchMessage::create_search(query.clone(), self.match_case)
                            {
                                events.push(SearchEvent::Search(msg));
                            }
                        }

                        ui.add_space(2.0);
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
                ui.label(
                    egui::RichText::new(format!("{} result(s)", result_count))
                        .size(12.0)
                        .color(header_color),
                );
                ui.add_space(4.0);

                // Scrollable results list with virtual scrolling for performance
                let row_height = 54.0;
                let row_count = result_count;

                egui::ScrollArea::vertical()
                    .id_salt("search_results_scroll")
                    .auto_shrink([false, false])
                    .show_rows(ui, row_height, row_count, |ui, row_range| {
                        for idx in row_range {
                            let Some(hit) = props.search_state.results.get(idx) else {
                                continue;
                            };
                            let record_index = hit.record_index;
                            let is_even = idx % 2 == 0;
                            let bg_color = if is_even {
                                ui.visuals().faint_bg_color
                            } else {
                                ui.visuals().extreme_bg_color
                            };

                            let button_text =
                                build_result_preview_text(ui, record_index, hit.preview.as_ref());
                            let button = egui::Button::new(button_text)
                                .fill(bg_color)
                                .frame(true)
                                .min_size(egui::vec2(ui.available_width(), row_height - 4.0));

                            let response = ui.add(button);

                            // Set pointer cursor on hover
                            if response.hovered() {
                                ui.ctx().set_cursor_icon(egui::CursorIcon::PointingHand);
                            }

                            if response.clicked() {
                                events.push(SearchEvent::NavigateToResult { record_index });
                            }

                            // Add small spacing between buttons to prevent overlap
                            ui.add_space(2.0);
                        }
                    });
            } else {
                ui.label(
                    egui::RichText::new("No results found")
                        .size(12.0)
                        .color(ui.visuals().weak_text_color()),
                );
            }
        }

        SearchOutput { events }
    }
}

fn build_result_preview_text(
    ui: &egui::Ui,
    record_index: usize,
    preview: Option<&MatchPreview>,
) -> WidgetText {
    let mut job = LayoutJob::default();
    let title_format = egui::TextFormat {
        font_id: egui::FontId::proportional(13.0),
        color: ui.visuals().text_color(),
        ..Default::default()
    };

    job.append(&format!("Record #{}", record_index), 0.0, title_format);

    job.append("\n", 0.0, egui::TextFormat::default());

    let base_format = egui::TextFormat {
        font_id: egui::FontId::proportional(11.0),
        color: ui.visuals().weak_text_color(),
        ..Default::default()
    };

    let highlight_format = egui::TextFormat {
        font_id: egui::FontId::proportional(11.0),
        color: ui.visuals().strong_text_color(),
        background: ui.visuals().selection.bg_fill,
        ..Default::default()
    };

    match preview {
        Some(p) => {
            append_snippet_segment(&mut job, p.before.trim(), &base_format, false, true);

            let highlight = if p.highlight.trim().is_empty() {
                "…"
            } else {
                p.highlight.trim()
            };
            append_snippet_segment(&mut job, highlight, &highlight_format, false, true);

            append_snippet_segment(&mut job, p.after.trim(), &base_format, false, false);
        }
        None => {
            job.append(
                "Match found",
                0.0,
                egui::TextFormat {
                    color: ui.visuals().weak_text_color(),
                    ..base_format
                },
            );
        }
    }

    WidgetText::LayoutJob(Arc::new(job))
}

fn append_snippet_segment(
    job: &mut LayoutJob,
    text: &str,
    format: &egui::TextFormat,
    prepend_space: bool,
    append_space: bool,
) {
    if text.is_empty() {
        return;
    }

    let mut content = String::new();
    if prepend_space {
        content.push(' ');
    }
    content.push_str(text);
    if append_space {
        content.push(' ');
    }

    job.append(&content, 0.0, format.clone());
}
