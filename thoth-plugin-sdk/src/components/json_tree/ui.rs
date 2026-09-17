//! Rendering for [`JsonTree`].
//!
//! ## Why the rows are built the way they are
//!
//! The obvious implementation flattens the whole value into a `Vec` of rows and
//! lets `show_rows` pick out the visible slice. That costs one row struct per
//! record whether or not it is on screen, which a file-sized dataset cannot
//! afford — a million collapsed records would allocate a million rows to draw
//! forty of them.
//!
//! So rows are materialized only for **expanded** records. A collapsed record
//! is exactly one row and its text is formulaic, so it is synthesized on demand
//! when the viewport asks for it. Mapping a row index back to a record is a
//! binary search over the (few) expanded records — see [`RowIndex`].
//!
//! Memory is therefore O(expanded), not O(records).

use std::collections::HashSet;

use serde_json::Value;

use crate::components::{DataRow, RowHighlights};
use crate::dataset::{DatasetAccess, NodeKind, TreeNode, dataset_access};
use crate::theme::{
    RADIUS_PANEL, ROW_HEIGHT, TextToken, ThemeColors, color_to_hex, edge_stroke, with_alpha,
};

use super::{JsonTree, JsonTreeOutput, TreeAction};

/// Inner padding of the design's `.tree{padding:4px}` container.
const TREE_PAD: i8 = 4;
/// Zebra wash — design `.tree.zebra .dr:nth-child(even){background:text 3%}`.
const ZEBRA_ALPHA: u8 = 8;
/// Indent guide width, matching `DataRow`'s indent step.
const INDENT_STEP: f32 = 16.0;
/// Indent guide alpha against the row text colour.
const GUIDE_ALPHA: u8 = 40;

/// A render-ready row, mapped directly onto [`DataRow`] fields.
#[derive(Clone)]
struct TreeRow {
    /// Full tree path — identity for expansion, selection and highlight lookup.
    path: String,
    /// Which record this row belongs to, and where within it — what the
    /// clipboard reads, rather than the rendered text, which is truncated.
    root: u64,
    rel: String,
    /// The row's key, for copying a key on its own.
    label: String,
    indent: usize,
    text: String,
    key_token: TextToken,
    value_token: Option<TextToken>,
    /// `Some(expanded)` for expandable rows; `None` for leaves and closers.
    caret: Option<bool>,
    /// Drawn muted — a collapsed-container summary.
    summary: bool,
}

#[derive(Clone, Default)]
struct TreeState {
    expanded: HashSet<String>,
    selected: Option<String>,
}

// ── Source ───────────────────────────────────────────────────────────────────

/// Where a tree's nodes come from: an inline value, or a dataset read lazily.
enum Kind<'a> {
    Inline(&'a Value),
    Handle {
        handle: &'a str,
        access: &'static DatasetAccess,
    },
}

struct Source<'a> {
    kind: Kind<'a>,
    /// Memoized answer for handle sources, where expandability comes from the
    /// schema and is therefore the same for every record. Without this the
    /// registry is consulted once per visible row, every frame.
    uniform_expandable: std::cell::Cell<Option<bool>>,
}

impl<'a> Source<'a> {
    fn new(tree: &'a JsonTree) -> Self {
        let kind = match (tree.handle.as_deref(), dataset_access()) {
            (Some(handle), Some(access)) => Kind::Handle { handle, access },
            _ => Kind::Inline(&tree.value),
        };
        Self {
            kind,
            uniform_expandable: std::cell::Cell::new(None),
        }
    }

    /// How many records the tree has. An inline array is a list of records;
    /// any other inline value is a single record.
    fn records(&self) -> u64 {
        match &self.kind {
            Kind::Inline(Value::Array(items)) => items.len() as u64,
            Kind::Inline(_) => 1,
            Kind::Handle { handle, access } => (access.total)(handle),
        }
    }

    /// Whether records can be expanded. Handle-backed sources answer from the
    /// schema, so this costs no read.
    fn record_expandable(&self, root: u64) -> bool {
        match &self.kind {
            Kind::Inline(_) => matches!(
                self.record(root),
                Some(Value::Object(_)) | Some(Value::Array(_))
            ),
            Kind::Handle { handle, access } => match self.uniform_expandable.get() {
                Some(known) => known,
                None => {
                    let answer = (access.records_expandable)(handle);
                    self.uniform_expandable.set(Some(answer));
                    answer
                }
            },
        }
    }

