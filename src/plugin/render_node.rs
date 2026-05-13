use eframe::egui::{self, Frame};
use egui_code_editor::CodeEditor;
use serde::{Deserialize, Serialize};

use crate::components::button::{Button, ButtonProps, ButtonType};
use crate::components::common::button_group::{ButtonGroup, ButtonGroupItem, ButtonGroupProps};
use crate::components::common::input::{Input, InputProps};
use crate::components::common::json_tree::{JsonTree, JsonTreeProps};
use crate::components::common::list::{List, ListItem, ListProps};
use crate::components::common::select::{Select, SelectOption as CommonSelectOption, SelectProps};
use crate::components::common::tabs::{TabItem, TabProps, Tabs};
use crate::components::icon_button::{IconButton, IconButtonProps};
use crate::components::table_view::{TableCell, TableView, TableViewProps};
use crate::components::traits::StatelessComponent;
use crate::theme::{BgColorOptions, ThemeColors};

// =============================================================================
// UiNode — unified display + interactive DSL
//
// Used by both file-viewer plugins (render-record) and data-source plugins
// (render-ui / handle-event). Display-only variants carry no `id` and emit
// no events; interactive variants carry an `id` and emit UiEvent on change/click.
// =============================================================================

// ── Supporting types ─────────────────────────────────────────────────────────

/// An option entry used by select, multi-select, and radio nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
}

/// A single key/value pair used by the key-value-list node.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct KvEntry {
    pub key: String,
    pub value: String,
}

/// A single option in a button-group node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ButtonGroupOption {
    pub value: String,
    pub label: String,
}

/// An action button inside a list item.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListItemActionNode {
    pub icon: String,
    pub tooltip: String,
}

/// A colored badge shown before a list item's title (e.g. HTTP method).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListItemBadgeNode {
    pub text: String,
    /// Semantic color: "blue" | "green" | "red" | "orange" | "gray"
    pub color: String,
}

