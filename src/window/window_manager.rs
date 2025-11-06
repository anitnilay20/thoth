use eframe::egui;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};

use crate::state::{SharedState, WindowState};

/// Manages multiple independent windows
pub struct WindowManager {
    /// Shared state across all windows
    pub shared_state: SharedState,

    /// Other windows (keyed by viewport ID)
    pub windows: HashMap<egui::ViewportId, Arc<Mutex<WindowState>>>,

    /// Counter for generating unique window IDs
    next_window_id: usize,

    /// Channel to receive close requests from windows
    close_sender: Sender<egui::ViewportId>,
    close_receiver: Receiver<egui::ViewportId>,
}

impl WindowManager {
    pub fn new(shared_state: SharedState) -> Self {
        let (close_sender, close_receiver) = channel();
        Self {
            shared_state,
            windows: HashMap::new(),
            next_window_id: 1,
            close_sender,
            close_receiver,
        }
    }

    /// Request to create a new window
    pub fn request_new_window(&mut self) {
        let window_id = self.next_window_id;
        self.next_window_id += 1;

        let viewport_id = egui::ViewportId::from_hash_of(format!("thoth_window_{}", window_id));

        // Create new window state
        let window_state = Arc::new(Mutex::new(WindowState::default()));
        self.windows.insert(viewport_id, window_state);
    }

    /// Show all open windows - MUST be called every frame
    pub fn show_windows(&mut self, ctx: &egui::Context) {
        let settings = self.shared_state.settings.lock().unwrap().clone();

        for (viewport_id, window_state) in &self.windows {
            let window_state = Arc::clone(window_state);
            let shared_state = self.shared_state.clone();
            let close_sender = self.close_sender.clone();
            let viewport_id_copy = *viewport_id;

            ctx.show_viewport_deferred(
                *viewport_id,
                egui::ViewportBuilder::default()
                    .with_title("Thoth — JSON & NDJSON Viewer")
                    .with_inner_size([
                        settings.window.default_width,
                        settings.window.default_height,
                    ])
                    .with_close_button(true),
                move |ctx, _class| {
                    // This closure runs every frame for the window
                    Self::render_window(
                        ctx,
                        &window_state,
                        &shared_state,
                        &close_sender,
                        viewport_id_copy,
                    );
                },
            );
        }

        // Clean up closed windows
        self.cleanup_closed_windows();
    }

    /// Render a single window
    fn render_window(
        ctx: &egui::Context,
        window_state: &Arc<Mutex<WindowState>>,
        shared_state: &SharedState,
        close_sender: &Sender<egui::ViewportId>,
        viewport_id: egui::ViewportId,
    ) {
        // Check if close was requested
        if ctx.input(|i| i.viewport().close_requested()) {
            // Send close request via channel
            let _ = close_sender.send(viewport_id);
            return;
        }

        // Apply theme settings
        let settings = shared_state.settings.lock().unwrap().clone();
        crate::theme::apply_theme(ctx, &settings);

        let state = window_state.lock().unwrap();

        // Render the window content
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Independent Thoth Window");
            ui.separator();
            ui.label("Multi-window support is active!");
            ui.label("Each window can open different files independently.");

            // Show file info if available
            if let Some(path) = &state.file_path {
                ui.separator();
                ui.label(format!("File: {}", path.display()));
            } else {
                ui.separator();
                ui.label("No file loaded in this window.");
                ui.label("Drag & drop or use the main window to open files.");
            }
        });
    }

    /// Remove closed windows from our tracking
    fn cleanup_closed_windows(&mut self) {
        // Collect all close requests from the channel
        while let Ok(viewport_id) = self.close_receiver.try_recv() {
            self.windows.remove(&viewport_id);
        }
    }

    // Get number of open windows
    // pub fn window_count(&self) -> usize {
    //     self.windows.len()
    // }
}