    fn record(&self, root: u64) -> Option<&'a Value> {
        match &self.kind {
            Kind::Inline(Value::Array(items)) => items.get(root as usize),
            Kind::Inline(value) => (root == 0).then_some(*value),
            Kind::Handle { .. } => None,
        }
    }

    fn children(&self, root: u64, rel: &str) -> Vec<TreeNode> {
        match &self.kind {
            Kind::Inline(_) => self
                .record(root)
                .and_then(|record| walk(record, rel))
                .map(value_children)
                .unwrap_or_default(),
            Kind::Handle { handle, access } => (access.children)(handle, root, rel),
        }
    }

    /// The subtree at `rel` as JSON — the edge conversion, for the clipboard.
    fn node_json(&self, root: u64, rel: &str) -> Option<Value> {
        match &self.kind {
            Kind::Inline(_) => self
                .record(root)
                .and_then(|record| walk(record, rel))
                .cloned(),
            Kind::Handle { handle, access } => (access.node_json)(handle, root, rel),
        }
    }

    fn node_preview(&self, root: u64, rel: &str) -> String {
        match &self.kind {
            Kind::Inline(_) => self
                .record(root)
                .and_then(|record| walk(record, rel))
                .map(scalar_text)
                .unwrap_or_else(|| "null".to_string()),
            Kind::Handle { handle, access } => (access.node_preview)(handle, root, rel),
        }
    }
}

/// Resolve a record-relative path inside an inline value.
fn walk<'a>(value: &'a Value, rel: &str) -> Option<&'a Value> {
    let mut cur = value;
    for segment in split_path(rel) {
        cur = match segment {
            Segment::Field(name) => cur.get(&name)?,
            Segment::Index(i) => cur.get(i)?,
        };
    }
    Some(cur)
}

enum Segment {
    Field(String),
    Index(usize),
}