/// A single item inside a `list` DSL node.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListItemNode {
    pub title: String,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub icon: Option<String>,
    #[serde(default)]
    pub badge: Option<ListItemBadgeNode>,
    #[serde(default)]
    pub actions: Vec<ListItemActionNode>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum TextSize {
    Sm,
    #[default]
    Md,
    Lg,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Align {
    #[default]
    Start,
    Center,
    End,
    Fill,
}

/// An event emitted when the user interacts with a widget.
#[derive(Debug, Clone)]
pub struct UiEvent {
    pub widget_id: String,
    pub kind: String,
    pub value: String,
}

/// Return type of the plugin's `render-ui` and `handle-event` WIT functions.
#[derive(Debug, Clone)]
pub struct UiOutput {
    pub node_json: String,
    pub height_hint: u32,
}

// ── UiNode ────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum UiNode {
    // ── Layout ────────────────────────────────────────────────────────────
    Row {
        children: Vec<UiNode>,
        #[serde(default = "default_gap")]
        gap: f32,
        #[serde(default)]
        align: Align,
        #[serde(default, rename = "bg-color")]
        bg_color: BgColorOptions,
        #[serde(default, rename = "max-width")]
        max_width: bool,
        #[serde(default)]
        height: Option<f32>,
        #[serde(default)]
        padding: f32,
    },
    Column {
        children: Vec<UiNode>,
        #[serde(default = "default_gap")]
        gap: f32,
        #[serde(default, rename = "bg-color")]
        bg_color: BgColorOptions,
    },
    /// Horizontal split — each child gets a proportional share of the width.
    /// `widths` is an optional list of relative weights (e.g. `[1, 3]` gives
    /// 25 % / 75 %). When absent or empty every child gets an equal share.
    Split {
        children: Vec<UiNode>,
        #[serde(default = "default_gap")]
        gap: f32,
        #[serde(default)]
        widths: Vec<f32>,
        /// Draw a 1 px vertical separator line between each column.
        #[serde(default)]
        separator: bool,
    },
    /// Collapsible section — open by default (use for navigation/grouping).
    Group {
        label: String,
        children: Vec<UiNode>,
    },
    /// Collapsible section — closed by default (display-only, from file-viewer).
    Collapsible {
        label: String,
        children: Vec<UiNode>,
    },
    Scroll {
        id: String,
        child: Box<UiNode>,
        #[serde(default)]
        height: Option<f32>,
        #[serde(default, rename = "bg-color")]
        bg_color: BgColorOptions,
    },
    Spacer {
        #[serde(default = "default_spacer_height")]
        height: f32,
    },
    Separator,

    // ── Display (no id, no events) ────────────────────────────────────────
    Text {
        value: String,
        #[serde(default)]
        size: TextSize,
        #[serde(default)]
        muted: bool,
    },
    Bold {
        child: Box<UiNode>,
    },
    Italic {
        child: Box<UiNode>,
    },
    Colored {
        color: String,
        child: Box<UiNode>,
    },
    Heading {
        value: String,
        #[serde(default = "default_heading_level")]
        level: u8,
    },
    Badge {
        label: String,
        color: String,
    },
    Link {
        label: String,
        url: String,
    },
    Code {
        value: String,
        #[serde(default)]
        language: Option<String>,
    },
    Markdown {
        value: String,
    },
    Progress {
        value: f64,
    },
    #[serde(rename = "json-tree")]
    JsonTree {
        value: serde_json::Value,
    },
    /// Inline key → value display (display-only).
    KeyValue {
        key: String,
        value: Box<UiNode>,
    },
    /// Table display (display-only, from file-viewer render-record).
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<UiNode>>,
    },
    /// Interactive list using the List component with virtual scrolling.
    /// Each item emits a "click" event with value = item index (as JSON number)
    /// when the row is clicked, and "action" with value = JSON {"item":i,"action":j}
    /// when an action button is clicked.
    List {
        id: String,
        #[serde(default)]
        items: Vec<ListItemNode>,
        #[serde(default, rename = "empty-label")]
        empty_label: Option<String>,
    },
    /// A panel pinned to the bottom of its parent, outside any scroll area.
    /// Children are rendered top-to-bottom inside the footer area.
    Footer {
        #[serde(default = "default_gap")]
        gap: f32,
        #[serde(default)]
        padding: f32,
        children: Vec<UiNode>,
    },
    /// A modal overlay dialog, rendered on top of the full viewport.
    /// When `open` is false the modal is not shown.
    Modal {
        id: String,
        title: String,
        open: bool,
        children: Vec<UiNode>,
        /// Widget id emitted as a "click" event when the close (×) button is pressed.
        #[serde(default, rename = "close-id")]
        close_id: Option<String>,
        /// Fraction of screen width to use (0.0–1.0). Defaults to a fixed 480 px max.
        #[serde(default, rename = "width-pct")]
        width_pct: Option<f32>,
        /// Fraction of screen height to use (0.0–1.0). Defaults to auto.
        #[serde(default, rename = "height-pct")]
        height_pct: Option<f32>,
    },

    // ── Inputs (all have `id`, fire "change" events) ──────────────────────
    TextInput {
        id: String,
        label: String,
        #[serde(default)]
        value: String,
        #[serde(default)]
        placeholder: String,
        #[serde(default)]
        required: bool,
        #[serde(default)]
        disabled: bool,
        /// When true, the input expands to fill remaining width in a row.
        #[serde(default)]
        grow: bool,
        /// When true, renders a multiline text area instead of a single line.
        #[serde(default)]
        multiline: bool,
        /// Number of visible rows when multiline is true (default 4).
        #[serde(default)]
        rows: Option<u32>,
    },
    NumberInput {
        id: String,
        label: String,
        value: f64,
        #[serde(default)]
        min: Option<f64>,
        #[serde(default)]
        max: Option<f64>,
        #[serde(default)]
        disabled: bool,
    },
    PasswordInput {
        id: String,
        label: String,
        #[serde(default)]
        value: String,
        #[serde(default)]
        disabled: bool,
    },
    Textarea {
        id: String,
        label: String,
        #[serde(default)]
        value: String,
        #[serde(default = "default_rows")]
        rows: u32,
        #[serde(default)]
        disabled: bool,
    },
    Select {
        id: String,
        label: String,
        value: String,
        options: Vec<SelectOption>,
        #[serde(default)]
        disabled: bool,
    },
    MultiSelect {
        id: String,
        label: String,
        value: Vec<String>,
        options: Vec<SelectOption>,
        #[serde(default)]
        disabled: bool,
    },
    Checkbox {
        id: String,
        label: String,
        checked: bool,
        #[serde(default)]
        disabled: bool,
    },
    Toggle {
        id: String,
        label: String,
        checked: bool,
        #[serde(default)]
        disabled: bool,
    },
    Radio {
        id: String,
        label: String,
        value: String,
        options: Vec<SelectOption>,
        #[serde(default)]
        disabled: bool,
    },
    Slider {
        id: String,
        label: String,
        value: f64,
        min: f64,
        max: f64,
        #[serde(default)]
        disabled: bool,
    },
    KeyValueList {
        id: String,
        label: String,
        entries: Vec<KvEntry>,
        #[serde(default = "default_add_label", rename = "add-label")]
        add_label: String,
        #[serde(default)]
        disabled: bool,
    },

    /// Pill-style segmented selector. Emits a "change" event with the selected value.
    #[serde(rename = "button-group")]
    ButtonGroup {
        id: String,
        /// List of `{"value": "...", "label": "..."}` options.
        options: Vec<ButtonGroupOption>,
        /// Currently selected value.
        value: String,
    },

    // ── Actions (fire "click" events) ─────────────────────────────────────
    Button {
        id: String,
        props: ButtonProps,
        /// When set, clicking this button copies the text to the clipboard
        /// directly in the host (no plugin round-trip).
        #[serde(default)]
        copy: Option<String>,
    },
    IconButton {
        id: String,
        icon: String,
        #[serde(default)]
        tooltip: Option<String>,
        #[serde(default = "default_true")]
        enabled: bool,
        #[serde(default = "default_true")]
        frame: bool,
    },
    #[serde(rename = "code-editor")]
    CodeEditor {
        id: String,
        value: String,
    },
    Tabs {
        id: String,
        header: Vec<String>,
        children: Vec<UiNode>,
    },
    Spinner {
        #[serde(default)]
        size: Option<f32>,
    },
}

// ── serde default helpers ─────────────────────────────────────────────────────

fn default_gap() -> f32 {
    8.0
}
fn default_spacer_height() -> f32 {
    8.0
}
fn default_heading_level() -> u8 {
    2
}
fn default_rows() -> u32 {
    4
}
fn default_add_label() -> String {
    "Add".to_string()
}
fn default_true() -> bool {
    true
}

// =============================================================================
// Renderer
// =============================================================================

/// Map a badge color name to (background, foreground) colors.
fn method_badge_colors(color: &str) -> (egui::Color32, egui::Color32) {
    let white = egui::Color32::WHITE;
    match color {
        "blue" => (egui::Color32::from_rgb(59, 130, 246), white),
        "green" => (egui::Color32::from_rgb(34, 197, 94), white),
        "red" => (egui::Color32::from_rgb(239, 68, 68), white),
        "orange" => (egui::Color32::from_rgb(249, 115, 22), white),
        "purple" => (egui::Color32::from_rgb(168, 85, 247), white),
        _ => (egui::Color32::from_rgb(107, 114, 128), white), // gray
    }
}

