use std::path::PathBuf;

use anyhow::Result;
use eframe::{
    App, Frame, NativeOptions,
    egui::{self},
};

use crate::{components::theme, helpers::load_icon};

mod components;
mod file;
mod helpers;
mod search;

#[derive(Default)]
struct ThothApp {
    top_bar: components::top_bar::TopBar,
    central_panel: components::central_panel::CentralPanel,
    error: Option<String>,
    file_path: Option<PathBuf>,
    file_type: file::lazy_loader::FileType,

    // search engine state
    search: search::Search,
    search_rx: Option<std::sync::mpsc::Receiver<search::Search>>,

    // UI
    dark_mode: bool,
}

impl App for ThothApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        self.dark_mode = ctx.style().visuals.dark_mode;

        self.handle_file_drop(ctx);
        
        if let Some(path) = &self.file_path {
            let file_name = std::path::Path::new(path)
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown file");
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(format!(
                "Thoth — {}",
                file_name
            )));
        } else {
            ctx.send_viewport_cmd(egui::ViewportCommand::Title(
                "Thoth — JSON & NDJSON Viewer".to_owned(),
            ));
        }

        // Get user's action from TopBar (open file / change type / search / stop)
        let incoming_msg = self.top_bar.ui(
            ctx,
            &mut self.file_path,
            &mut self.file_type,
            &mut self.error,
            &mut self.dark_mode,
        );

        // We will forward a processed message (with results) to the CentralPanel
        let mut msg_to_central: Option<search::SearchMessage> = None;

        if let Some(rx) = &self.search_rx {
            if let Ok(done) = rx.try_recv() {
                self.search = done.clone(); // finished: scanning=false, results filled
                msg_to_central = Some(search::SearchMessage::StartSearch(done));
                self.search_rx = None; // finished
            }
        }

        if let Some(msg) = incoming_msg {
            match msg {
                search::SearchMessage::StartSearch(s) => {
                    // kick off background
                    self.search = s.clone();
                    self.search.scanning = true;

                    // tell CentralPanel to show loader NOW
                    msg_to_central = Some(search::SearchMessage::StartSearch(self.search.clone()));

                    // spawn and keep receiver
                    self.search_rx =
                        Some(self.search.start_scanning(&self.file_path, &self.file_type));

                    // keep UI repainting while scanning
                    ctx.request_repaint();
                }
                search::SearchMessage::StopSearch => {
                    self.search_rx = None; // optional: drop pending result
                    msg_to_central = Some(search::SearchMessage::StopSearch);
                }
            }
        }

        theme::apply_theme(ctx, self.dark_mode); // Always dark mode

        // Render the central panel, passing the processed search message (if any)
        self.central_panel.ui(
            ctx,
            &self.file_path,
            &mut self.file_type,
            &mut self.error,
            msg_to_central,
        );
    }
}

fn main() -> Result<()> {
    let icon = load_icon(include_bytes!("../assets/thoth_icon_256.png"));
    let options = NativeOptions {
        viewport: egui::ViewportBuilder::default().with_icon(icon),
        ..Default::default()
    };

    if let Err(e) = eframe::run_native(
        "Thoth — JSON & NDJSON Viewer",
        options,
        Box::new(|_cc| Ok(Box::new(ThothApp::default()))),
    ) {
        eprintln!("Error running application: {e:?}");
        return Err(anyhow::anyhow!("Failed to run application"));
    }
    Ok(())
}
