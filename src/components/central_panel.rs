use crate::components::file_viewer::FileViewer;
use crate::components::traits::ContextComponent;
use crate::error::{ErrorHandler, ThothError};
use crate::file::loaders::FileType;
use crate::search;
use eframe::egui;
use std::path::PathBuf;

/// Props passed down to the CentralPanel (immutable, one-way binding)
pub struct CentralPanelProps<'a> {
    pub file_path: &'a Option<PathBuf>,
    pub file_type: FileType,
    pub error: &'a Option<ThothError>,
    pub search_message: Option<search::SearchMessage>,
    pub cache_size: usize,
    pub syntax_highlighting: bool,
}

/// Events emitted by the central panel (bottom-to-top communication)
pub enum CentralPanelEvent {
    FileOpened {
        path: PathBuf,
        file_type: FileType,
        total_items: usize,
    },
    FileOpenError(ThothError),
    FileClosed,
    FileTypeChanged(FileType),
    ErrorCleared,
}

pub struct CentralPanelOutput {
    pub events: Vec<CentralPanelEvent>,
}

#[derive(Default)]
pub struct CentralPanel {
    file_viewer: FileViewer,
    loaded_path: Option<PathBuf>,
    loaded_type: Option<FileType>,
    last_open_err: Option<ThothError>,
    searching: bool,
}

impl ContextComponent for CentralPanel {
    type Props<'a> = CentralPanelProps<'a>;
    type Output = CentralPanelOutput;

    fn render(&mut self, ctx: &egui::Context, props: Self::Props<'_>) -> Self::Output {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        let mut events = Vec::new();
        self.render_ui(ctx, props, &mut events);
        CentralPanelOutput { events }
    }
}

impl CentralPanel {
    fn render_ui(
        &mut self,
        ctx: &egui::Context,
        props: CentralPanelProps<'_>,
        events: &mut Vec<CentralPanelEvent>,
    ) {
        // Open / close viewer once on change
        match (props.file_path, self.loaded_path.as_ref(), self.loaded_type) {
            (Some(new_path), Some(curr_path), Some(curr_ty))
                if curr_path == new_path && curr_ty == props.file_type =>
            {
                // no change
            }
            (Some(new_path), _, _) => {
                self.last_open_err = None;
                let mut file_type = props.file_type;
                match self.file_viewer.open(new_path, &mut file_type) {
                    Ok(()) => {
                        self.loaded_path = Some(new_path.clone());
                        self.loaded_type = Some(file_type);
                        let total_items = self.file_viewer.total_item_count();
                        events.push(CentralPanelEvent::FileOpened {
                            path: new_path.clone(),
                            file_type,
                            total_items,
                        });
                        events.push(CentralPanelEvent::ErrorCleared);
                        // clear any prior search filter on new file
                        self.file_viewer.set_root_filter(None);

                        // Emit event if file type changed during opening
                        if file_type != props.file_type {
                            events.push(CentralPanelEvent::FileTypeChanged(file_type));
                        }
                    }
                    Err(e) => {
                        // Use the error as-is if it's already a ThothError variant,
                        // otherwise wrap it appropriately
                        let error = match &e {
                            ThothError::FileNotFound { .. }
                            | ThothError::FileReadError { .. }
                            | ThothError::InvalidFileType { .. }
                            | ThothError::JsonParseError { .. } => e,
                            _ => ThothError::FileReadError {
                                path: new_path.to_path_buf(),
                                reason: e.to_string(),
                            },
                        };
                        self.last_open_err = Some(error.clone());
                        events.push(CentralPanelEvent::FileOpenError(error));
                        self.loaded_path = None;
                        self.loaded_type = None;
                    }
                }
            }
            (None, Some(_), _) => {
                self.file_viewer = FileViewer::with_cache_size(props.cache_size);
                self.loaded_path = None;
                self.loaded_type = None;
                self.last_open_err = None;
                events.push(CentralPanelEvent::FileClosed);
            }
            (None, None, _) => { /* nothing selected */ }
        }

        // React to search messages
        if let Some(msg) = props.search_message {
            self.searching = msg.is_searching();

            match msg {
                search::SearchMessage::StartSearch(search) => {
                    self.file_viewer.set_highlights(Some(&search.results));
                    // Search results are now displayed in the sidebar as a clickable list
                    // Don't filter the main view - keep all records visible
                    // Users can click on search results to navigate to them
                }
                search::SearchMessage::StopSearch => {
                    // No filtering to clear
                    self.file_viewer.set_highlights(None);
                }
            }
        }

        egui::CentralPanel::default().show(ctx, |ui| {
            // Show any error (either from props or open attempt)
            if let Some(err) = props.error.as_ref().or(self.last_open_err.as_ref()) {
                let message = ErrorHandler::get_user_message(err);
                ui.colored_label(egui::Color32::RED, message);
                ui.separator();
            }

            if self.searching {
                ui.horizontal(|ui| {
                    ui.add(egui::Spinner::new().size(16.0));
                    ui.label("Searching…");
                });
                ui.add_space(6.0);
                return;
            }

            if self.loaded_path.is_none() {
                ui.label("Open a JSON/NDJSON file from the top bar to begin.");
                return;
            }

            // Update viewer settings right before rendering (so changes apply immediately)
            self.file_viewer
                .set_syntax_highlighting(props.syntax_highlighting);

            // Render the viewer (no filtering UI needed - search results shown in sidebar)
            self.file_viewer.ui(ui);
        });
    }

    // ========================================================================
    // Keyboard Shortcut Support - Wrapper methods
    // ========================================================================

    /// Expand the currently selected node (for keyboard shortcuts)
    pub fn expand_selected_node(&mut self) {
        self.file_viewer.expand_selected_node();
    }

    /// Collapse the currently selected node (for keyboard shortcuts)
    pub fn collapse_selected_node(&mut self) {
        self.file_viewer.collapse_selected_node();
    }

    /// Expand all nodes in the tree (for keyboard shortcuts)
    pub fn expand_all_nodes(&mut self) {
        self.file_viewer.expand_all_nodes();
    }

    /// Collapse all nodes in the tree (for keyboard shortcuts)
    pub fn collapse_all_nodes(&mut self) {
        self.file_viewer.collapse_all_nodes();
    }

    /// Move selection up to previous item (for keyboard shortcuts)
    pub fn move_selection_up(&mut self) {
        self.file_viewer.move_selection_up();
    }

    /// Move selection down to next item (for keyboard shortcuts)
    pub fn move_selection_down(&mut self) {
        self.file_viewer.move_selection_down();
    }

    /// Copy the key of the currently selected item (for keyboard shortcuts)
    pub fn copy_selected_key(&mut self) -> Option<String> {
        self.file_viewer.copy_selected_key()
    }

    /// Copy the value of the currently selected item (for keyboard shortcuts)
    pub fn copy_selected_value(&mut self) -> Option<String> {
        self.file_viewer.copy_selected_value()
    }

    /// Copy the entire object of the currently selected item (for keyboard shortcuts)
    pub fn copy_selected_object(&mut self) -> Option<String> {
        self.file_viewer.copy_selected_object()
    }

    /// Copy the path of the currently selected item (for keyboard shortcuts)
    pub fn copy_selected_path(&mut self) -> Option<String> {
        self.file_viewer.copy_selected_path()
    }

    /// Navigate to a specific root record (for search result navigation)
    pub fn navigate_to_record(&mut self, record_index: usize) {
        self.file_viewer.navigate_to_root(record_index);
    }
}