/// Render a `UiNode` tree and collect interaction events.
///
/// For display-only nodes (Bold, Italic, Table, …) events will never be
/// emitted. For interactive nodes (inputs, buttons) events are collected and
/// should be forwarded to the plugin via `handle-event`.
pub fn render_ui_node(ui: &mut egui::Ui, node: &UiNode, events: &mut Vec<UiEvent>) {
    let colors = ui.ctx().memory(|mem| {
        mem.data
            .get_temp::<ThemeColors>(egui::Id::new("theme_colors"))
            .unwrap_or_else(|| crate::theme::Theme::default().colors())
    });

    match node {
        // ── Layout ────────────────────────────────────────────────────────
        UiNode::Row {
            children,
            gap,
            align,
            bg_color,
            height,
            max_width,
            padding,
        } => {
            let mut frame = egui::Frame::new().inner_margin(*padding);
            if let Some(color) = bg_color.into_color(&colors) {
                frame = frame.fill(color);
            }
            frame.show(ui, |ui| {
                if *max_width {
                    ui.set_width(ui.available_width());
                }

                if let Some(height) = height {
                    ui.set_height(*height);
                }

                ui.horizontal(|ui| {
                    ui.spacing_mut().item_spacing.x = *gap;
                    match align {
                        Align::Center | Align::End => {
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    for child in children.iter().rev() {
                                        render_ui_node(ui, child, events);
                                    }
                                },
                            );
                        }
                        // Fill: render children right-to-left in reverse order so
                        // right-side fixed items claim space first, then any `grow`
                        // child (e.g. TextInput with grow:true) fills the remainder.
                        // Visual result: [left fixed…] [grow fills] [right fixed…]
                        // Fill: produces [left fixed… | grow fills | right fixed…]
                        // 1. Render children BEFORE the grow item LTR — they claim
                        //    space from the left edge.
                        // 2. Open an RTL sub-layout for children AFTER the grow item
                        //    — they claim space from the right edge.
                        // 3. The grow child renders last inside the RTL layout and
                        //    calls available_width(), filling exactly the middle gap.
                        Align::Fill => {
                            let grow_idx = children
                                .iter()
                                .position(|c| matches!(c, UiNode::TextInput { grow: true, .. }));
                            ui.spacing_mut().item_spacing.x = *gap;
                            if let Some(gi) = grow_idx {
                                for child in &children[..gi] {
                                    render_ui_node(ui, child, events);
                                }
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.spacing_mut().item_spacing.x = *gap;
                                        for child in children[gi + 1..].iter() {
                                            render_ui_node(ui, child, events);
                                        }
                                        render_ui_node(ui, &children[gi], events);
                                    },
                                );
                            } else {
                                for child in children {
                                    render_ui_node(ui, child, events);
                                }
                            }
                        }
                        _ => {
                            for child in children {
                                render_ui_node(ui, child, events);
                            }
                        }
                    }
                });
            });
        }
        UiNode::Column {
            children,
            gap,
            bg_color,
        } => {
            let mut frame = Frame::new();
            if let Some(color) = bg_color.into_color(&colors) {
                frame = frame.fill(color);
            }
            frame.show(ui, |ui| {
                // Footer children must be rendered first so TopBottomPanel::bottom
                // can claim space before the rest of the children fill the remaining area.
                for child in children {
                    if matches!(child, UiNode::Footer { .. }) {
                        render_ui_node(ui, child, events);
                    }
                }
                ui.vertical(|ui| {
                    ui.spacing_mut().item_spacing.y = *gap;
                    for child in children {
                        if !matches!(child, UiNode::Footer { .. }) {
                            render_ui_node(ui, child, events);
                        }
                    }
                });
            });
        }
        UiNode::Split {
            children,
            gap,
            widths,
            separator,
        } => {
            let count = children.len();
            if count == 0 {
                return;
            }
            let mut col_events: Vec<UiEvent> = Vec::new();
            let sep_color = ui.visuals().widgets.noninteractive.bg_stroke.color;

            if widths.is_empty() {
                // Equal-width fast path — manual rect allocation so we can draw separators.
                let available = ui.available_width();
                let total_gap = *gap * count.saturating_sub(1) as f32;
                let col_w = (available - total_gap) / count as f32;

                let cursor = ui.cursor();
                let avail_h = ui.available_height();
                let mut x = cursor.left();
                let mut max_h: f32 = 0.0;

                for (i, child) in children.iter().enumerate() {
                    let col_rect = egui::Rect::from_min_size(
                        egui::pos2(x, cursor.top()),
                        egui::vec2(col_w, avail_h),
                    );
                    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(col_rect));
                    render_ui_node(&mut child_ui, child, &mut col_events);
                    max_h = max_h.max(child_ui.min_rect().height());
                    x += col_w;
                    if i + 1 < count {
                        if *separator {
                            let sep_x = x + gap / 2.0;
                            ui.painter().line_segment(
                                [
                                    egui::pos2(sep_x, cursor.top()),
                                    egui::pos2(sep_x, cursor.top() + avail_h),
                                ],
                                egui::Stroke::new(1.0, sep_color),
                            );
                        }
                        x += gap;
                    }
                }
                ui.allocate_space(egui::vec2(available, max_h));
            } else {
                // Proportional widths — manual rect allocation.
                let available = ui.available_width();
                let total_gap = gap * count.saturating_sub(1) as f32;
                let sum: f32 = widths.iter().copied().sum::<f32>().max(0.001);
                let col_widths: Vec<f32> = widths
                    .iter()
                    .map(|w| w / sum * (available - total_gap))
                    .collect();

                let cursor = ui.cursor();
                let avail_h = ui.available_height();
                let mut x = cursor.left();
                let mut max_h: f32 = 0.0;

                for (i, child) in children.iter().enumerate() {
                    let w = col_widths.get(i).copied().unwrap_or(0.0).max(0.0);
                    let col_rect = egui::Rect::from_min_size(
                        egui::pos2(x, cursor.top()),
                        egui::vec2(w, avail_h),
                    );
                    let mut child_ui = ui.new_child(egui::UiBuilder::new().max_rect(col_rect));
                    render_ui_node(&mut child_ui, child, &mut col_events);
                    max_h = max_h.max(child_ui.min_rect().height());
                    x += w;
                    if i + 1 < count {
                        if *separator {
                            let sep_x = x + gap / 2.0;
                            ui.painter().line_segment(
                                [
                                    egui::pos2(sep_x, cursor.top()),
                                    egui::pos2(sep_x, cursor.top() + avail_h),
                                ],
                                egui::Stroke::new(1.0, sep_color),
                            );
                        }
                        x += gap;
                    }
                }
                ui.allocate_space(egui::vec2(available, max_h));
            }

            events.extend(col_events);
        }
        UiNode::Group { label, children } => {
            egui::CollapsingHeader::new(label)
                .default_open(true)
                .show(ui, |ui| {
                    for child in children {
                        render_ui_node(ui, child, events);
                    }
                });
        }
        UiNode::Collapsible { label, children } => {
            egui::CollapsingHeader::new(label)
                .default_open(false)
                .show(ui, |ui| {
                    for child in children {
                        render_ui_node(ui, child, events);
                    }
                });
        }
        UiNode::Scroll {
            child,
            height,
            id,
            bg_color,
        } => {
            // Paint the full allocated rect (entire column height) before
            // rendering the scroll area so the background fills edge-to-edge.
            if let Some(fill) = bg_color.into_color(&colors) {
                ui.painter().rect_filled(ui.max_rect(), 0.0, fill);
            }
            ui.set_min_size(ui.available_size());
            let mut area = egui::ScrollArea::vertical().id_salt(id);
            if let Some(h) = height {
                area = area.max_height(*h);
            }
            area.show(ui, |ui| {
                render_ui_node(ui, child, events);
            });
        }
        UiNode::Spacer { height } => {
            ui.add_space(*height);
        }
        UiNode::Separator => {
            ui.separator();
        }

        // ── Display ───────────────────────────────────────────────────────
        UiNode::Text { value, size, muted } => {
            let mut rich = egui::RichText::new(value);
            rich = match size {
                TextSize::Sm => rich.small(),
                TextSize::Md => rich,
                TextSize::Lg => rich.size(18.0),
            };
            if *muted {
                rich = rich.weak();
            }
            ui.add(egui::Label::new(rich).wrap());
        }
        UiNode::Bold { child } => {
            ui.label(egui::RichText::new(collect_text(child)).strong());
        }
        UiNode::Italic { child } => {
            ui.label(egui::RichText::new(collect_text(child)).italics());
        }
        UiNode::Colored { color, child } => {
            let rich = egui::RichText::new(collect_text(child));
            if let Some(c) = parse_hex_color(color) {
                ui.colored_label(c, rich.text());
            } else {
                render_ui_node(ui, child, events);
            }
        }
        UiNode::Heading { value, level } => {
            let size = match level {
                1 => 24.0_f32,
                2 => 20.0,
                _ => 16.0,
            };
            ui.label(egui::RichText::new(value).strong().size(size));
        }
        UiNode::Badge { label, color } => {
            let c = parse_hex_color(color).unwrap_or(egui::Color32::GRAY);
            egui::Frame::new()
                .fill(c)
                .corner_radius(3.0)
                .inner_margin(egui::Margin::symmetric(4, 2))
                .show(ui, |ui| {
                    ui.label(label);
                });
        }
        UiNode::Link { label, url } => {
            ui.hyperlink_to(label, url);
        }
        UiNode::Code { value, .. } => {
            egui::ScrollArea::horizontal().show(ui, |ui| {
                ui.add(
                    egui::Label::new(egui::RichText::new(value).monospace())
                        .wrap_mode(egui::TextWrapMode::Extend),
                );
            });
        }
        UiNode::Markdown { value } => {
            ui.label(value);
        }
        UiNode::Progress { value } => {
            ui.add(egui::ProgressBar::new(*value as f32));
        }
        UiNode::JsonTree { value } => {
            // Use a stable ID derived from the widget's position in the tree
            // so multiple json-tree nodes on the same frame don't share state.
            let tree_id = ui.next_auto_id().with("json_tree");
            JsonTree::render(ui, JsonTreeProps { value, id: tree_id });
        }
        UiNode::KeyValue { key, value } => {
            ui.horizontal(|ui| {
                ui.strong(key);
                ui.label(":");
                render_ui_node(ui, value, events);
            });
        }
        UiNode::Table { headers, rows } => {
            let row_count = rows.len();
            TableView::render(
                ui,
                TableViewProps {
                    headers,
                    row_count,
                    min_col_width: None,
                    build_row: Box::new(move |i| {
                        rows.get(i)
                            .map(|row| {
                                row.iter()
                                    .map(|cell| {
                                        TableCell::custom(move |ui| {
                                            render_ui_node(ui, cell, &mut Vec::new());
                                        })
                                    })
                                    .collect()
                            })
                            .unwrap_or_default()
                    }),
                },
            );
        }

        UiNode::List {
            id,
            items,
            empty_label,
        } => {
            let list_items: Vec<ListItem<'_>> = items
                .iter()
                .map(|item| ListItem {
                    title: &item.title,
                    description: item.description.as_deref(),
                    prefix: item.icon.as_deref().map(|glyph| {
                        crate::components::common::list::ListItemPrefix::Icon { glyph, color: None }
                    }),
                    postfix: None,
                    badge: item.badge.as_ref().map(|b| {
                        let (bg, fg) = method_badge_colors(b.color.as_str());
                        crate::components::common::list::ListItemBadge {
                            text: &b.text,
                            color: bg,
                            text_color: fg,
                        }
                    }),
                    selected: false,
                    tags: &[],
                })
                .collect();

            // Wrap in a child UI keyed by id so the List's internal egui IDs
            // (hover state, scroll position) are stable across re-renders.
            ui.push_id(egui::Id::new(("ui:list", id)), |ui| {
                let output = List::render(
                    ui,
                    ListProps {
                        items: &list_items,
                        empty_label: empty_label.as_deref(),
                        shrink_to_fit: false,
                        show_separators: true,
                        compact: false,
                    },
                );

                if let Some(row_idx) = output.row_clicked {
                    events.push(UiEvent {
                        widget_id: id.clone(),
                        kind: "click".to_string(),
                        value: row_idx.to_string(),
                    });
                }
            });
        }

        UiNode::Footer {
            gap,
            padding,
            children,
        } => {
            egui::Panel::bottom(egui::Id::new("plugin_footer"))
                .frame(egui::Frame::new().inner_margin(*padding))
                .show_inside(ui, |ui| {
                    ui.spacing_mut().item_spacing.y = *gap;
                    for child in children {
                        render_ui_node(ui, child, events);
                    }
                });
        }

        UiNode::Modal {
            id,
            title,
            open,
            children,
            close_id,
            width_pct,
            height_pct,
        } => {
            if !open {
                return;
            }
            use crate::components::common::modal::{
                Modal as ModalComponent, ModalPropsBoxed, ModalSize,
            };
            let size = match (width_pct, height_pct) {
                (Some(w), Some(h)) => Some(ModalSize {
                    width_pct: *w,
                    height_pct: *h,
                }),
                (Some(w), None) => Some(ModalSize {
                    width_pct: *w,
                    height_pct: 0.8,
                }),
                _ => None,
            };
            let output = ModalComponent::render(
                ui,
                ModalPropsBoxed {
                    id: id.as_str(),
                    title: title.as_str(),
                    size,
                    body: Box::new(|ui| {
                        for child in children {
                            render_ui_node(ui, child, events);
                        }
                    }),
                },
            );
            if output.close_requested {
                if let Some(cid) = close_id {
                    events.push(UiEvent {
                        widget_id: cid.clone(),
                        kind: "click".to_string(),
                        value: String::new(),
                    });
                }
            }
        }

        // ── Inputs ────────────────────────────────────────────────────────
        UiNode::TextInput {
            id,
            label,
            value,
            placeholder,
            disabled,
            multiline,
            rows,
            ..
        } => {
            let buf_id = egui::Id::new(("ui:text", id));
            let prev_id = egui::Id::new(("ui:text:prev", id));
            // If the plugin-side value changed (e.g. a saved request was loaded),
            // discard the egui buffer and use the fresh value instead.
            let prev: String = ui.ctx().data(|d| d.get_temp(prev_id).unwrap_or_default());
            let mut buf = if prev != *value {
                value.clone()
            } else {
                ui.ctx().data(|d| {
                    d.get_temp::<String>(buf_id)
                        .unwrap_or_else(|| value.clone())
                })
            };
            if !label.is_empty() {
                ui.label(label.as_str());
            }
            if *multiline {
                let out = Input::render(
                    ui,
                    InputProps {
                        value: &mut buf,
                        placeholder: placeholder.as_str(),
                        icon: None,
                        password: false,
                        disabled: *disabled,
                        multiline: true,
                        rows: rows.unwrap_or(4) as usize,
                        desired_width: None,
                        id_salt: Some(egui::Id::new(("ui:text:scroll", id.as_str()))),
                    },
                );
                if out.changed {
                    events.push(ui_event(id, "change", json_str(&buf)));
                }
                ui.ctx().data_mut(|d| {
                    d.insert_temp(buf_id, buf);
                    d.insert_temp(prev_id, value.clone());
                });
                return;
            } else {
                let out = Input::render(
                    ui,
                    InputProps {
                        value: &mut buf,
                        placeholder: placeholder.as_str(),
                        icon: None,
                        password: false,
                        disabled: *disabled,
                        multiline: false,
                        rows: 1,
                        desired_width: None,
                        id_salt: None,
                    },
                );
                if out.changed {
                    events.push(ui_event(id, "change", json_str(&buf)));
                }
            }
            ui.ctx().data_mut(|d| {
                d.insert_temp(buf_id, buf);
                d.insert_temp(prev_id, value.clone());
            });
        }
        UiNode::ButtonGroup { id, options, value } => {
            let items: Vec<ButtonGroupItem<'_>> = options
                .iter()
                .map(|o| ButtonGroupItem {
                    value: &o.value,
                    label: &o.label,
                })
                .collect();
            let output = ButtonGroup::render(
                ui,
                ButtonGroupProps {
                    items: &items,
                    active: value.as_str(),
                },
            );
            if let Some(selected) = output.selected {
                events.push(ui_event(id, "change", json_str(&selected)));
            }
        }
        UiNode::NumberInput {
            id,
            label,
            value,
            min,
            max,
            disabled,
        } => {
            let buf_id = egui::Id::new(("ui:num", id));
            let mut buf = ui
                .ctx()
                .data_mut(|d| d.get_temp::<f64>(buf_id).unwrap_or(*value));
            if !label.is_empty() {
                ui.label(label.as_str());
            }
            let range = min.unwrap_or(f64::NEG_INFINITY)..=max.unwrap_or(f64::INFINITY);
            let resp = ui.add_enabled(!disabled, egui::DragValue::new(&mut buf).range(range));
            if resp.changed() {
                events.push(ui_event(id, "change", buf.to_string()));
            }
            ui.ctx().data_mut(|d| d.insert_temp(buf_id, buf));
        }
        UiNode::PasswordInput {
            id,
            label,
            value,
            disabled,
        } => {
            let buf_id = egui::Id::new(("ui:pw", id));
            let mut buf = ui.ctx().data_mut(|d| {
                d.get_temp::<String>(buf_id)
                    .unwrap_or_else(|| value.clone())
            });
            if !label.is_empty() {
                ui.label(label.as_str());
            }
            let out = Input::render(
                ui,
                InputProps {
                    value: &mut buf,
                    placeholder: "",
                    icon: None,
                    password: true,
                    disabled: *disabled,
                    multiline: false,
                    rows: 1,
                    desired_width: None,
                    id_salt: None,
                },
            );
            if out.changed {
                events.push(ui_event(id, "change", json_str(&buf)));
            }
            ui.ctx().data_mut(|d| d.insert_temp(buf_id, buf));
        }
        UiNode::Textarea {
            id,
            label,
            value,
            rows,
            disabled,
        } => {
            let buf_id = egui::Id::new(("ui:ta", id));
            let mut buf = ui.ctx().data_mut(|d| {
                d.get_temp::<String>(buf_id)
                    .unwrap_or_else(|| value.clone())
            });
            if !label.is_empty() {
                ui.label(label.as_str());
            }
            let out = Input::render(
                ui,
                InputProps {
                    value: &mut buf,
                    placeholder: "",
                    icon: None,
                    password: false,
                    disabled: *disabled,
                    multiline: true,
                    rows: *rows as usize,
                    desired_width: None,
                    id_salt: None,
                },
            );
            if out.changed {
                events.push(ui_event(id, "change", json_str(&buf)));
            }
            ui.ctx().data_mut(|d| d.insert_temp(buf_id, buf));
        }
        UiNode::Select {
            id,
            label,
            value,
            options,
            disabled,
        } => {
            let buf_id = egui::Id::new(("ui:sel", id));
            let mut buf = ui.ctx().data_mut(|d| {
                d.get_temp::<String>(buf_id)
                    .unwrap_or_else(|| value.clone())
            });
            if !label.is_empty() {
                ui.label(label.as_str());
            }
            if !disabled {
                let common_opts: Vec<CommonSelectOption> = options
                    .iter()
                    .map(|o| CommonSelectOption {
                        value: o.value.clone(),
                        label: o.label.clone(),
                    })
                    .collect();
                let out = Select::render(
                    ui,
                    SelectProps {
                        id_salt: id,
                        value: buf.as_str(),
                        options: &common_opts,
                        prefix_label: None,
                        size: Default::default(),
                    },
                );
                if let Some(new_val) = out.changed {
                    events.push(ui_event(id, "change", json_str(&new_val)));
                    buf = new_val;
                }
            } else {
                let current = options
                    .iter()
                    .find(|o| o.value == buf)
                    .map(|o| o.label.as_str())
                    .unwrap_or(buf.as_str());
                ui.add_enabled(false, egui::Label::new(current));
            }
            ui.ctx().data_mut(|d| d.insert_temp(buf_id, buf));
        }
        UiNode::MultiSelect {
            id,
            label,
            value,
            options,
            disabled,
        } => {
            let buf_id = egui::Id::new(("ui:ms", id));
            let mut buf: Vec<String> = ui.ctx().data_mut(|d| {
                d.get_temp::<Vec<String>>(buf_id)
                    .unwrap_or_else(|| value.clone())
            });
            if !label.is_empty() {
                ui.label(label.as_str());
            }
            let mut changed = false;
            for opt in options {
                let mut sel = buf.contains(&opt.value);
                if ui
                    .add_enabled(!disabled, egui::Checkbox::new(&mut sel, opt.label.as_str()))
                    .changed()
                {
                    if sel {
                        if !buf.contains(&opt.value) {
                            buf.push(opt.value.clone());
                            changed = true;
                        }
                    } else {
                        buf.retain(|v| v != &opt.value);
                        changed = true;
                    }
                }
            }
            if changed {
                events.push(ui_event(
                    id,
                    "change",
                    serde_json::to_string(&buf).unwrap_or_default(),
                ));
            }
            ui.ctx().data_mut(|d| d.insert_temp(buf_id, buf));
        }
        UiNode::Radio {
            id,
            label,
            value,
            options,
            disabled,
        } => {
            let buf_id = egui::Id::new(("ui:rad", id));
            let mut buf = ui.ctx().data_mut(|d| {
                d.get_temp::<String>(buf_id)
                    .unwrap_or_else(|| value.clone())
            });
            if !label.is_empty() {
                ui.label(label.as_str());
            }
            ui.horizontal(|ui| {
                for opt in options {
                    let sel = buf == opt.value;
                    if ui
                        .add_enabled(!disabled, egui::RadioButton::new(sel, opt.label.as_str()))
                        .clicked()
                        && !sel
                    {
                        buf = opt.value.clone();
                        events.push(ui_event(id, "change", json_str(&buf)));
                    }
                }
            });
            ui.ctx().data_mut(|d| d.insert_temp(buf_id, buf));
        }
        UiNode::Checkbox {
            id,
            label,
            checked,
            disabled,
        } => {
            let buf_id = egui::Id::new(("ui:cb", id));
            let mut buf = ui
                .ctx()
                .data_mut(|d| d.get_temp::<bool>(buf_id).unwrap_or(*checked));
            if ui
                .add_enabled(!disabled, egui::Checkbox::new(&mut buf, label.as_str()))
                .changed()
            {
                events.push(ui_event(id, "change", buf.to_string()));
            }
            ui.ctx().data_mut(|d| d.insert_temp(buf_id, buf));
        }
        UiNode::Toggle {
            id,
            label,
            checked,
            disabled,
        } => {
            let buf_id = egui::Id::new(("ui:tog", id));
            let mut buf = ui
                .ctx()
                .data_mut(|d| d.get_temp::<bool>(buf_id).unwrap_or(*checked));
            if ui
                .add_enabled(!disabled, egui::Checkbox::new(&mut buf, label.as_str()))
                .changed()
            {
                events.push(ui_event(id, "change", buf.to_string()));
            }
            ui.ctx().data_mut(|d| d.insert_temp(buf_id, buf));
        }
        UiNode::Slider {
            id,
            label,
            value,
            min,
            max,
            disabled,
        } => {
            let buf_id = egui::Id::new(("ui:sl", id));
            let mut buf = ui
                .ctx()
                .data_mut(|d| d.get_temp::<f64>(buf_id).unwrap_or(*value));
            if !label.is_empty() {
                ui.label(label.as_str());
            }
            if ui
                .add_enabled(!disabled, egui::Slider::new(&mut buf, *min..=*max))
                .changed()
            {
                events.push(ui_event(id, "change", buf.to_string()));
            }
            ui.ctx().data_mut(|d| d.insert_temp(buf_id, buf));
        }
        UiNode::KeyValueList {
            id,
            label,
            entries,
            add_label,
            disabled,
        } => {
            let buf_id = egui::Id::new(("ui:kv", id));
            let prev_id = egui::Id::new(("ui:kv:prev", id));
            // If the plugin changed `entries` externally (e.g. URL param parsing),
            // discard the stale egui-memory buffer and use the new entries instead.
            let prev: Vec<KvEntry> = ui.ctx().data(|d| d.get_temp(prev_id).unwrap_or_default());
            let entries_changed = prev != *entries;
            let mut buf: Vec<KvEntry> = if entries_changed {
                entries.clone()
            } else {
                ui.ctx().data(|d| {
                    d.get_temp::<Vec<KvEntry>>(buf_id)
                        .unwrap_or_else(|| entries.clone())
                })
            };
            if !label.is_empty() {
                ui.label(label.as_str());
            }

            let mut changed = false;
            let mut to_remove: Option<usize> = None;
            let delete_col_w = 24.0;
            let available = ui.available_width();
            let input_w = ((available - delete_col_w - 8.0) / 2.0).max(40.0);

            // ── Header row ────────────────────────────────────────────────
            // TextEdit has ~4px internal left padding; match it so header
            // labels align with the placeholder/input text below.
            let text_edit_pad = 4.0;
            let header_rect = ui
                .horizontal(|ui| {
                    ui.set_width(available);
                    ui.spacing_mut().item_spacing.x = 4.0;
                    let label_color = colors.fg_muted;
                    let font = egui::FontId::proportional(11.0);
                    ui.painter().text(
                        ui.cursor().min + egui::vec2(text_edit_pad, 8.0),
                        egui::Align2::LEFT_TOP,
                        "KEY",
                        font.clone(),
                        label_color,
                    );
                    ui.allocate_exact_size(egui::vec2(input_w, 24.0), egui::Sense::hover());
                    ui.painter().text(
                        ui.cursor().min + egui::vec2(text_edit_pad, 8.0),
                        egui::Align2::LEFT_TOP,
                        "VALUE",
                        font,
                        label_color,
                    );
                    ui.allocate_exact_size(egui::vec2(input_w, 24.0), egui::Sense::hover());
                })
                .response
                .rect;

            // Bottom border under header
            let border_y = header_rect.bottom();
            ui.painter().line_segment(
                [
                    egui::pos2(header_rect.left(), border_y),
                    egui::pos2(header_rect.right(), border_y),
                ],
                egui::Stroke::new(1.0, colors.surface_raised),
            );

            // ── Data rows ─────────────────────────────────────────────────
            for (i, entry) in buf.iter_mut().enumerate() {
                ui.horizontal(|ui| {
                    ui.set_width(available);
                    ui.spacing_mut().item_spacing.x = 4.0;
                    if ui
                        .add_sized(
                            egui::vec2(input_w, 24.0),
                            egui::TextEdit::singleline(&mut entry.key)
                                .frame(Frame::NONE)
                                .hint_text("key")
                                .background_color(egui::Color32::TRANSPARENT),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    if ui
                        .add_sized(
                            egui::vec2(input_w, 24.0),
                            egui::TextEdit::singleline(&mut entry.value)
                                .frame(Frame::NONE)
                                .hint_text("value")
                                .background_color(egui::Color32::TRANSPARENT),
                        )
                        .changed()
                    {
                        changed = true;
                    }
                    if !*disabled
                        && IconButton::render(
                            ui,
                            IconButtonProps {
                                icon: egui_phosphor::regular::X,
                                frame: false,
                                tooltip: Some("Remove"),
                                size: Some((14.0, 14.0).into()),
                                ..Default::default()
                            },
                        )
                        .clicked
                    {
                        to_remove = Some(i);
                    }
                });

                // Row separator
                let row_rect = ui.min_rect();
                ui.painter().line_segment(
                    [
                        egui::pos2(row_rect.left(), row_rect.bottom()),
                        egui::pos2(row_rect.right(), row_rect.bottom()),
                    ],
                    egui::Stroke::new(1.0, colors.surface),
                );
            }

            if let Some(idx) = to_remove {
                buf.remove(idx);
                changed = true;
            }

            // ── Add row button ─────────────────────────────────────────────
            if !*disabled {
                ui.add_space(4.0);
                if Button::render(
                    ui,
                    ButtonProps {
                        icon: Some(egui_phosphor::regular::PLUS.to_string()),
                        label: add_label.to_string(),
                        button_type: ButtonType::Text,
                        ..Default::default()
                    },
                )
                .clicked
                {
                    buf.push(KvEntry {
                        key: String::new(),
                        value: String::new(),
                    });
                    changed = true;
                }
            }

            if changed {
                events.push(ui_event(
                    id,
                    "change",
                    serde_json::to_string(&buf).unwrap_or_default(),
                ));
            }
            ui.ctx().data_mut(|d| {
                d.insert_temp(buf_id, buf);
                // Record what entries looked like this frame so next frame
                // can detect plugin-side mutations (e.g. URL param parsing).
                d.insert_temp(prev_id, entries.clone());
            });
        }

        // ── Actions ───────────────────────────────────────────────────────
        UiNode::Button { id, props, copy } => {
            if Button::render(ui, props.clone()).clicked {
                if let Some(text) = copy {
                    ui.ctx().copy_text(text.clone());
                }
                if !id.is_empty() {
                    events.push(ui_event(id, "click", String::new()));
                }
            }
        }
        UiNode::IconButton {
            id,
            icon,
            tooltip,
            enabled,
            frame,
        } => {
            let btn = IconButton::render(
                ui,
                IconButtonProps {
                    icon,
                    frame: *frame,
                    tooltip: Some(&tooltip.clone().unwrap_or_default()),
                    disabled: !*enabled,
                    selected: false,
                    ..Default::default()
                },
            );
            if btn.clicked {
                events.push(ui_event(id, "click", String::new()));
            }
        }
        UiNode::Spinner { size } => {
            let sz = size.unwrap_or(16.0);
            ui.add(egui::Spinner::new().color(colors.accent).size(sz));
        }
        UiNode::CodeEditor { id, value } => {
            let buf_id = egui::Id::new(("ui:code-editor", id));
            let mut buf = ui.ctx().data_mut(|d| {
                d.get_temp::<String>(buf_id)
                    .unwrap_or_else(|| value.clone())
            });
            let colors = ui.ctx().memory(|mem| {
                mem.data
                    .get_temp::<crate::theme::ThemeColors>(egui::Id::new("theme_colors"))
                    .unwrap_or_else(|| crate::theme::Theme::default().colors())
            });
            let resp = CodeEditor::default()
                .with_theme(colors.code_editor_theme())
                .with_ui_fontsize(ui)
                .show(ui, &mut buf);
            if resp.response.changed() {
                events.push(ui_event(id, "change", json_str(&buf)));
            }
            ui.ctx().data_mut(|d| d.insert_temp(buf_id, buf));
        }
        UiNode::Tabs {
            id,
            header,
            children,
        } => {
            // Active tab index is persisted in egui memory keyed by id.
            let mem_id = egui::Id::new(("ui:tabs", id.as_str()));
            let mut active_idx: usize = ui.ctx().data(|d| d.get_temp(mem_id).unwrap_or(0usize));
            active_idx = active_idx.min(header.len().saturating_sub(1));

            // Build owned index strings first, then borrow them for TabItem.
            let index_strs: Vec<String> = (0..header.len()).map(|i| i.to_string()).collect();
            let tab_items: Vec<TabItem<'_>> = header
                .iter()
                .zip(index_strs.iter())
                .map(|(h, idx)| TabItem {
                    value: idx.as_str(),
                    label: h.as_str(),
                })
                .collect();

            let active_str = active_idx.to_string();
            let output = Tabs::render(
                ui,
                TabProps {
                    id: mem_id,
                    items: &tab_items,
                    active: &active_str,
                },
            );

            if let Some(val) = output.selected {
                if let Ok(new_idx) = val.parse::<usize>() {
                    active_idx = new_idx;
                    ui.ctx().data_mut(|d| d.insert_temp(mem_id, active_idx));
                    // Notify the plugin which header label was selected.
                    if let Some(h) = header.get(active_idx) {
                        events.push(ui_event(id, "change", json_str(h)));
                    }
                }
            }

            if let Some(child) = children.get(active_idx) {
                egui::Frame::new()
                    .inner_margin(egui::Margin {
                        left: 8,
                        right: 8,
                        top: 4,
                        bottom: 8,
                    })
                    .show(ui, |ui| {
                        render_ui_node(ui, child, events);
                    });
            }
        }
    }
}

