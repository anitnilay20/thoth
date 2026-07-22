//! The Chart Studio **config panel** rendered in the sidebar content area.
//!
//! Holds the current selection (source, type, axes, options) plus the
//! app-injected producer list, resolved column schema, and open-chart list.
//! Emits [`ChartStudioEvent`]s up to the app, which does the data fetching and
//! tab creation.

use eframe::egui::{self, RichText};

use super::{
    ChartOptions, ChartSpec, ChartType, ColumnInfo, ProducerKind, ProducerRef, series_palette,
};
use crate::app::tab_manager::TabId;
use crate::theme::ThemeColors;

/// What the config panel is asking the app to do.
pub enum ChartStudioEvent {
    /// The user picked a data source; the app should resolve its columns and
    /// feed them back via [`ChartStudio::set_columns`].
    SelectSource(TabId),
    /// Build a chart tab from this spec.
    Generate(ChartSpec),
    /// Activate an already-open chart tab.
    FocusChart(TabId),
}

#[derive(Default)]
pub struct ChartStudio {
    /// Eligible producer tabs (injected by the app each frame).
    producers: Vec<ProducerRef>,
    /// The currently selected source tab, if any.
    selected: Option<TabId>,
    /// Column schema of the selected source (injected after a resolve).
    columns: Vec<ColumnInfo>,
    chart_type: ChartType,
    x_col: usize,
    y_cols: Vec<usize>,
    options: ChartOptions,
    /// Open chart tabs `(tab id, title)` for the "Open Charts" list.
    open_charts: Vec<(TabId, String)>,
}

impl ChartStudio {
    /// Refresh the eligible producer list (called by the app each frame).
    /// Drops the selection if the source tab has gone away.
    pub fn set_producers(&mut self, producers: Vec<ProducerRef>) {
        if let Some(sel) = self.selected
            && !producers.iter().any(|p| p.tab_id == sel)
        {
            self.selected = None;
            self.columns.clear();
        }
        self.producers = producers;
    }

    /// Feed the resolved column schema for the selected source. Resets the axis
    /// selection to sensible defaults (X = first column, Y = first numeric).
    pub fn set_columns(&mut self, columns: Vec<ColumnInfo>) {
        self.x_col = 0;
        let first_numeric = columns.iter().position(|c| c.numeric).unwrap_or(0);
        self.y_cols = vec![first_numeric];
        self.columns = columns;
    }

    /// Update the "Open Charts" list.
    pub fn set_open_charts(&mut self, open: Vec<(TabId, String)>) {
        self.open_charts = open;
    }

    fn numeric_cols(&self) -> Vec<usize> {
        self.columns
            .iter()
            .enumerate()
            .filter(|(_, c)| c.numeric)
            .map(|(i, _)| i)
            .collect()
    }

    pub fn render(&mut self, ui: &mut egui::Ui) -> Vec<ChartStudioEvent> {
        let colors = ThemeColors::from_ctx(ui.ctx());
        let mut events = Vec::new();
        ui.spacing_mut().item_spacing = egui::vec2(6.0, 8.0);
        ui.add_space(8.0);

        self.data_source_section(ui, &colors, &mut events);
        ui.add_space(6.0);
        self.chart_type_section(ui, &colors);

        if !self.columns.is_empty() {
            ui.add_space(6.0);
            self.axes_section(ui, &colors);
            ui.add_space(6.0);
            self.options_section(ui);
        }

        ui.add_space(10.0);
        self.generate_button(ui, &colors, &mut events);

        if !self.open_charts.is_empty() {
            ui.add_space(12.0);
            self.open_charts_section(ui, &colors, &mut events);
        }

        events
    }

    fn section_label(ui: &mut egui::Ui, colors: &ThemeColors, text: &str) {
        ui.add_space(2.0);
        ui.label(
            RichText::new(text.to_uppercase())
                .color(colors.sidebar_header)
                .size(10.0)
                .strong(),
        );
        ui.separator();
    }