fn split_path(rel: &str) -> Vec<Segment> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut chars = rel.chars().peekable();
    while let Some(ch) = chars.next() {
        match ch {
            '.' => {
                if !current.is_empty() {
                    out.push(Segment::Field(std::mem::take(&mut current)));
                }
            }
            '[' => {
                if !current.is_empty() {
                    out.push(Segment::Field(std::mem::take(&mut current)));
                }
                let mut digits = String::new();
                for d in chars.by_ref() {
                    if d == ']' {
                        break;
                    }
                    digits.push(d);
                }
                if let Ok(i) = digits.parse::<usize>() {
                    out.push(Segment::Index(i));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        out.push(Segment::Field(current));
    }
    out
}

/// Tree nodes for an inline value's immediate children.
fn value_children(value: &Value) -> Vec<TreeNode> {
    let node = |label: String, segment: String, child: &Value| {
        let kind = match child {
            Value::Object(_) => NodeKind::Struct,
            Value::Array(_) => NodeKind::List,
            _ => NodeKind::Leaf,
        };
        TreeNode {
            label,
            segment,
            kind,
            preview: if kind.is_expandable() {
                String::new()
            } else {
                scalar_text(child)
            },
            token: scalar_token(child),
        }
    };
    match value {
        Value::Object(map) => map
            .iter()
            .map(|(k, v)| node(k.clone(), format!(".{k}"), v))
            .collect(),
        Value::Array(items) => items
            .iter()
            .enumerate()
            .map(|(i, v)| node(i.to_string(), format!("[{i}]"), v))
            .collect(),
        _ => Vec::new(),
    }
}

fn scalar_text(val: &Value) -> String {
    match val {
        Value::String(s) => format!("\"{s}\""),
        Value::Null => "null".to_string(),
        other => other.to_string(),
    }
}

fn scalar_token(val: &Value) -> TextToken {
    match val {
        Value::String(_) => TextToken::Str,
        Value::Number(_) => TextToken::Number,
        Value::Bool(_) | Value::Null => TextToken::Boolean,
        _ => TextToken::Bracket,
    }
}

// ── Lazy row index ───────────────────────────────────────────────────────────

/// One expanded record's materialized rows and where they start.
struct Expanded {
    /// Position in the visible-record list, so a path can be located without
    /// walking the rows.
    pos: usize,
    start: usize,
    rows: Vec<TreeRow>,
}

/// Maps row indices to rows, materializing only expanded records.
struct RowIndex {
    /// The records on show, in order. `None` means "all of them", which avoids
    /// allocating a list of a million indices just to say so.
    visible: Option<Vec<u64>>,
    records: u64,
    expanded: Vec<Expanded>,
    total_rows: usize,
}

impl RowIndex {
    fn build(tree: &JsonTree, source: &Source<'_>, state: &TreeState) -> Self {
        let records = source.records();
        let visible = tree.visible_roots.clone();
        let count = visible.as_ref().map(|v| v.len() as u64).unwrap_or(records);

        // Walk the *expanded set*, not the records. It holds a handful of
        // entries where the file may hold tens of millions, and this runs every
        // frame -- iterating the records here would allocate a path per record
        // just to probe a set that almost always says no.
        let positions: Option<std::collections::HashMap<u64, usize>> = visible
            .as_ref()
            .map(|list| list.iter().enumerate().map(|(i, r)| (*r, i)).collect());

        let mut roots: Vec<(usize, u64)> = state
            .expanded
            .iter()
            // Record roots are bare indices; anything else is a path *within* a
            // record and is materialized by its record, not here.
            .filter_map(|path| path.parse::<u64>().ok())
            .filter_map(|root| match &positions {
                Some(by_root) => by_root.get(&root).map(|pos| (*pos, root)),
                None => (root < records).then_some((root as usize, root)),
            })
            .collect();
        // Rows are laid out in display order, so offsets accumulate in order.
        roots.sort_unstable();

        let mut expanded = Vec::with_capacity(roots.len());
        let mut extra = 0usize;
        for (pos, root) in roots {
            if !source.record_expandable(root) {
                continue;
            }
            let path = root.to_string();
            let mut rows = Vec::new();
            rows.push(record_row(root, true, source));
            build_children(source, state, root, "", &path, 1, &mut rows);
            rows.push(closing_row(root, &path, 0, "}"));

            let start = pos + extra;
            extra += rows.len() - 1; // a collapsed record already occupies one row
            expanded.push(Expanded { pos, start, rows });
        }

        Self {
            visible,
            records,
            expanded,
            total_rows: count as usize + extra,
        }
    }

    fn record_at(&self, pos: usize) -> u64 {
        self.visible
            .as_ref()
            .map(|v| v.get(pos).copied().unwrap_or(0))
            .unwrap_or(pos as u64)
    }

    /// Where a path currently sits in the row list, if it is on screen.
    ///
    /// Resolved from the path rather than by searching: a path names its
    /// record, the record's position is arithmetic, and only an expanded
    /// record's own rows -- a handful -- are scanned. Searching every row
    /// would mean synthesizing millions of them to answer a keypress.
    fn position_of(&self, path: &str) -> Option<usize> {
        let root: u64 = path.split(['.', '[']).next()?.parse().ok()?;
        let pos = match &self.visible {
            Some(list) => list.iter().position(|r| *r == root)?,
            None => (root < self.records).then_some(root as usize)?,
        };

        // An expanded record holds its own rows, including nested ones.
        if let Some(entry) = self.expanded.iter().find(|e| e.pos == pos) {
            return entry
                .rows
                .iter()
                .position(|row| row.path == path)
                .map(|offset| entry.start + offset);
        }
        // Otherwise it is a single collapsed row, shifted by whatever was
        // expanded before it.
        let shift: usize = self
            .expanded
            .iter()
            .take_while(|e| e.pos < pos)
            .map(|e| e.rows.len() - 1)
            .sum();
        Some(pos + shift)
    }

    /// The row at `index`, synthesized for collapsed records.
    fn row(&self, index: usize, source: &Source<'_>) -> Option<TreeRow> {
        // The last expanded record starting at or before this row.
        let hit = self.expanded.partition_point(|e| e.start <= index);
        if hit > 0 {
            let e = &self.expanded[hit - 1];
            if index < e.start + e.rows.len() {
                return Some(e.rows[index - e.start].clone());
            }
        }
        // Otherwise a collapsed record: subtract the rows expanded ones added.
        let consumed: usize = self.expanded[..hit].iter().map(|e| e.rows.len() - 1).sum();
        let pos = index.checked_sub(consumed)?;
        if pos as u64
            >= self
                .visible
                .as_ref()
                .map(|v| v.len() as u64)
                .unwrap_or(self.records)
        {
            return None;
        }
        Some(record_row(self.record_at(pos), false, source))
    }
}

/// The row for a record itself.
fn record_row(root: u64, expanded: bool, source: &Source<'_>) -> TreeRow {
    let path = root.to_string();
    let expandable = source.record_expandable(root);
    let text = if expandable {
        if expanded {
            format!("[{root}]: {{")
        } else {
            format!("[{root}]: (…) ")
        }
    } else {
        format!("[{root}]: {}", source.node_preview(root, ""))
    };
    TreeRow {
        root,
        rel: String::new(),
        label: root.to_string(),
        indent: 0,
        text,
        key_token: TextToken::Key,
        value_token: Some(if expandable {
            TextToken::Bracket
        } else {
            TextToken::Str
        }),
        caret: expandable.then_some(expanded),
        summary: expandable && !expanded,
        path,
    }
}

fn closing_row(root: u64, path: &str, indent: usize, bracket: &str) -> TreeRow {
    TreeRow {
        path: format!("{path}/_close"),
        root,
        rel: String::new(),
        label: String::new(),
        indent,
        text: bracket.to_string(),
        key_token: TextToken::Bracket,
        value_token: None,
        caret: None,
        summary: false,
    }
}

/// Append rows for the children of `rel`, recursing only into expanded nodes.
fn build_children(
    source: &Source<'_>,
    state: &TreeState,
    root: u64,
    rel: &str,
    path: &str,
    indent: usize,
    out: &mut Vec<TreeRow>,
) {
    for node in source.children(root, rel) {
        // The display path carries the record prefix; the relative path is
        // record-local and must not start with a separator.
        let child_rel = if rel.is_empty() {
            node.segment.trim_start_matches('.').to_string()
        } else {
            format!("{rel}{}", node.segment)
        };
        let child_path = format!("{path}{}", node.segment);
        let expandable = node.kind.is_expandable();
        let expanded = expandable && state.expanded.contains(&child_path);
        let is_index = node.segment.starts_with('[');

        let (open, empty) = if node.kind == NodeKind::List {
            ("[", "[]")
        } else {
            ("{", "{}")
        };
        let value_text = if expandable {
            if expanded { open } else { empty }
        } else {
            node.preview.as_str()
        };
        let text = if is_index {
            format!("[{}]: {}", node.label, value_text)
        } else {
            format!("\"{}\": {}", node.label, value_text)
        };

        out.push(TreeRow {
            path: child_path.clone(),
            root,
            rel: child_rel.clone(),
            label: node.label.clone(),
            indent,
            text,
            key_token: TextToken::Key,
            value_token: Some(node.token),
            caret: expandable.then_some(expanded),
            summary: expandable && !expanded,
        });

        if expanded {
            build_children(
                source,
                state,
                root,
                &child_rel,
                &child_path,
                indent + 1,
                out,
            );
            out.push(closing_row(
                root,
                &child_path,
                indent,
                if node.kind == NodeKind::List {
                    "]"
                } else {
                    "}"
                },
            ));
        }
    }
}

// ── Rendering ────────────────────────────────────────────────────────────────

impl JsonTree {
    /// Render the tree into the available area.
    pub fn show(&self, ui: &mut egui::Ui) -> JsonTreeOutput {
        let id = if self.id.is_empty() {
            "json-tree"
        } else {
            self.id.as_str()
        };
        let base_id = ui.make_persistent_id(id);
        let state_id = base_id.with("json_tree_state");
        let init_id = base_id.with("json_tree_init");

        let source = Source::new(self);
        let initialized: bool = ui.ctx().data(|d| d.get_temp(init_id).unwrap_or(false));
        let mut state: TreeState = if initialized {
            ui.ctx().data(|d| d.get_temp(state_id).unwrap_or_default())
        } else {
            let mut fresh = TreeState::default();
            // Expanding everything is opt-in: on a file-sized source it would
            // read the entire dataset just to lay out the first frame.
            if self.expand_all_initially {
                for root in 0..source.records() {
                    fresh.expanded.insert(root.to_string());
                }
            }
            fresh
        };

        let mut index = RowIndex::build(self, &source, &state);
        let mut toggle: Option<String> = None;
        let mut copied: Option<String> = None;

        let colors = ThemeColors::from_ctx(ui.ctx());
        let stripe = color_to_hex(with_alpha(colors.fg, ZEBRA_ALPHA));
        let guide = with_alpha(colors.fg, GUIDE_ALPHA);

        container(ui, self.framed, &colors, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            egui::ScrollArea::both()
                .auto_shrink([false, false])
                .show_rows(ui, ROW_HEIGHT, index.total_rows, |ui, range| {
                    for idx in range {
                        let Some(row) = index.row(idx, &source) else {
                            continue;
                        };
                        let selected = state.selected.as_deref() == Some(row.path.as_str());
                        let background = if selected {
                            Some(color_to_hex(with_alpha(colors.fg, ZEBRA_ALPHA * 3)))
                        } else if idx % 2 == 1 {
                            Some(stripe.clone())
                        } else {
                            None
                        };

                        // Indent guides, drawn before the row claims its rect.
                        if row.indent > 0 {
                            let rect = ui.available_rect_before_wrap();
                            let painter = ui.painter();
                            for level in 0..row.indent {
                                let x = rect.min.x + (level as f32 * INDENT_STEP) + 8.0;
                                painter.line_segment(
                                    [
                                        egui::pos2(x, rect.min.y),
                                        egui::pos2(x, rect.min.y + ROW_HEIGHT),
                                    ],
                                    egui::Stroke::new(1.0, guide),
                                );
                            }
                        }

                        let out = DataRow::builder()
                            .display_text(row.text.clone())
                            .row_id(row.path.clone())
                            .key_token(row.key_token)
                            .maybe_value_token(row.value_token)
                            .maybe_caret(row.caret)
                            .maybe_background(background)
                            .highlights(
                                self.highlights
                                    .get(&row.path)
                                    .cloned()
                                    .unwrap_or_else(RowHighlights::default),
                            )
                            .syntax_highlighting(true)
                            .summary_value(row.summary)
                            .indent(row.indent)
                            .build()
                            .show(ui);

                        if out.caret_clicked {
                            toggle = Some(row.path.clone());
                        } else if out.clicked {
                            state.selected = Some(row.path.clone());
                        }
                        // The menu reads the node, not the row: a rendered value
                        // is truncated for display, and copying the truncation
                        // would be quietly wrong.
                        out.response.context_menu(|ui| {
                            if let Some(text) = node_menu(ui, &row, &source) {
                                copied = Some(text);
                            }
                        });
                        if out.right_clicked {
                            state.selected = Some(row.path.clone());
                        }
                    }
                });
        });

        if let Some(path) = toggle
            && !state.expanded.remove(&path)
        {
            state.expanded.insert(path);
        }

        // Keyboard navigation, after the rows are known so a move can resolve
        // against what is actually on screen.
        if navigate(ui, &mut state, &index, &source) {
            index = RowIndex::build(self, &source, &state);
        }

        // A command from the host's configurable shortcuts. Same behaviour as
        // the keys and the menu, reached a different way.
        if let Some(action) = self.action {
            let (rebuilt, text) = self.apply(action, &mut state, &index, &source);
            if rebuilt {
                index = RowIndex::build(self, &source, &state);
            }
            if text.is_some() {
                copied = text;
            }
        }

        let output = JsonTreeOutput {
            selected: state.selected.clone(),
            copied: copied.clone(),
            row_count: index.total_rows,
        };

        if let Some(text) = copied {
            ui.ctx().copy_text(text);
        }

        ui.ctx().data_mut(|d| {
            d.insert_temp(state_id, state);
            d.insert_temp(init_id, true);
        });

        output
    }
}

/// Draw the tree inside the design's `.tree` container: a `bg` fill, a hairline
/// [`edge_stroke`], [`RADIUS_PANEL`] corners and 4px of inner padding. `framed`
/// is false when the tree is nested in a container that already owns those
/// corners.
fn container<R>(
    ui: &mut egui::Ui,
    framed: bool,
    colors: &ThemeColors,
    content: impl FnOnce(&mut egui::Ui) -> R,
) -> R {
    if !framed {
        return content(ui);
    }
    egui::Frame::NONE
        .fill(colors.bg)
        .stroke(edge_stroke(colors))
        .corner_radius(RADIUS_PANEL)
        .inner_margin(TREE_PAD)
        .show(ui, content)
        .inner
}

impl JsonTree {
    /// Carry out a host command, reporting whether rows changed and any text
    /// to copy.
    fn apply(
        &self,
        action: TreeAction,
        state: &mut TreeState,
        index: &RowIndex,
        source: &Source<'_>,
    ) -> (bool, Option<String>) {
        let current = state
            .selected
            .as_ref()
            .and_then(|path| index.position_of(path));
        let row = current.and_then(|r| index.row(r, source));

        match action {
            TreeAction::ExpandNode => {
                if let Some(path) = state.selected.clone()
                    && row.as_ref().is_some_and(|r| r.caret == Some(false))
                {
                    state.expanded.insert(path);
                    return (true, None);
                }
            }
            TreeAction::CollapseNode => {
                if let Some(path) = state.selected.clone() {
                    return (state.expanded.remove(&path), None);
                }
            }
            TreeAction::ExpandAll => {
                // Records only: expanding every node of every record would read
                // the whole document, which is what lazy reading exists to
                // avoid.
                for root in 0..source.records() {
                    state.expanded.insert(root.to_string());
                }
                return (true, None);
            }
            TreeAction::CollapseAll => {
                let had = !state.expanded.is_empty();
                state.expanded.clear();
                return (had, None);
            }
            TreeAction::MoveUp => {
                select_row(state, index, source, current.unwrap_or(0).saturating_sub(1));
            }
            TreeAction::MoveDown => {
                select_row(state, index, source, current.map(|r| r + 1).unwrap_or(0));
            }
            TreeAction::CopyKey => return (false, row.map(|r| r.label)),
            TreeAction::CopyPath => return (false, row.map(|r| r.path)),
            TreeAction::CopyValue | TreeAction::CopyObject => {
                return (
                    false,
                    row.and_then(|r| source.node_json(r.root, &r.rel))
                        .map(json_to_clipboard),
                );
            }
        }
        (false, None)
    }
}

/// How a node reads on the clipboard: a string pastes as its own text, and
/// anything else as JSON.
fn json_to_clipboard(value: Value) -> String {
    match value {
        Value::String(text) => text,
        other => serde_json::to_string_pretty(&other).unwrap_or_else(|_| other.to_string()),
    }
}

// ── Context menu ─────────────────────────────────────────────────────────────

/// The menu for one row, returning the text to copy.
///
/// Entries are offered by what the row actually is: a container has an object
/// to copy, a leaf has a value, and a closing bracket has neither.
fn node_menu(ui: &mut egui::Ui, row: &TreeRow, source: &Source<'_>) -> Option<String> {
    use crate::components::{ContextMenu, ContextMenuItem};

    let is_container = row.caret.is_some();
    let is_closer = row.label.is_empty() && row.caret.is_none();
    if is_closer {
        return None;
    }

    let items = vec![
        ContextMenuItem::builder()
            .label(if is_container {
                "Copy object"
            } else {
                "Copy value"
            })
            .shortcut("⌘C")
            .build(),
        ContextMenuItem::builder().label("Copy key").build(),
        ContextMenuItem::builder().label("Copy path").build(),
    ];

    let picked = ContextMenu::builder().items(items).build().show(ui)?;
    match picked {
        // Read the node, not the row -- the rendered value is truncated.
        0 => source.node_json(row.root, &row.rel).map(json_to_clipboard),
        1 => Some(row.label.clone()),
        2 => Some(row.path.clone()),
        _ => None,
    }
}

// ── Keyboard navigation ──────────────────────────────────────────────────────

/// Handle arrow-key movement and expansion.
///
/// Returns whether the row list needs rebuilding, which expansion changes do.
/// Left and right follow the tree rather than the list: right opens a closed
/// container and otherwise descends, left closes an open one and otherwise
/// climbs to its parent — which is what makes a keyboard walk feel like a tree
/// and not a table.
fn navigate(
    ui: &mut egui::Ui,
    state: &mut TreeState,
    index: &RowIndex,
    source: &Source<'_>,
) -> bool {
    use egui::Key;

    let keys: Vec<Key> = ui.input(|i| {
        [
            Key::ArrowUp,
            Key::ArrowDown,
            Key::ArrowLeft,
            Key::ArrowRight,
            Key::Home,
            Key::End,
        ]
        .into_iter()
        .filter(|k| i.key_pressed(*k))
        .collect()
    });
    if keys.is_empty() {
        return false;
    }

    let current = state
        .selected
        .as_ref()
        .and_then(|path| index.position_of(path));

    let mut rebuild = false;
    for key in keys {
        match key {
            Key::ArrowDown => {
                let next = current.map(|row| row + 1).unwrap_or(0);
                select_row(state, index, source, next);
            }
            Key::ArrowUp => {
                let previous = current.unwrap_or(0).saturating_sub(1);
                select_row(state, index, source, previous);
            }
            Key::Home => select_row(state, index, source, 0),
            Key::End => select_row(state, index, source, index.total_rows.saturating_sub(1)),
            Key::ArrowRight => {
                if let Some(path) = state.selected.clone()
                    && let Some(row) = current.and_then(|r| index.row(r, source))
                    && row.caret == Some(false)
                {
                    state.expanded.insert(path);
                    rebuild = true;
                } else if let Some(row) = current {
                    select_row(state, index, source, row + 1);
                }
            }
            Key::ArrowLeft => {
                if let Some(path) = state.selected.clone()
                    && state.expanded.remove(&path)
                {
                    rebuild = true;
                } else if let Some(parent) = state.selected.as_deref().and_then(parent_path) {
                    state.selected = Some(parent);
                }
            }
            _ => {}
        }
    }
    rebuild
}

fn select_row(state: &mut TreeState, index: &RowIndex, source: &Source<'_>, row: usize) {
    if let Some(row) = index.row(row.min(index.total_rows.saturating_sub(1)), source) {
        state.selected = Some(row.path);
    }
}

/// The path of a node's parent: `3.user.name` -> `3.user`, `3.tags[2]` -> `3.tags`.
fn parent_path(path: &str) -> Option<String> {
    let dot = path.rfind('.');
    let bracket = path.rfind('[');
    match (dot, bracket) {
        (None, None) => None,
        (a, b) => {
            let cut = a.into_iter().chain(b).max()?;
            (cut > 0).then(|| path[..cut].to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tree(value: Value, expanded: &[&str]) -> (JsonTree, TreeState) {
        let mut state = TreeState::default();
        for p in expanded {
            state.expanded.insert((*p).to_string());
        }
        (JsonTree::builder().value(value).build(), state)
    }

    fn records(n: usize) -> Value {
        Value::Array((0..n).map(|i| json!({ "n": i })).collect())
    }

    #[test]
    fn collapsed_records_are_one_row_each() {
        let (t, state) = tree(records(1000), &[]);
        let source = Source::new(&t);
        let index = RowIndex::build(&t, &source, &state);

        assert_eq!(index.total_rows, 1000);
        // Nothing was materialized for them.
        assert!(index.expanded.is_empty());
        assert_eq!(index.row(0, &source).unwrap().text, "[0]: (…) ");
        assert_eq!(index.row(999, &source).unwrap().text, "[999]: (…) ");
        assert!(index.row(1000, &source).is_none());
    }

    #[test]
    fn expanding_a_record_only_materializes_that_record() {
        let (t, state) = tree(records(1000), &["7"]);
        let source = Source::new(&t);
        let index = RowIndex::build(&t, &source, &state);

        // Record 7 becomes: header + one field + closer = 3 rows, so two extra.
        assert_eq!(index.total_rows, 1002);
        assert_eq!(index.expanded.len(), 1, "only the expanded record is built");

        // Rows before it are still the untouched collapsed records.
        assert_eq!(index.row(6, &source).unwrap().text, "[6]: (…) ");
        // Then the expanded record's own rows.
        assert_eq!(index.row(7, &source).unwrap().text, "[7]: {");
        assert_eq!(index.row(8, &source).unwrap().text, "\"n\": 7");
        assert_eq!(index.row(9, &source).unwrap().text, "}");
        // And the record after it has shifted by exactly the extra rows.
        assert_eq!(index.row(10, &source).unwrap().text, "[8]: (…) ");
        assert_eq!(index.row(1001, &source).unwrap().text, "[999]: (…) ");
    }

    #[test]
    fn several_expanded_records_stay_in_order() {
        let (t, state) = tree(records(100), &["2", "50", "99"]);
        let source = Source::new(&t);
        let index = RowIndex::build(&t, &source, &state);

        assert_eq!(index.total_rows, 100 + 3 * 2);
        assert_eq!(index.row(2, &source).unwrap().text, "[2]: {");
        // 50 is preceded by 47 collapsed rows plus record 2's two extra.
        assert_eq!(index.row(50 + 2, &source).unwrap().text, "[50]: {");
        assert_eq!(index.row(99 + 4, &source).unwrap().text, "[99]: {");
        assert_eq!(index.row(index.total_rows - 1, &source).unwrap().text, "}");
    }

    #[test]
    fn nested_expansion_indents_and_closes() {
        let value = Value::Array(vec![json!({ "user": { "name": "ada" } })]);
        let (t, state) = tree(value, &["0", "0.user"]);
        let source = Source::new(&t);
        let index = RowIndex::build(&t, &source, &state);

        let texts: Vec<String> = (0..index.total_rows)
            .map(|i| index.row(i, &source).unwrap().text)
            .collect();
        assert_eq!(
            texts,
            ["[0]: {", "\"user\": {", "\"name\": \"ada\"", "}", "}"]
        );
        assert_eq!(index.row(2, &source).unwrap().indent, 2);
    }

    #[test]
    fn a_visible_root_filter_selects_and_orders_records() {
        let mut t = JsonTree::builder().value(records(100)).build();
        t.visible_roots = Some(vec![9, 4, 1]);
        let state = TreeState::default();
        let source = Source::new(&t);
        let index = RowIndex::build(&t, &source, &state);

        assert_eq!(index.total_rows, 3);
        assert_eq!(index.row(0, &source).unwrap().text, "[9]: (…) ");
        assert_eq!(index.row(1, &source).unwrap().text, "[4]: (…) ");
        assert_eq!(index.row(2, &source).unwrap().text, "[1]: (…) ");
    }

    #[test]
    fn a_scalar_record_is_not_expandable() {
        let (t, state) = tree(json!(["hello", 42]), &[]);
        let source = Source::new(&t);
        let index = RowIndex::build(&t, &source, &state);

        assert_eq!(index.total_rows, 2);
        assert_eq!(index.row(0, &source).unwrap().text, "[0]: \"hello\"");
        assert!(index.row(0, &source).unwrap().caret.is_none());
        assert_eq!(index.row(1, &source).unwrap().text, "[1]: 42");
    }

    // ── A dataset far larger than anything that could be iterated ───────────
    //
    // Installs a stub access reporting ten million records and counting every
    // call, so a build that walks the records instead of the expanded set is a
    // test failure rather than a report of sluggishness.

    use std::sync::atomic::{AtomicUsize, Ordering};

    static HUGE_RECORDS: u64 = 10_000_000;
    static EXPANDABLE_CALLS: AtomicUsize = AtomicUsize::new(0);
    static CHILDREN_CALLS: AtomicUsize = AtomicUsize::new(0);

    fn install_huge_access() {
        use crate::dataset::{DatasetAccess, set_dataset_access};
        set_dataset_access(DatasetAccess {
            total: |_| HUGE_RECORDS,
            records_expandable: |_| {
                EXPANDABLE_CALLS.fetch_add(1, Ordering::Relaxed);
                true
            },
            children: |_, _, rel| {
                CHILDREN_CALLS.fetch_add(1, Ordering::Relaxed);
                if rel.is_empty() {
                    vec![TreeNode {
                        label: "id".into(),
                        segment: ".id".into(),
                        kind: NodeKind::Leaf,
                        preview: "1".into(),
                        token: TextToken::Number,
                    }]
                } else {
                    Vec::new()
                }
            },
            node_preview: |_, _, _| "x".to_string(),
            node_json: |_, _, _| None,
        });
    }

    /// The call counters are process-wide, so these tests take turns.
    static HUGE_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn huge_tree(expanded: &[&str]) -> (JsonTree, TreeState) {
        install_huge_access();
        let mut state = TreeState::default();
        for p in expanded {
            state.expanded.insert((*p).to_string());
        }
        let mut tree = JsonTree::builder().build();
        tree.handle = Some("huge".to_string());
        (tree, state)
    }

    #[test]
    fn building_ten_million_collapsed_records_touches_none_of_them() {
        let _guard = HUGE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let (tree, state) = huge_tree(&[]);
        let source = Source::new(&tree);
        EXPANDABLE_CALLS.store(0, Ordering::Relaxed);
        CHILDREN_CALLS.store(0, Ordering::Relaxed);

        let index = RowIndex::build(&tree, &source, &state);

        assert_eq!(index.total_rows, HUGE_RECORDS as usize);
        // The build must scale with what is expanded, not with what exists --
        // this ran once per record before, allocating a path each time.
        assert_eq!(EXPANDABLE_CALLS.load(Ordering::Relaxed), 0);
        assert_eq!(CHILDREN_CALLS.load(Ordering::Relaxed), 0);
    }

    #[test]
    fn expanding_two_records_reads_only_those_two() {
        let _guard = HUGE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let (tree, state) = huge_tree(&["5", "9000000"]);
        let source = Source::new(&tree);
        EXPANDABLE_CALLS.store(0, Ordering::Relaxed);
        CHILDREN_CALLS.store(0, Ordering::Relaxed);

        let index = RowIndex::build(&tree, &source, &state);

        assert_eq!(index.expanded.len(), 2);
        // Memoized: expandability is a schema property, so the registry is
        // consulted once no matter how many records are drawn.
        assert_eq!(EXPANDABLE_CALLS.load(Ordering::Relaxed), 1);
        // One `children` call per expanded record, plus one per leaf visited.
        assert!(CHILDREN_CALLS.load(Ordering::Relaxed) <= 4);
        // Each expanded record adds its field and a closing row.
        assert_eq!(index.total_rows, HUGE_RECORDS as usize + 4);
    }

    #[test]
    fn expanded_records_keep_their_place_however_they_were_inserted() {
        let _guard = HUGE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // The expanded set is unordered; row offsets must still accumulate in
        // display order.
        let (tree, state) = huge_tree(&["9000000", "5", "100"]);
        let source = Source::new(&tree);
        let index = RowIndex::build(&tree, &source, &state);

        let starts: Vec<usize> = index.expanded.iter().map(|e| e.start).collect();
        let mut sorted = starts.clone();
        sorted.sort_unstable();
        assert_eq!(starts, sorted, "offsets must be in display order");
        assert_eq!(index.row(5, &source).unwrap().text, "[5]: {");
    }

    #[test]
    fn a_nested_path_in_the_expanded_set_is_not_mistaken_for_a_record() {
        let _guard = HUGE_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        // "5.user" belongs to record 5 and is materialized by it -- treating it
        // as a root would double-count rows.
        let (tree, state) = huge_tree(&["5", "5.user"]);
        let source = Source::new(&tree);
        let index = RowIndex::build(&tree, &source, &state);
        assert_eq!(index.expanded.len(), 1);
    }

    // ── Locating a path, and walking with the keyboard ──────────────────────

    #[test]
    fn a_path_resolves_to_its_row_without_searching() {
        // The cheap-lookup property: a keypress on a ten-million-record file
        // must not synthesize rows to find the selection.
        let (tree, state) = huge_tree(&["5"]);
        let source = Source::new(&tree);
        let index = RowIndex::build(&tree, &source, &state);

        // A collapsed record sits at its own position, shifted by expansions.
        assert_eq!(index.position_of("0"), Some(0));
        assert_eq!(index.position_of("5"), Some(5));
        // Record 5 expands to header + field + closer, so 6 shifts by two.
        assert_eq!(index.position_of("6"), Some(8));
        assert_eq!(index.position_of("9000000"), Some(9_000_002));
        // And a node inside the expanded record is found among its own rows.
        assert_eq!(index.position_of("5.id"), Some(6));
    }

    #[test]
    fn an_unknown_path_resolves_to_nothing() {
        let (tree, state) = huge_tree(&[]);
        let source = Source::new(&tree);
        let index = RowIndex::build(&tree, &source, &state);

        assert_eq!(index.position_of("not-a-record"), None);
        assert_eq!(index.position_of(""), None);
        // Past the end of the dataset.
        assert_eq!(index.position_of("99999999999"), None);
    }

    #[test]
    fn a_parent_path_drops_one_segment() {
        assert_eq!(parent_path("3.user.name").as_deref(), Some("3.user"));
        assert_eq!(parent_path("3.tags[2]").as_deref(), Some("3.tags"));
        assert_eq!(parent_path("3.user").as_deref(), Some("3"));
        // A record is already the root; there is nowhere further up.
        assert_eq!(parent_path("3"), None);
    }
}
