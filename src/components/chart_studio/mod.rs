//! Chart Studio (#133 / epic #118) — a host-rendered charting consumer of the
//! dataset bus.
//!
//! The **config panel** ([`ChartStudio`]) lives in the sidebar: pick a data
//! source (any open producer tab), a chart type, X / Y fields, and options,
//! then **Generate** opens a [`ChartTab`] in the dock. Each chart tab owns a
//! *snapshot* of its data, so it survives the source tab closing.
//!
//! Charts render host-side: the cartesian types use `egui_plot`; the radial
//! types (pie/doughnut/polar/radar) and the heatmap use a custom `egui`
//! painter. Colours come from the active theme — a data-viz [`series_palette`]
//! derived from role tokens, so charts re-theme with the rest of the app.

mod chart_tab;
mod studio;
mod transform;

pub use chart_tab::ChartTab;
pub use studio::{ChartStudio, ChartStudioEvent};

use eframe::egui::Color32;
use serde::{Deserialize, Serialize};

use crate::app::tab_manager::TabId;
use crate::theme::ThemeColors;

/// The eleven chart types offered in the studio.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum ChartType {
    #[default]
    Bar,
    HBar,
    Line,
    Area,
    Scatter,
    Pie,
    Doughnut,
    Radar,
    PolarArea,
    Histogram,
    Heatmap,
}

impl ChartType {
    /// All types, in the order shown in the config grid.
    pub const ALL: [ChartType; 11] = [
        ChartType::Bar,
        ChartType::HBar,
        ChartType::Line,
        ChartType::Area,
        ChartType::Scatter,
        ChartType::Pie,
        ChartType::Doughnut,
        ChartType::Radar,
        ChartType::PolarArea,
        ChartType::Histogram,
        ChartType::Heatmap,
    ];

    pub fn label(self) -> &'static str {
        match self {
            ChartType::Bar => "Bar",
            ChartType::HBar => "H-Bar",
            ChartType::Line => "Line",
            ChartType::Area => "Area",
            ChartType::Scatter => "Scatter",
            ChartType::Pie => "Pie",
            ChartType::Doughnut => "Donut",
            ChartType::Radar => "Radar",
            ChartType::PolarArea => "Polar",
            ChartType::Histogram => "Histogram",
            ChartType::Heatmap => "Heatmap",
        }
    }

    /// Phosphor glyph shown on the type button.
    pub fn icon(self) -> &'static str {
        use egui_phosphor::regular as ph;
        match self {
            ChartType::Bar => ph::CHART_BAR,
            ChartType::HBar => ph::CHART_BAR_HORIZONTAL,
            ChartType::Line => ph::CHART_LINE,
            ChartType::Area => ph::CHART_LINE_UP,
            ChartType::Scatter => ph::CHART_SCATTER,
            ChartType::Pie => ph::CHART_PIE,
            ChartType::Doughnut => ph::CHART_DONUT,
            ChartType::Radar => ph::POLYGON,
            ChartType::PolarArea => ph::CHART_POLAR,
            ChartType::Histogram => ph::CHART_BAR,
            ChartType::Heatmap => ph::GRID_NINE,
        }
    }

    /// Types that plot a single Y field (the others accept multiple series).
    /// Heatmap is special-cased: it uses *all* numeric columns.
    pub fn single_series(self) -> bool {
        matches!(
            self,
            ChartType::Pie
                | ChartType::Doughnut
                | ChartType::PolarArea
                | ChartType::Histogram
                | ChartType::Heatmap
        )
    }
}

/// Toggle options shared across chart types (not all apply to every type).
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
pub struct ChartOptions {
    pub legend: bool,
    pub grid: bool,
    pub smooth: bool,
    pub stacked: bool,
    /// Draw the numeric value on each bar / point.
    pub data_labels: bool,
}

impl Default for ChartOptions {
    fn default() -> Self {
        Self {
            legend: true,
            grid: true,
            smooth: false,
            stacked: false,
            data_labels: false,
        }
    }
}

/// How to combine rows that share the same X value.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum Aggregation {
    /// No grouping — one mark per row.
    #[default]
    None,
    Sum,
    Average,
    Count,
    Min,
    Max,
}

impl Aggregation {
    pub const ALL: [Aggregation; 6] = [
        Aggregation::None,
        Aggregation::Sum,
        Aggregation::Average,
        Aggregation::Count,
        Aggregation::Min,
        Aggregation::Max,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Aggregation::None => "None",
            Aggregation::Sum => "Sum",
            Aggregation::Average => "Average",
            Aggregation::Count => "Count",
            Aggregation::Min => "Min",
            Aggregation::Max => "Max",
        }
    }
}

/// Row ordering applied after aggregation.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default, Serialize, Deserialize)]
pub enum SortMode {
    #[default]
    None,
    ValueDesc,
    ValueAsc,
    LabelAsc,
    LabelDesc,
}

impl SortMode {
    pub const ALL: [SortMode; 5] = [
        SortMode::None,
        SortMode::ValueDesc,
        SortMode::ValueAsc,
        SortMode::LabelAsc,
        SortMode::LabelDesc,
    ];

    pub fn label(self) -> &'static str {
        match self {
            SortMode::None => "None",
            SortMode::ValueDesc => "Value ↓",
            SortMode::ValueAsc => "Value ↑",
            SortMode::LabelAsc => "Label A–Z",
            SortMode::LabelDesc => "Label Z–A",
        }
    }
}

/// The 8-colour data-viz palette, derived from the active theme's role tokens
/// so charts re-theme with the app (and never hard-code colours). For
/// Catppuccin Mocha this reproduces the design's `--s0..--s7` almost exactly.
pub fn series_palette(c: &ThemeColors) -> [Color32; 8] {
    [
        c.accent,
        c.accent_secondary,
        c.syntax_key,
        c.success,
        c.syntax_number,
        c.error,
        c.warning,
        c.info,
    ]
}

/// Where a producer's data comes from — used to group the source picker.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProducerKind {
    /// A core file tab (JSON/NDJSON or a file-loader plugin like CSV).
    File,
    /// A plugin pane that declares the `data-producer` capability.
    Plugin,
}

/// An open tab eligible to provide a dataset, offered in the source picker.
#[derive(Clone)]
pub struct ProducerRef {
    pub tab_id: TabId,
    pub label: String,
    pub kind: ProducerKind,
}

/// A resolved column: display name + whether its values parse as numbers.
#[derive(Clone)]
pub struct ColumnInfo {
    pub name: String,
    pub numeric: bool,
}

/// A fully-specified chart request emitted by the config panel on **Generate**.
/// The app resolves the source rows and builds a [`ChartTab`] from this.
#[derive(Clone, Debug)]
pub struct ChartSpec {
    pub source_tab: TabId,
    pub source_label: String,
    pub chart_type: ChartType,
    pub x_col: usize,
    pub y_cols: Vec<usize>,
    pub options: ChartOptions,
    /// How to combine rows sharing an X value.
    pub aggregation: Aggregation,
    /// Keep only the top-N groups/rows by value (0 = all).
    pub top_n: usize,
    /// Row ordering.
    pub sort: SortMode,
    /// When `Some`, "Generate" updates this existing chart tab in place
    /// (the panel is editing it) rather than opening a new one.
    pub edit_target: Option<TabId>,
}

/// A toolbar action raised from a chart tab's header.
#[derive(Clone, Copy)]
pub enum ChartTabAction {
    /// Load this chart's config back into the studio panel for editing.
    Edit,
    /// Re-fetch the source data and rebuild the chart (and its axis names).
    Refresh,
}
