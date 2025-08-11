use std::path::PathBuf;

use anyhow::Result;
use eframe::{
    App, Frame,
    egui::{self},
};
use pprof::ProfilerGuard;

mod components;
mod load_file;

#[derive(Default)]
struct ThothApp {
    top_bar: components::top_bar::TopBar,
    central_panel: components::central_panel::CentralPanel,
    error: Option<String>,
    file_path: Option<PathBuf>,
    file_type: FileType,
    data: Vec<serde_json::Value>,
}

#[derive(PartialEq, Default, Debug, Clone)]
enum FileType {
    Json,
    #[default]
    Ndjson,
}

impl App for ThothApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut Frame) {
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

        self.top_bar.ui(
            ctx,
            &mut self.file_path,
            &mut self.file_type,
            &mut self.data,
            &mut self.error,
        );

        self.central_panel.ui(ctx, &self.data, &mut self.error);
    }
}

fn main() -> Result<()> {
    let guard = ProfilerGuard::new(100).unwrap();

    let options = eframe::NativeOptions::default();
    if let Err(e) = eframe::run_native(
        "Thoth — JSON & NDJSON Viewer",
        options,
        Box::new(|_cc| Ok(Box::new(ThothApp::default()))),
    ) {
        eprintln!("Error running application: {e:?}");
        // If you want to convert it to anyhow error without using the full error:
        return Err(anyhow::anyhow!("Failed to run application"));
    }

    if let Ok(report) = guard.report().build() {
        let file = std::fs::File::create("flamegraph.svg").unwrap();
        report.flamegraph(file).unwrap();
    }
    Ok(())
}