// =============================================================================
// Helpers
// =============================================================================

/// Recursively extract plain text from a display node (for Bold/Italic/Colored).
fn collect_text(node: &UiNode) -> String {
    match node {
        UiNode::Text { value, .. }
        | UiNode::Heading { value, .. }
        | UiNode::Code { value, .. }
        | UiNode::Markdown { value } => value.clone(),
        UiNode::Bold { child } | UiNode::Italic { child } | UiNode::Colored { child, .. } => {
            collect_text(child)
        }
        UiNode::Badge { label, .. } | UiNode::Link { label, .. } => label.clone(),
        UiNode::KeyValue { key, value } => format!("{}: {}", key, collect_text(value)),
        UiNode::Row { children, .. }
        | UiNode::Column { children, .. }
        | UiNode::Split { children, .. }
        | UiNode::Group { children, .. }
        | UiNode::Collapsible { children, .. } => children
            .iter()
            .map(collect_text)
            .collect::<Vec<_>>()
            .join(" "),
        UiNode::Table { headers, rows } => {
            let mut t = headers.join(" ") + "\n";
            for row in rows {
                t += &row.iter().map(collect_text).collect::<Vec<_>>().join(" ");
                t += "\n";
            }
            t
        }
        UiNode::JsonTree { value } => value.to_string(),
        UiNode::ButtonGroup { value, .. } => value.clone(),
        _ => String::new(),
    }
}

fn parse_hex_color(s: &str) -> Option<egui::Color32> {
    let s = s.strip_prefix('#')?;
    if s.len() == 6 {
        let r = u8::from_str_radix(&s[0..2], 16).ok()?;
        let g = u8::from_str_radix(&s[2..4], 16).ok()?;
        let b = u8::from_str_radix(&s[4..6], 16).ok()?;
        Some(egui::Color32::from_rgb(r, g, b))
    } else {
        None
    }
}

fn ui_event(widget_id: &str, kind: &str, value: String) -> UiEvent {
    UiEvent {
        widget_id: widget_id.to_string(),
        kind: kind.to_string(),
        value,
    }
}

fn json_str(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| format!("\"{}\"", s))
}