    fn data_source_section(
        &mut self,
        ui: &mut egui::Ui,
        colors: &ThemeColors,
        events: &mut Vec<ChartStudioEvent>,
    ) {
        Self::section_label(ui, colors, "Data Source");
        if self.producers.is_empty() {
            ui.label(
                RichText::new("No open data sources. Open a file or a producer plugin.")
                    .color(colors.fg_muted)
                    .size(11.0),
            );
            return;
        }
        let selected_label = self
            .selected
            .and_then(|id| self.producers.iter().find(|p| p.tab_id == id))
            .map(|p| p.label.clone())
            .unwrap_or_else(|| "Select a source…".to_string());

        egui::ComboBox::from_id_salt("chart_ds")
            .selected_text(selected_label)
            .width(ui.available_width() - 4.0)
            .show_ui(ui, |ui| {
                for kind in [ProducerKind::File, ProducerKind::Plugin] {
                    let group: Vec<&ProducerRef> =
                        self.producers.iter().filter(|p| p.kind == kind).collect();
                    if group.is_empty() {
                        continue;
                    }
                    let heading = match kind {
                        ProducerKind::File => "Files",
                        ProducerKind::Plugin => "Plugins",
                    };
                    ui.label(RichText::new(heading).color(colors.fg_muted).size(10.0));
                    for p in group {
                        let is_sel = self.selected == Some(p.tab_id);
                        if ui.selectable_label(is_sel, &p.label).clicked() && !is_sel {
                            self.selected = Some(p.tab_id);
                            events.push(ChartStudioEvent::SelectSource(p.tab_id));
                        }
                    }
                }
            });
    }

    fn chart_type_section(&mut self, ui: &mut egui::Ui, colors: &ThemeColors) {
        Self::section_label(ui, colors, "Chart Type");
        let spacing = 5.0;
        let cols = 4;
        let cell = ((ui.available_width() - spacing * (cols as f32 - 1.0)) / cols as f32)
            .clamp(40.0, 80.0);
        let prev_spacing = ui.spacing().item_spacing;
        ui.spacing_mut().item_spacing = egui::vec2(spacing, spacing);
        for chunk in ChartType::ALL.chunks(cols) {
            ui.horizontal(|ui| {
                for &ct in chunk {
                    let selected = self.chart_type == ct;
                    let text = RichText::new(format!("{}\n{}", ct.icon(), ct.label()))
                        .size(10.0)
                        .color(if selected {
                            colors.accent
                        } else {
                            colors.fg_muted
                        });
                    let mut btn = egui::Button::new(text)
                        .min_size(egui::vec2(cell, cell))
                        .wrap();
                    btn = if selected {
                        btn.fill(colors.surface_active)
                            .stroke(egui::Stroke::new(1.0, colors.accent))
                    } else {
                        btn.fill(colors.surface)
                            .stroke(egui::Stroke::new(1.0, colors.surface_raised))
                    };
                    if ui.add(btn).clicked() {
                        self.chart_type = ct;
                    }
                }
            });
        }
        ui.spacing_mut().item_spacing = prev_spacing;
    }

    fn axes_section(&mut self, ui: &mut egui::Ui, colors: &ThemeColors) {
        Self::section_label(ui, colors, "Axes");
        let names: Vec<String> = self.columns.iter().map(|c| c.name.clone()).collect();

        ui.label(RichText::new("X Axis").color(colors.fg_muted).size(10.0));
        self.x_col = self.x_col.min(names.len().saturating_sub(1));
        egui::ComboBox::from_id_salt("chart_x")
            .selected_text(names.get(self.x_col).cloned().unwrap_or_default())
            .width(ui.available_width() - 4.0)
            .show_ui(ui, |ui| {
                for (i, n) in names.iter().enumerate() {
                    ui.selectable_value(&mut self.x_col, i, n);
                }
            });

        ui.add_space(6.0);
        let y_label = if self.chart_type.single_series() {
            "Value"
        } else {
            "Y Series"
        };
        ui.label(RichText::new(y_label).color(colors.fg_muted).size(10.0));

        let numeric = self.numeric_cols();
        let palette = series_palette(colors);
        // Radial / single-series types collapse to one Y row.
        if self.chart_type.single_series() {
            self.y_cols.truncate(1);
            if self.y_cols.is_empty() {
                self.y_cols.push(*numeric.first().unwrap_or(&0));
            }
        }

        let mut remove: Option<usize> = None;
        for i in 0..self.y_cols.len() {
            ui.horizontal(|ui| {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(10.0, 10.0), egui::Sense::hover());
                ui.painter()
                    .rect_filled(rect, 2.0, palette[i % palette.len()]);
                let mut sel = self.y_cols[i];
                egui::ComboBox::from_id_salt(("chart_y", i))
                    .selected_text(names.get(sel).cloned().unwrap_or_default())
                    .width(ui.available_width() - if self.y_cols.len() > 1 { 26.0 } else { 6.0 })
                    .show_ui(ui, |ui| {
                        for &ni in &numeric {
                            ui.selectable_value(&mut sel, ni, &names[ni]);
                        }
                    });
                self.y_cols[i] = sel;
                if self.y_cols.len() > 1
                    && ui
                        .button(RichText::new(egui_phosphor::regular::X).color(colors.fg_muted))
                        .clicked()
                {
                    remove = Some(i);
                }
            });
        }
        if let Some(i) = remove {
            self.y_cols.remove(i);
        }

