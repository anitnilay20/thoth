use crate::components::traits::ContextComponent;
use crate::search::SearchMessage;
use eframe::egui;

/// Props passed to the SearchDropdown (immutable, one-way binding)
pub struct SearchDropdownProps {}

/// Events emitted by the SearchDropdown
pub enum SearchDropdownEvent {
    Search(SearchMessage),
}

pub struct SearchDropdownOutput {
    pub events: Vec<SearchDropdownEvent>,
}

/// Stateful search dropdown component - includes both button and dropdown
/// Manages its own open/close state
#[derive(Default)]
pub struct SearchDropdown {
    is_open: bool,
    search_query: String,
    match_case: bool,
}

impl SearchDropdown {
    /// Toggle the dropdown (called by keyboard shortcut)
    pub fn toggle(&mut self) {
        self.is_open = !self.is_open;
    }

    /// Close the dropdown
    pub fn close(&mut self) {
        self.is_open = false;
    }

    /// Check if dropdown is open
    pub fn is_open(&self) -> bool {
        self.is_open
    }
}

impl ContextComponent for SearchDropdown {
    type Props<'a> = SearchDropdownProps;
    type Output = SearchDropdownOutput;

    fn render(&mut self, ctx: &egui::Context, _props: Self::Props<'_>) -> Self::Output {
        let mut events = Vec::new();

        // Render dropdown as a full-width panel if open
        if self.is_open {
            let bg_color = ctx.memory(|mem| {
                mem.data
                    .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
                    .unwrap_or_else(|| {
                        // Fallback: create default theme based on dark mode from visuals
                        let dark_mode = ctx.style().visuals.dark_mode;
                        crate::theme::Theme::for_dark_mode(dark_mode).colors()
                    })
                    .base
            });

            egui::TopBottomPanel::top("search_dropdown_panel")
                .frame(egui::Frame::NONE.fill(bg_color).inner_margin(egui::Margin {
                    left: 16,
                    right: 16,
                    top: 12,
                    bottom: 12,
                }))
                .show(ctx, |ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing = egui::vec2(8.0, 0.0);

                        // Search icon
                        ui.label(egui_phosphor::regular::MAGNIFYING_GLASS);

                        // Search input
                        let response = ui.add(
                            egui::TextEdit::singleline(&mut self.search_query)
                                .desired_width(400.0)
                                .hint_text("Search..."),
                        );

                        // Auto-focus when opened
                        response.request_focus();

                        // Handle Enter key
                        if response.lost_focus()
                            && ui.input(|i| i.key_pressed(egui::Key::Enter))
                            && !self.search_query.is_empty()
                        {
                            if let Some(msg) = SearchMessage::create_search(
                                self.search_query.clone(),
                                self.match_case,
                            ) {
                                events.push(SearchDropdownEvent::Search(msg));
                                self.is_open = false;
                            }
                        }

                        // Handle Escape key to close
                        if ui.input(|i| i.key_pressed(egui::Key::Escape)) {
                            self.is_open = false;
                        }

                        // Match case toggle
                        ui.checkbox(&mut self.match_case, "Match case");

                        // Search button
                        if ui
                            .button(format!(
                                "{} Search",
                                egui_phosphor::regular::MAGNIFYING_GLASS
                            ))
                            .clicked()
                            && !self.search_query.is_empty()
                        {
                            if let Some(msg) = SearchMessage::create_search(
                                self.search_query.clone(),
                                self.match_case,
                            ) {
                                events.push(SearchDropdownEvent::Search(msg));
                                self.is_open = false;
                            }
                        }

                        // Clear button
                        if ui
                            .button(format!("{} Clear", egui_phosphor::regular::X))
                            .clicked()
                        {
                            self.search_query.clear();
                            if let Some(msg) =
                                SearchMessage::create_search(String::new(), self.match_case)
                            {
                                events.push(SearchDropdownEvent::Search(msg));
                            }
                            self.is_open = false;
                        }
                    });
                });
        }

        SearchDropdownOutput { events }
    }
}
