use std::path::PathBuf;

use anyhow::Result;
use eframe::{
    App, Frame,
    egui::{self},
};

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

    // Own the engine here
    search: search::Search,
}

impl App for ThothApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
        // Window title
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
        let incoming_msg = self
            .top_bar
            .ui(ctx, &mut self.file_path, &mut self.file_type, &mut self.error);

        // We will forward a processed message (with results) to the CentralPanel
        let mut msg_to_central: Option<search::SearchMessage> = None;
        
        if let Some(msg) = incoming_msg {
            println!("TopBar message: {:?}", msg);
            match msg {
                // StartSearch carries query, match_case (and maybe empty results)
                search::SearchMessage::StartSearch(s) => {
                    // Run the engine HERE (parent owns side effects / file access)
                    self.search.query = s.query;
                    self.search.match_case = s.match_case;
                    self.search.start_scanning(&self.file_path, &self.file_type);

                    // Now forward a StartSearch with the *filled* results
                    msg_to_central = Some(search::SearchMessage::StartSearch(self.search.clone()));
                }
                search::SearchMessage::StopSearch => {
                    // Clear any filter in the viewer
                    msg_to_central = Some(search::SearchMessage::StopSearch);
                }
            }
        }

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
    let options = eframe::NativeOptions::default();
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