        // "Add series" for the multi-series types (cap at palette size).
        if !self.chart_type.single_series()
            && self.y_cols.len() < palette.len()
            && !numeric.is_empty()
            && ui
                .button(
                    RichText::new(format!("{}  Add series", egui_phosphor::regular::PLUS))
                        .color(colors.accent_secondary)
                        .size(11.0),
                )
                .clicked()
        {
            let next = numeric
                .iter()
                .find(|c| !self.y_cols.contains(c))
                .copied()
                .unwrap_or(numeric[0]);
            self.y_cols.push(next);
        }
    }

    fn options_section(&mut self, ui: &mut egui::Ui) {
        let colors = ThemeColors::from_ctx(ui.ctx());
        Self::section_label(ui, &colors, "Options");
        let o = &mut self.options;
        ui.checkbox(&mut o.legend, RichText::new("Show legend").size(11.0));
        ui.checkbox(&mut o.grid, RichText::new("Show gridlines").size(11.0));
        ui.checkbox(&mut o.smooth, RichText::new("Smooth curves").size(11.0));
        ui.checkbox(&mut o.stacked, RichText::new("Stacked").size(11.0));
    }

    fn generate_button(
        &mut self,
        ui: &mut egui::Ui,
        colors: &ThemeColors,
        events: &mut Vec<ChartStudioEvent>,
    ) {
        let ready = self.selected.is_some() && !self.columns.is_empty() && !self.y_cols.is_empty();
        let btn = egui::Button::new(
            RichText::new(format!(
                "{}  Generate Chart",
                egui_phosphor::regular::CHART_LINE
            ))
            .color(colors.bg)
            .strong(),
        )
        .min_size(egui::vec2(ui.available_width() - 4.0, 30.0))
        .fill(if ready {
            colors.accent_secondary
        } else {
            colors.surface_raised
        });
        if ui.add_enabled(ready, btn).clicked()
            && let (Some(tab), Some(src)) = (
                self.selected,
                self.selected
                    .and_then(|id| self.producers.iter().find(|p| p.tab_id == id)),
            )
        {
            events.push(ChartStudioEvent::Generate(ChartSpec {
                source_tab: tab,
                source_label: src.label.clone(),
                chart_type: self.chart_type,
                x_col: self.x_col,
                y_cols: self.y_cols.clone(),
                options: self.options,
            }));
        }
    }

    fn open_charts_section(
        &mut self,
        ui: &mut egui::Ui,
        colors: &ThemeColors,
        events: &mut Vec<ChartStudioEvent>,
    ) {
        Self::section_label(ui, colors, "Open Charts");
        for (id, title) in &self.open_charts {
            let resp = ui.add(
                egui::Label::new(
                    RichText::new(format!("{}  {}", egui_phosphor::regular::CHART_LINE, title))
                        .color(colors.fg_muted)
                        .size(11.0),
                )
                .sense(egui::Sense::click())
                .truncate(),
            );
            if resp.clicked() {
                events.push(ChartStudioEvent::FocusChart(*id));
            }
        }
    }
}
