use crate::components::file_viewer::FileViewer;
use crate::components::traits::ContextComponent;
use crate::error::{ErrorHandler, ThothError};
use crate::file::loaders::FileKind;
use crate::plugin::render_node::{UiEvent, UiNode, UiOutput, render_ui_node};
use crate::search;
use eframe::egui;
use std::path::PathBuf;

/// Props passed down to the CentralPanel (immutable, one-way binding)
pub struct CentralPanelProps<'a> {
    pub file_path: &'a Option<PathBuf>,
    pub file_type: FileKind,
    pub error: &'a Option<ThothError>,
    pub search_message: Option<search::SearchMessage>,
    pub cache_size: usize,
    pub syntax_highlighting: bool,
    /// When `Some`, render this interactive `UiNode` tree from the plugin instead of the file viewer.
    pub plugin_ui: Option<&'a UiOutput>,
    /// Recent files passed down for the Welcome screen shown on empty tabs.
    pub recent_files: &'a [String],
    /// Current theme colors forwarded to the Welcome screen.
    pub colors: Option<crate::theme::ThemeColors>,
}

/// Events emitted by the central panel (bottom-to-top communication)
pub enum CentralPanelEvent {
    FileOpened {
        path: PathBuf,
        file_type: FileKind,
        total_items: usize,
    },
    FileOpenError(ThothError),
    FileClosed,
    FileTypeChanged(FileKind),
    ErrorCleared,
    /// A widget interaction from the active plugin pane — forward to the loader.
    PluginUiEvent(UiEvent),
    /// User clicked "Open file…" on the Welcome screen.
    OpenFilePicker,
    /// User clicked a recent file on the Welcome screen.
    OpenRecentFile(PathBuf),
}

pub struct CentralPanelOutput {
    pub events: Vec<CentralPanelEvent>,
}

#[derive(Default)]
pub struct CentralPanel {
    file_viewer: FileViewer,
    loaded_path: Option<PathBuf>,
    loaded_type: Option<FileKind>,
    last_open_err: Option<ThothError>,
    searching: bool,
}

impl ContextComponent for CentralPanel {
    type Props<'a> = CentralPanelProps<'a>;
    type Output = CentralPanelOutput;

    fn render(&mut self, ui: &mut egui::Ui, props: Self::Props<'_>) -> Self::Output {
        #[cfg(feature = "profiling")]
        puffin::profile_function!();

        let mut events = Vec::new();
        self.render_ui(ui, props, &mut events);
        CentralPanelOutput { events }
    }
}

impl CentralPanel {
    fn render_ui(
        &mut self,
        ui: &mut egui::Ui,
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

        // Plugin panes manage their own padding, so drop the central-panel inner
        // margin for them — but keep the panel *fill* (the dock tab viewer's
        // clear_background is false, so this frame provides the background).
        let panel_frame = if props.plugin_ui.is_some() {
            egui::Frame::central_panel(ui.style()).inner_margin(0)
        } else {
            egui::Frame::central_panel(ui.style())
        };
        egui::CentralPanel::default()
            .frame(panel_frame)
            .show_inside(ui, |ui| {
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

                // Plugin pane takes priority over the file viewer.
                if let Some(output) = props.plugin_ui {
                    match serde_json::from_str::<UiNode>(&output.node_json) {
                        Ok(node) => {
                            let mut ui_events = Vec::new();
                            render_ui_node(ui, &node, &mut ui_events);
                            for evt in ui_events {
                                events.push(CentralPanelEvent::PluginUiEvent(evt));
                            }
                        }
                        Err(e) => {
                            ui.colored_label(
                                egui::Color32::RED,
                                format!("Plugin UI parse error: {e}"),
                            );
                        }
                    }
                    return;
                }

                if self.loaded_path.is_none() {
                    use crate::components::welcome::{WelcomeEvent, WelcomePanel};
                    let welcome_events = WelcomePanel::render(ui, props.recent_files, props.colors);
                    for evt in welcome_events {
                        match evt {
                            WelcomeEvent::OpenFilePicker => {
                                events.push(CentralPanelEvent::OpenFilePicker)
                            }
                            WelcomeEvent::OpenRecentFile(path) => {
                                events.push(CentralPanelEvent::OpenRecentFile(path))
                            }
                        }
                    }
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

    /// Navigate to a specific JSON path (for navigation history)
    pub fn navigate_to_path(&mut self, path: String) {
        self.file_viewer.navigate_to_path(path);
    }

    /// Get the currently selected path (for navigation history tracking)
    pub fn get_selected_path(&self) -> Option<&String> {
        self.file_viewer.get_selected_path()
    }
}
