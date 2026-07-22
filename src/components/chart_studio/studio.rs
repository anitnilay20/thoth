//! The Chart Studio **config panel** rendered in the sidebar content area.
//!
//! Holds the current selection (source, type, axes, options) plus the
//! app-injected producer list, resolved column schema, and open-chart list.
//! Emits [`ChartStudioEvent`]s up to the app, which does the data fetching and
//! tab creation.
//!
//! Built entirely from `thoth-plugin-sdk` components (Select, ToggleSwitch,
//! Button, IconButton, Icon, List, Typography, SidebarHeader) so it matches the
//! rest of the app's styling.

use eframe::egui;
use thoth_plugin_sdk::components::{
    Button, ButtonColor, ButtonType, Icon, IconButton, List, ListEvent, ListItem, ListItemPrefix,
    NumberInput, Select, SelectOption, SidebarHeader, Size, ToggleSwitch, Typography,
    TypographyVariant,
};
use thoth_plugin_sdk::theme::color_to_hex;

use super::{
    Aggregation, ChartOptions, ChartSpec, ChartType, ColumnInfo, ProducerKind, ProducerRef,
    SortMode, series_palette,
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
    aggregation: Aggregation,
    top_n: usize,
    sort: SortMode,
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
        self.aggregation = spec.aggregation;
        self.top_n = spec.top_n;
        self.sort = spec.sort;
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
        egui::Frame::new()
            .inner_margin(egui::Margin {
                left: PAD_X as i8,
                right: PAD_X as i8,
                top: 4,
                bottom: 8,
            })
            .show(ui, |ui| {
                ui.spacing_mut().item_spacing = egui::vec2(6.0, 8.0);
                // Fit the panel width exactly — no horizontal scrolling.
                let width = ui.available_width();

                self.data_source_section(ui, &colors, width, &mut events);
                ui.add_space(6.0);
                self.chart_type_section(ui, width);

                if !self.columns.is_empty() {
                    ui.add_space(6.0);
                    self.axes_section(ui, &colors, width);
                    ui.add_space(6.0);
                    self.data_section(ui, width);
                    ui.add_space(6.0);
                    self.options_section(ui);
                }

                ui.add_space(10.0);
                self.generate_button(ui, width, &mut events);

                if !self.open_charts.is_empty() {
                    ui.add_space(14.0);
                    self.open_charts_section(ui, &mut events);
                }
            });

        events
    }

    fn group_label(ui: &mut egui::Ui, text: &str) {
        ui.add_space(2.0);
        ui.add(
            Typography::builder()
                .text(text)
                .variant(TypographyVariant::GroupLabel)
                .build(),
        );
        ui.add_space(2.0);
    }

    fn field_label(ui: &mut egui::Ui, text: &str) {
        ui.add(
            Typography::builder()
                .text(text)
                .variant(TypographyVariant::Label)
                .build(),
        );
    }

    fn data_source_section(
        &mut self,
        ui: &mut egui::Ui,
        colors: &ThemeColors,
        width: f32,
        events: &mut Vec<ChartStudioEvent>,
    ) {
        Self::group_label(ui, "DATA SOURCE");
        if self.producers.is_empty() {
            ui.add(
                Typography::builder()
                    .text("No open data sources. Open a file or a producer plugin.")
                    .variant(TypographyVariant::BodyMuted)
                    .build(),
            );
            return;
        }
        // Flat option list, files first then plugins, each prefixed with a kind
        // glyph (Select has no option groups).
        let mut options = Vec::new();
        for kind in [ProducerKind::File, ProducerKind::Plugin] {
            let glyph = match kind {
                ProducerKind::File => egui_phosphor::regular::FILE,
                ProducerKind::Plugin => egui_phosphor::regular::PLUG,
            };
            for p in self.producers.iter().filter(|p| p.kind == kind) {
                options.push(
                    SelectOption::builder()
                        .value(p.tab_id.to_string())
                        .label(format!("{glyph}  {}", p.label))
                        .build(),
                );
            }
        }
        let _ = colors;
        let mut select = Select::builder()
            .id("chart_ds")
            .value(self.selected.map(|t| t.to_string()).unwrap_or_default())
            .options(options)
            .width(width)
            .size(Size::Medium)
            .build();
        if let Some(v) = select.show(ui).inner.selected
            && let Ok(tab) = v.parse::<TabId>()
            && self.selected != Some(tab)
        {
            self.selected = Some(tab);
            events.push(ChartStudioEvent::SelectSource(tab));
        }
    }

    fn chart_type_section(&mut self, ui: &mut egui::Ui, width: f32) {
        Self::group_label(ui, "CHART TYPE");
        let spacing = 5.0;
        let cols = 4;
        // Never exceed the row width (avoids horizontal overflow on narrow panels).
        let cell = ((width - spacing * (cols as f32 - 1.0)) / cols as f32).clamp(1.0, 84.0);
        let prev = ui.spacing().item_spacing;
        ui.spacing_mut().item_spacing = egui::vec2(spacing, spacing);
        for chunk in ChartType::ALL.chunks(cols) {
            ui.horizontal(|ui| {
                for &ct in chunk {
                    let clicked = ui
                        .add(
                            IconButton::builder()
                                .icon(ct.icon())
                                .tooltip(ct.label())
                                .frame(true)
                                .selected(self.chart_type == ct)
                                .size_px(cell)
                                .icon_size(22.0)
                                .build(),
                        )
                        .clicked();
                    if clicked {
                        self.chart_type = ct;
                    }
                }
            });
        }
        ui.spacing_mut().item_spacing = prev;
    }

    fn axes_section(&mut self, ui: &mut egui::Ui, colors: &ThemeColors, width: f32) {
        Self::group_label(ui, "AXES");
        let names: Vec<String> = self.columns.iter().map(|c| c.name.clone()).collect();
        let col_options: Vec<SelectOption> = names
            .iter()
            .enumerate()
            .map(|(i, n)| {
                SelectOption::builder()
                    .value(i.to_string())
                    .label(n)
                    .build()
            })
            .collect();

        Self::field_label(ui, "X Axis");
        self.x_col = self.x_col.min(names.len().saturating_sub(1));
        let mut x_select = Select::builder()
            .id("chart_x")
            .value(self.x_col.to_string())
            .options(col_options)
            .width(width)
            .size(Size::Medium)
            .build();
        if let Some(v) = x_select.show(ui).inner.selected
            && let Ok(i) = v.parse::<usize>()
        {
            self.x_col = i;
        }

        ui.add_space(6.0);
        let y_label = if self.chart_type.single_series() {
            "Value"
        } else {
            "Y Series"
        };
        Self::field_label(ui, y_label);

        let numeric = self.numeric_cols();
        let numeric_options: Vec<SelectOption> = numeric
            .iter()
            .map(|&ni| {
                SelectOption::builder()
                    .value(ni.to_string())
                    .label(&names[ni])
                    .build()
            })
            .collect();
        let palette = series_palette(colors);
        if self.chart_type.single_series() {
            self.y_cols.truncate(1);
            if self.y_cols.is_empty() {
                self.y_cols.push(*numeric.first().unwrap_or(&0));
            }
        }

        let multi = !self.chart_type.single_series() && self.y_cols.len() > 1;
        // Leave room for the colour swatch (and the remove button when multi).
        let combo_w = if multi { width - 52.0 } else { width - 22.0 };
        let mut remove: Option<usize> = None;
        for i in 0..self.y_cols.len() {
            ui.horizontal(|ui| {
                ui.add(
                    Icon::builder()
                        .glyph(egui_phosphor::regular::SQUARE)
                        .color(color_to_hex(palette[i % palette.len()]))
                        .size(12.0)
                        .build(),
                );
                let mut y_select = Select::builder()
                    .id(format!("chart_y_{i}"))
                    .value(self.y_cols[i].to_string())
                    .options(numeric_options.clone())
                    .width(combo_w)
                    .size(Size::Medium)
                    .build();
                if let Some(v) = y_select.show(ui).inner.selected
                    && let Ok(c) = v.parse::<usize>()
                {
                    self.y_cols[i] = c;
                }
                if multi
                    && ui
                        .add(
                            IconButton::builder()
                                .icon(egui_phosphor::regular::X)
                                .tooltip("Remove series")
                                .build(),
                        )
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
                .add(
                    Button::builder()
                        .label("Add series")
                        .icon(egui_phosphor::regular::PLUS)
                        .button_type(ButtonType::Text)
                        .color(ButtonColor::Secondary)
                        .size(11.0)
                        .build(),
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

    fn data_section(&mut self, ui: &mut egui::Ui, width: f32) {
        Self::group_label(ui, "TRANSFORM");

        Self::field_label(ui, "Aggregate");
        let agg_opts: Vec<SelectOption> = Aggregation::ALL
            .iter()
            .enumerate()
            .map(|(i, a)| {
                SelectOption::builder()
                    .value(i.to_string())
                    .label(a.label())
                    .build()
            })
            .collect();
        let cur = Aggregation::ALL
            .iter()
            .position(|a| *a == self.aggregation)
            .unwrap_or(0);
        let mut agg_sel = Select::builder()
            .id("chart_agg")
            .value(cur.to_string())
            .options(agg_opts)
            .width(width)
            .size(Size::Medium)
            .build();
        if let Some(v) = agg_sel.show(ui).inner.selected
            && let Ok(i) = v.parse::<usize>()
            && let Some(a) = Aggregation::ALL.get(i)
        {
            self.aggregation = *a;
        }

        ui.add_space(6.0);
        Self::field_label(ui, "Sort");
        let sort_opts: Vec<SelectOption> = SortMode::ALL
            .iter()
            .enumerate()
            .map(|(i, s)| {
                SelectOption::builder()
                    .value(i.to_string())
                    .label(s.label())
                    .build()
            })
            .collect();
        let cur = SortMode::ALL
            .iter()
            .position(|s| *s == self.sort)
            .unwrap_or(0);
        let mut sort_sel = Select::builder()
            .id("chart_sort")
            .value(cur.to_string())
            .options(sort_opts)
            .width(width)
            .size(Size::Medium)
            .build();
        if let Some(v) = sort_sel.show(ui).inner.selected
            && let Ok(i) = v.parse::<usize>()
            && let Some(s) = SortMode::ALL.get(i)
        {
            self.sort = *s;
        }

        ui.add_space(6.0);
        Self::field_label(ui, "Top N (0 = all)");
        let mut top = NumberInput::builder()
            .id("chart_topn")
            .value(self.top_n as f64)
            .min(0.0)
            .max(1000.0)
            .build();
        top.show(ui);
        self.top_n = top.value.max(0.0) as usize;
    }

    fn options_section(&mut self, ui: &mut egui::Ui) {
        Self::group_label(ui, "OPTIONS");
        let rows = [
            ("Show legend", self.options.legend),
            ("Show gridlines", self.options.grid),
            ("Smooth curves", self.options.smooth),
            ("Stacked", self.options.stacked),
        ];
        let mut toggled = [false; 4];
        for (i, (label, enabled)) in rows.iter().enumerate() {
            ui.horizontal(|ui| {
                if ui
                    .add(
                        ToggleSwitch::builder()
                            .id(format!("chart_opt_{i}"))
                            .enabled(*enabled)
                            .build(),
                    )
                    .clicked()
                {
                    toggled[i] = true;
                }
                ui.add_space(4.0);
                Self::field_label(ui, label);
            });
        }
        if toggled[0] {
            self.options.legend = !self.options.legend;
        }
        if toggled[1] {
            self.options.grid = !self.options.grid;
        }
        if toggled[2] {
            self.options.smooth = !self.options.smooth;
        }
        if toggled[3] {
            self.options.stacked = !self.options.stacked;
        }
    }

    fn generate_button(
        &mut self,
        ui: &mut egui::Ui,
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
        let clicked = ui
            .add_enabled(
                ready,
                Button::builder()
                    .id("chart_generate")
                    .label(label)
                    .icon(icon)
                    .button_type(ButtonType::Elevated)
                    .color(ButtonColor::Secondary)
                    .width(width)
                    .height(30.0)
                    .build(),
            )
            .clicked();
        if clicked
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
                aggregation: self.aggregation,
                top_n: self.top_n,
                sort: self.sort,
                edit_target: self.editing,
            }));
            self.editing = None;
        }
        if editing
            && ui
                .add(
                    Button::builder()
                        .label("Cancel edit")
                        .button_type(ButtonType::Text)
                        .size(11.0)
                        .build(),
                )
                .clicked()
        {
            self.editing = None;
        }
    }

    fn open_charts_section(&mut self, ui: &mut egui::Ui, events: &mut Vec<ChartStudioEvent>) {
        Self::group_label(ui, "OPEN CHARTS");
        let items: Vec<ListItem> = self
            .open_charts
            .iter()
            .map(|(_, title)| {
                ListItem::builder()
                    .title(title.clone())
                    .prefix(ListItemPrefix::Icon {
                        glyph: egui_phosphor::regular::CHART_LINE.to_string(),
                        color: None,
                    })
                    .build()
            })
            .collect();
        if let Some(ListEvent::ItemClicked(i)) = List::builder().items(items).build().show(ui)
            && let Some((id, _)) = self.open_charts.get(i)
        {
            events.push(ChartStudioEvent::FocusChart(*id));
        }
    }
}
