//! The Chart Studio **config panel** rendered in the sidebar content area.
//!
//! Holds the current selection (source, type, axes, options) plus the
//! app-injected producer list, resolved column schema, and open-chart list.
//! Emits [`ChartStudioEvent`]s up to the app, which does the data fetching and
//! tab creation.

use eframe::egui::{self, Align2, FontId, RichText, Sense, Stroke, StrokeKind, Vec2};
use thoth_plugin_sdk::components::{SidebarHeader, Typography, TypographyVariant};

use super::{
    ChartOptions, ChartSpec, ChartType, ColumnInfo, ProducerKind, ProducerRef, series_palette,
};
use crate::app::tab_manager::TabId;
use crate::theme::ThemeColors;

/// Horizontal inset, matching `SidebarHeader`/list rows.
const PAD_X: f32 = 8.0;

/// What the config panel is asking the app to do.
pub enum ChartStudioEvent {
    /// The user picked a data source; the app should resolve its columns and
    /// feed them back via [`ChartStudio::set_columns`].
    SelectSource(TabId),
    /// Build (or update) a chart tab from this spec.
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
    /// When `Some`, the panel is editing this existing chart tab (Generate
    /// updates it in place and reads "Update Chart").
    editing: Option<TabId>,
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

    /// Load an existing chart's spec + columns for editing.
    pub fn edit(&mut self, spec: ChartSpec, columns: Vec<ColumnInfo>) {
        self.selected = Some(spec.source_tab);
        self.chart_type = spec.chart_type;
        self.x_col = spec.x_col;
        self.y_cols = if spec.y_cols.is_empty() {
            vec![0]
        } else {
            spec.y_cols
        };
        self.options = spec.options;
        self.columns = columns;
        self.editing = spec.edit_target;
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

        // Flush section header (aligns with other sidebar panels).
        ui.add(SidebarHeader::builder().title("CHART STUDIO").build());

        // Everything else is inset to match the list rows' left padding.
        let width = (ui.clip_rect().width() - 2.0 * PAD_X).max(120.0);
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: PAD_X as i8,
                right: PAD_X as i8,
                top: 4,
                bottom: 8,
            })
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 8.0);

                self.data_source_section(ui, &colors, width, &mut events);
                ui.add_space(6.0);
                self.chart_type_section(ui, &colors, width);

                if !self.columns.is_empty() {
                    ui.add_space(6.0);
                    self.axes_section(ui, &colors, width);
                    ui.add_space(6.0);
                    self.options_section(ui, &colors);
                }

                ui.add_space(10.0);
                self.generate_button(ui, &colors, width, &mut events);

                if !self.open_charts.is_empty() {
                    ui.add_space(14.0);
                    self.open_charts_section(ui, &colors, &mut events);
                }
            });

        events
    }

    fn subsection_label(ui: &mut egui::Ui, text: &str) {
        ui.add_space(2.0);
        ui.add(
            Typography::builder()
                .text(text)
                .variant(TypographyVariant::PanelHeader)
                .build(),
        );
        ui.add_space(2.0);
    }

    fn data_source_section(
        &mut self,
        ui: &mut egui::Ui,
        colors: &ThemeColors,
        width: f32,
        events: &mut Vec<ChartStudioEvent>,
    ) {
        Self::subsection_label(ui, "DATA SOURCE");
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
            .width(width)
            .show_ui(ui, |ui| {
                ui.set_min_width(width);
                for kind in [ProducerKind::File, ProducerKind::Plugin] {
                    let group: Vec<(TabId, String)> = self
                        .producers
                        .iter()
                        .filter(|p| p.kind == kind)
                        .map(|p| (p.tab_id, p.label.clone()))
                        .collect();
                    if group.is_empty() {
                        continue;
                    }
                    let heading = match kind {
                        ProducerKind::File => "Files",
                        ProducerKind::Plugin => "Plugins",
                    };
                    ui.label(RichText::new(heading).color(colors.fg_muted).size(10.0));
                    for (id, label) in group {
                        let is_sel = self.selected == Some(id);
                        if ui.selectable_label(is_sel, label).clicked() && !is_sel {
                            self.selected = Some(id);
                            events.push(ChartStudioEvent::SelectSource(id));
                        }
                    }
                }
            });
    }

    fn chart_type_section(&mut self, ui: &mut egui::Ui, colors: &ThemeColors, width: f32) {
        Self::subsection_label(ui, "CHART TYPE");
        let spacing = 5.0;
        let cols = 4;
        let cell = ((width - spacing * (cols as f32 - 1.0)) / cols as f32).clamp(42.0, 84.0);
        let prev = ui.spacing().item_spacing;
        ui.spacing_mut().item_spacing = egui::vec2(spacing, spacing);
        for chunk in ChartType::ALL.chunks(cols) {
            ui.horizontal(|ui| {
                for &ct in chunk {
                    self.chart_type_cell(ui, colors, ct, cell);
                }
            });
        }
        ui.spacing_mut().item_spacing = prev;
    }

    /// A single chart-type button: a large icon over a small label, painted so
    /// the glyph reads clearly (icon-button sized).
    fn chart_type_cell(
        &mut self,
        ui: &mut egui::Ui,
        colors: &ThemeColors,
        ct: ChartType,
        cell: f32,
    ) {
        let selected = self.chart_type == ct;
        let (rect, resp) = ui.allocate_exact_size(Vec2::splat(cell), Sense::click());
        let hovered = resp.hovered();
        let (fill, stroke_c, fg) = if selected {
            (colors.surface_active, colors.accent, colors.accent)
        } else if hovered {
            (colors.surface_raised, colors.accent_secondary, colors.fg)
        } else {
            (colors.surface, colors.surface_raised, colors.fg_muted)
        };
        let painter = ui.painter();
        painter.rect(
            rect,
            4.0,
            fill,
            Stroke::new(1.0, stroke_c),
            StrokeKind::Inside,
        );
        painter.text(
            rect.center() - Vec2::new(0.0, 9.0),
            Align2::CENTER_CENTER,
            ct.icon(),
            FontId::proportional(22.0),
            fg,
        );
        painter.text(
            rect.center() + Vec2::new(0.0, 15.0),
            Align2::CENTER_CENTER,
            ct.label(),
            FontId::proportional(9.5),
            fg,
        );
        if resp.clicked() {
            self.chart_type = ct;
        }
    }

    fn axes_section(&mut self, ui: &mut egui::Ui, colors: &ThemeColors, width: f32) {
        Self::subsection_label(ui, "AXES");
        let names: Vec<String> = self.columns.iter().map(|c| c.name.clone()).collect();

        ui.label(RichText::new("X Axis").color(colors.fg_muted).size(10.0));
        self.x_col = self.x_col.min(names.len().saturating_sub(1));
        egui::ComboBox::from_id_salt("chart_x")
            .selected_text(names.get(self.x_col).cloned().unwrap_or_default())
            .width(width)
            .show_ui(ui, |ui| {
                ui.set_min_width(width);
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
        if self.chart_type.single_series() {
            self.y_cols.truncate(1);
            if self.y_cols.is_empty() {
                self.y_cols.push(*numeric.first().unwrap_or(&0));
            }
        }

        let multi = !self.chart_type.single_series() && self.y_cols.len() > 1;
        let combo_w = if multi { width - 26.0 } else { width };
        let mut remove: Option<usize> = None;
        for i in 0..self.y_cols.len() {
            ui.horizontal(|ui| {
                let (sw, _) = ui.allocate_exact_size(Vec2::splat(10.0), Sense::hover());
                ui.painter()
                    .rect_filled(sw, 2.0, palette[i % palette.len()]);
                let mut sel = self.y_cols[i];
                egui::ComboBox::from_id_salt(("chart_y", i))
                    .selected_text(names.get(sel).cloned().unwrap_or_default())
                    .width(combo_w)
                    .show_ui(ui, |ui| {
                        ui.set_min_width(combo_w);
                        for &ni in &numeric {
                            ui.selectable_value(&mut sel, ni, &names[ni]);
                        }
                    });
                self.y_cols[i] = sel;
                if multi
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

    fn options_section(&mut self, ui: &mut egui::Ui, _colors: &ThemeColors) {
        Self::subsection_label(ui, "OPTIONS");
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
        width: f32,
        events: &mut Vec<ChartStudioEvent>,
    ) {
        let ready = self.selected.is_some() && !self.columns.is_empty() && !self.y_cols.is_empty();
        let editing = self.editing.is_some();
        let (label, icon) = if editing {
            ("Update Chart", egui_phosphor::regular::CHECK)
        } else {
            ("Generate Chart", egui_phosphor::regular::CHART_LINE)
        };
        let btn = egui::Button::new(
            RichText::new(format!("{icon}  {label}"))
                .color(colors.bg)
                .strong(),
        )
        .min_size(egui::vec2(width, 30.0))
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
                edit_target: self.editing,
            }));
            self.editing = None;
        }
        if editing
            && ui
                .add(
                    egui::Button::new(
                        RichText::new("Cancel edit")
                            .color(colors.fg_muted)
                            .size(11.0),
                    )
                    .frame(false),
                )
                .clicked()
        {
            self.editing = None;
        }
    }

    fn open_charts_section(
        &mut self,
        ui: &mut egui::Ui,
        colors: &ThemeColors,
        events: &mut Vec<ChartStudioEvent>,
    ) {
        Self::subsection_label(ui, "OPEN CHARTS");
        for (id, title) in &self.open_charts {
            let resp = ui.add(
                egui::Label::new(
                    RichText::new(format!("{}  {}", egui_phosphor::regular::CHART_LINE, title))
                        .color(colors.fg_muted)
                        .size(11.0),
                )
                .sense(Sense::click())
                .truncate(),
            );
            if resp.clicked() {
                events.push(ChartStudioEvent::FocusChart(*id));
            }
        }
    }
}
