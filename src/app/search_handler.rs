use crate::{error::ThothError, search, state};
use eframe::egui;

/// Handles all search-related logic
pub struct SearchHandler;

impl SearchHandler {
    /// Process search messages from toolbar and background search
    /// Returns (message_to_central, error_if_any)
    pub fn handle_search_messages(
        incoming_msg: Option<search::SearchMessage>,
        search_state: &mut state::SearchEngineState,
        file_path: &Option<std::path::PathBuf>,
        file_type: &crate::file::lazy_loader::FileType,
        ctx: &egui::Context,
    ) -> (Option<search::SearchMessage>, Option<ThothError>) {
        let mut msg_to_central: Option<search::SearchMessage> = None;
        let mut search_error: Option<ThothError> = None;

        // Check if background search has completed
        if let Some(rx) = &search_state.search_rx {
            if let Ok(done) = rx.try_recv() {
                // Check if the search encountered an error
                if let Some(error) = &done.error {
                    search_error = Some(error.clone());
                }
                search_state.search = done.clone();
                msg_to_central = Some(search::SearchMessage::StartSearch(done));
                search_state.search_rx = None; // finished
            }
        }

        // Handle incoming search message from toolbar
        if let Some(msg) = incoming_msg {
            match msg {
                search::SearchMessage::StartSearch(s) => {
                    Self::start_search(s, search_state, file_path, file_type, ctx);
                    msg_to_central = Some(search::SearchMessage::StartSearch(
                        search_state.search.clone(),
                    ));
                }
                search::SearchMessage::StopSearch => {
                    Self::stop_search(search_state);
                    msg_to_central = Some(search::SearchMessage::StopSearch);
                }
            }
        }

        (msg_to_central, search_error)
    }

    // Private helper methods

    fn start_search(
        search: search::Search,
        search_state: &mut state::SearchEngineState,
        file_path: &Option<std::path::PathBuf>,
        file_type: &crate::file::lazy_loader::FileType,
        ctx: &egui::Context,
    ) {
        // Update search state
        search_state.search = search.clone();
        search_state.search.scanning = true;

        // Spawn background search
        search_state.search_rx = Some(search_state.search.start_scanning(file_path, file_type));

        // Keep UI repainting while scanning
        ctx.request_repaint();
    }

    fn stop_search(search_state: &mut state::SearchEngineState) {
        search_state.search_rx = None; // Drop pending result
    }
}
