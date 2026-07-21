//! Built-in chart view (#133) — a host-rendered consumer of the dataset bus
//! (#113). Requests a dataset from an open producer tab (via the consent
//! picker the app drives), then renders it as a table or an `egui_plot` chart.
//!
//! The chart reads the host-owned copy by handle (`plugin::datasets::read`) —
//! it never holds a second copy of the data.

use eframe::egui;
use egui_plot::{Bar, BarChart, Line, Plot, PlotPoints, Points};

use crate::app::tab_manager::TabId;

/// Rows fetched for preview / plotting.
const VIEW_ROWS: u32 = 5000;

#[derive(Clone, Copy, PartialEq, Default)]
pub enum ChartKind {
    #[default]
    Line,
    Bar,
    Scatter,
}

#[derive(Clone, Copy, PartialEq, Default)]
pub enum Mode {
    Table,
    #[default]
    Chart,
}

/// What the chart window is asking the app to do this frame.
pub enum ChartAction {
    /// Fetch the dataset from this producer tab.
    Pick(TabId),
    /// Re-open the producer picker.
    ChangeSource,
}

#[derive(Default)]
pub struct ChartWindow {
    pub open: bool,
    /// Producer tabs offered in the picker (set by the app when opening).
    pub producers: Vec<(TabId, String)>,
    /// Registry handle of the fetched dataset (None ⇒ show the picker).
    handle: Option<String>,
    source_name: String,
    x_col: usize,
    y_col: usize,
    kind: ChartKind,
    mode: Mode,
}

impl ChartWindow {
    /// Open the picker with the given producer tabs.
    pub fn open_picker(&mut self, producers: Vec<(TabId, String)>) {
        self.open = true;
        self.handle = None;
        self.producers = producers;
    }

    /// Bind a fetched dataset (called by the app after routing to a producer).
    pub fn set_dataset(&mut self, handle: String, name: String, col_count: usize) {
        self.handle = Some(handle);
        self.source_name = name;
        self.x_col = 0;
        self.y_col = if col_count > 1 { 1 } else { 0 };
    }

    /// Render the window body; returns an action for the app to service.
    pub fn render(&mut self, ui: &mut egui::Ui) -> Option<ChartAction> {
        let mut action = None;

        // Picker: no dataset bound yet.
        let Some(handle) = self.handle.clone() else {
            ui.label(egui::RichText::new("Pick a data source").strong());
            ui.add_space(4.0);
            if self.producers.is_empty() {
                ui.label(
                    egui::RichText::new("No open producer tabs. Open a data-source tab (e.g. run a Seshat query) and try again.")
                        .weak(),
                );
            }
            for (id, label) in &self.producers {
                if ui.button(label).clicked() {
                    action = Some(ChartAction::Pick(*id));
                }
            }
            return action;
        };

        let Some(page) = crate::plugin::datasets::read(&handle, 0, VIEW_ROWS) else {
            ui.label(egui::RichText::new("This dataset is no longer available.").weak());
            if ui.button("Pick another source").clicked() {
                action = Some(ChartAction::ChangeSource);
            }
            return action;
        };

        // Header: source + controls.
        ui.horizontal(|ui| {
            ui.label(egui::RichText::new(&self.source_name).strong());
            ui.label(egui::RichText::new(format!("· {} rows", page.total)).weak());
            if ui.button("Change source").clicked() {
                action = Some(ChartAction::ChangeSource);
            }
            ui.separator();
            ui.selectable_value(&mut self.mode, Mode::Chart, "Chart");
            ui.selectable_value(&mut self.mode, Mode::Table, "Table");
        });
        ui.separator();

        let names: Vec<&str> = page.columns.iter().map(|c| c.name.as_str()).collect();
        if names.is_empty() {
            ui.label(egui::RichText::new("Dataset has no columns.").weak());
            return action;
        }
        self.x_col = self.x_col.min(names.len() - 1);
        self.y_col = self.y_col.min(names.len() - 1);

        match self.mode {
            Mode::Table => render_table(ui, &page),
            Mode::Chart => {
                ui.horizontal(|ui| {
                    egui::ComboBox::from_label("X")
                        .selected_text(names[self.x_col])
                        .show_ui(ui, |ui| {
                            for (i, n) in names.iter().enumerate() {
                                ui.selectable_value(&mut self.x_col, i, *n);
                            }
                        });
                    egui::ComboBox::from_label("Y")
                        .selected_text(names[self.y_col])
                        .show_ui(ui, |ui| {
                            for (i, n) in names.iter().enumerate() {
                                ui.selectable_value(&mut self.y_col, i, *n);
                            }
                        });
                    ui.separator();
                    ui.selectable_value(&mut self.kind, ChartKind::Line, "Line");
                    ui.selectable_value(&mut self.kind, ChartKind::Bar, "Bar");
                    ui.selectable_value(&mut self.kind, ChartKind::Scatter, "Scatter");
                });
                self.render_chart(ui, &page);
            }
        }
        action
    }

    fn render_chart(&self, ui: &mut egui::Ui, page: &crate::plugin::datasets::Page) {
        // (x, y) points: x from the X column when numeric, else the row index;
        // rows whose Y isn't numeric are skipped.
        let points: Vec<[f64; 2]> = page
            .rows
            .iter()
            .enumerate()
            .filter_map(|(i, row)| {
                let y = row.get(self.y_col)?.trim().parse::<f64>().ok()?;
                let x = row
                    .get(self.x_col)
                    .and_then(|c| c.trim().parse::<f64>().ok())
                    .unwrap_or(i as f64);
                Some([x, y])
            })
            .collect();

        if points.is_empty() {
            ui.label(
                egui::RichText::new("Selected Y column has no numeric values to plot.").weak(),
            );
            return;
        }

        let y_name = page
            .columns
            .get(self.y_col)
            .map(|c| c.name.clone())
            .unwrap_or_else(|| "y".to_string());
        Plot::new("dataset_chart")
            .height(ui.available_height().max(120.0))
            .show(ui, |plot_ui| match self.kind {
                ChartKind::Line => {
                    plot_ui.line(Line::new(y_name.clone(), PlotPoints::from(points)))
                }
                ChartKind::Scatter => plot_ui
                    .points(Points::new(y_name.clone(), PlotPoints::from(points)).radius(2.5)),
                ChartKind::Bar => {
                    let bars: Vec<Bar> = points.iter().map(|p| Bar::new(p[0], p[1])).collect();
                    plot_ui.bar_chart(BarChart::new(y_name.clone(), bars));
                }
            });
    }
}

/// A compact scrollable preview table (first `VIEW_ROWS`).
fn render_table(ui: &mut egui::Ui, page: &crate::plugin::datasets::Page) {
    egui::ScrollArea::both().show(ui, |ui| {
        egui::Grid::new("dataset_table")
            .striped(true)
            .show(ui, |ui| {
                for c in &page.columns {
                    ui.label(egui::RichText::new(&c.name).strong());
                }
                ui.end_row();
                for row in &page.rows {
                    for cell in row {
                        ui.label(cell);
                    }
                    ui.end_row();
                }
            });
    });
}
