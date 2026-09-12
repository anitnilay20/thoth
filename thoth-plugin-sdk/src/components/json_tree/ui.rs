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

use super::{JsonTree, JsonTreeOutput};

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
enum Source<'a> {
    Inline(&'a Value),
    Handle {
        handle: &'a str,
        access: &'static DatasetAccess,
    },
}

impl<'a> Source<'a> {
    fn new(tree: &'a JsonTree) -> Self {
        match (tree.handle.as_deref(), dataset_access()) {
            (Some(handle), Some(access)) => Source::Handle { handle, access },
            _ => Source::Inline(&tree.value),
        }
    }

    /// How many records the tree has. An inline array is a list of records;
    /// any other inline value is a single record.
    fn records(&self) -> u64 {
        match self {
            Source::Inline(Value::Array(items)) => items.len() as u64,
            Source::Inline(_) => 1,
            Source::Handle { handle, access } => (access.total)(handle),
        }
    }

    /// Whether records can be expanded. Handle-backed sources answer from the
    /// schema, so this costs no read.
    fn record_expandable(&self, root: u64) -> bool {
        match self {
            Source::Inline(_) => matches!(
                self.record(root),
                Some(Value::Object(_)) | Some(Value::Array(_))
            ),
            Source::Handle { handle, access } => (access.records_expandable)(handle),
        }
    }

    fn record(&self, root: u64) -> Option<&'a Value> {
        match self {
            Source::Inline(Value::Array(items)) => items.get(root as usize),
            Source::Inline(value) => (root == 0).then_some(*value),
            Source::Handle { .. } => None,
        }
    }

    fn children(&self, root: u64, rel: &str) -> Vec<TreeNode> {
        match self {
            Source::Inline(_) => self
                .record(root)
                .and_then(|record| walk(record, rel))
                .map(value_children)
                .unwrap_or_default(),
            Source::Handle { handle, access } => (access.children)(handle, root, rel),
        }
    }

    fn node_preview(&self, root: u64, rel: &str) -> String {
        match self {
            Source::Inline(_) => self
                .record(root)
                .and_then(|record| walk(record, rel))
                .map(scalar_text)
                .unwrap_or_else(|| "null".to_string()),
            Source::Handle { handle, access } => (access.node_preview)(handle, root, rel),
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
        let count = visible
            .as_ref()
            .map(|v| v.len() as u64)
            .unwrap_or(records)
            .min(records.max(visible.as_ref().map(|v| v.len() as u64).unwrap_or(0)));
        let mut expanded = Vec::new();
        let mut extra = 0usize;
        for pos in 0..count as usize {
            let root = visible
                .as_ref()
                .map(|v| v[pos])
                .unwrap_or(pos as u64);
            let path = root.to_string();
            if !state.expanded.contains(&path) {
                continue;
            }
            if !source.record_expandable(root) {
                continue;
            }
            let mut rows = Vec::new();
            rows.push(record_row(root, true, source));
            build_children(source, state, root, "", &path, 1, &mut rows);
            rows.push(closing_row(&path, 0, "}"));

            let start = pos + extra;
            extra += rows.len() - 1; // a collapsed record already occupies one row
            expanded.push(Expanded { start, rows });
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
        let consumed: usize = self.expanded[..hit]
            .iter()
            .map(|e| e.rows.len() - 1)
            .sum();
        let pos = index.checked_sub(consumed)?;
        if pos as u64 >= self.visible.as_ref().map(|v| v.len() as u64).unwrap_or(self.records) {
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

fn closing_row(path: &str, indent: usize, bracket: &str) -> TreeRow {
    TreeRow {
        path: format!("{path}/_close"),
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
                &child_path,
                indent,
                if node.kind == NodeKind::List { "]" } else { "}" },
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

        let index = RowIndex::build(self, &source, &state);
        let mut toggle: Option<String> = None;
        let mut context_menu_at: Option<String> = None;

        let colors = ThemeColors::from_ctx(ui.ctx());
        let stripe = color_to_hex(with_alpha(colors.fg, ZEBRA_ALPHA));
        let guide = with_alpha(colors.fg, GUIDE_ALPHA);

        container(ui, self.framed, &colors, |ui| {
            ui.spacing_mut().item_spacing.y = 0.0;
            egui::ScrollArea::both().auto_shrink([false, false]).show_rows(
                ui,
                ROW_HEIGHT,
                index.total_rows,
                |ui, range| {
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
                        if out.right_clicked {
                            state.selected = Some(row.path.clone());
                            context_menu_at = Some(row.path.clone());
                        }
                    }
                },
            );
        });

        if let Some(path) = toggle
            && !state.expanded.remove(&path)
        {
            state.expanded.insert(path);
        }

        let output = JsonTreeOutput {
            selected: state.selected.clone(),
            context_menu_at,
            row_count: index.total_rows,
        };

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
}
