//! Walking Arrow values as a tree, without materializing them as JSON.
//!
//! The tree viewer needs three things about a node: is it a container, what
//! are its children, and how does a leaf read. All three come straight off the
//! Arrow arrays here — a node is addressed by `(row, path)` and only the cells
//! actually on screen are ever formatted.
//!
//! That is what keeps a huge file cheap. The JSON form of a window is several
//! times the size of its Arrow buffers (every key repeated per row, every
//! scalar boxed), so the viewer never builds one; [`node_to_json`] exists only
//! for the edges that genuinely need JSON — clipboard, export, plugins.
//!
//! ## Paths
//!
//! A path is the viewer's addressing scheme, relative to a record: `""` is the
//! record itself, `user` a struct field, `items[2]` a list element,
//! `user.address.city` a nested field. Record `3`'s full path is `3.user`, and
//! [`split_root_rel`](crate::helpers::split_root_rel) splits the two halves.
//!
//! ## Nulls
//!
//! A null struct field is *skipped*, not shown as `null`. DuckDB unions the
//! schema across every row, so a field missing from one record is present-but-
//! null in Arrow; skipping nulls is what makes each record show the keys it
//! actually has. The cost is that an explicit `null` in the source is
//! indistinguishable from an absent key — the same trade the JSON path already
//! made, since Arrow's JSON writer omits nulls too.

use std::sync::Arc;

use duckdb::arrow::array::{
    Array, ArrayRef, LargeListArray, ListArray, RecordBatch, StructArray,
};
use duckdb::arrow::datatypes::DataType;
use duckdb::arrow::util::display::{ArrayFormatter, FormatOptions};
use serde_json::{Map, Value};
use thoth_plugin_sdk::tokens::TextToken;

/// Long leaf strings are truncated for the row view.
const MAX_PREVIEW: usize = 120;

// The node types are the SDK's, so a tree walked here can be handed straight to
// a `data-view` without translation. They carry only strings and a `TextToken`,
// which is why the SDK needs no Arrow dependency of its own.
pub use thoth_plugin_sdk::dataset::{NodeKind, TreeNode as ArrowNode};

/// Locate the batch holding absolute row `row`, and the row's index within it.
fn locate(batches: &[RecordBatch], row: usize) -> Option<(&RecordBatch, usize)> {
    let mut remaining = row;
    for batch in batches {
        if remaining < batch.num_rows() {
            return Some((batch, remaining));
        }
        remaining -= batch.num_rows();
    }
    None
}

/// A record is a struct of the schema's columns — treating it as one makes
/// every level of the walk uniform.
fn record_as_struct(batch: &RecordBatch) -> ArrayRef {
    Arc::new(StructArray::new(
        batch.schema().fields().clone(),
        batch.columns().to_vec(),
        None,
    ))
}

/// Resolve a path to the array holding it and the index within that array.
fn resolve(batches: &[RecordBatch], row: usize, rel: &str) -> Option<(ArrayRef, usize)> {
    let (batch, local) = locate(batches, row)?;
    let mut array = record_as_struct(batch);
    let mut index = local;

    for segment in parse_path(rel) {
        let (next_array, next_index) = match segment {
            Segment::Field(name) => {
                let structs = array.as_any().downcast_ref::<StructArray>()?;
                (structs.column_by_name(&name)?.clone(), index)
            }
            Segment::Index(i) => list_element(&array, index, i)?,
        };
        if next_array.is_null(next_index) {
            return None;
        }
        array = next_array;
        index = next_index;
    }

    Some((array, index))
}

/// The `i`th element of a list value at `index`.
fn list_element(array: &ArrayRef, index: usize, i: usize) -> Option<(ArrayRef, usize)> {
    if let Some(list) = array.as_any().downcast_ref::<ListArray>() {
        let offsets = list.value_offsets();
        let start = *offsets.get(index)? as usize;
        let end = *offsets.get(index + 1)? as usize;
        let child = start + i;
        (child < end).then(|| (list.values().clone(), child))
    } else if let Some(list) = array.as_any().downcast_ref::<LargeListArray>() {
        let offsets = list.value_offsets();
        let start = *offsets.get(index)? as usize;
        let end = *offsets.get(index + 1)? as usize;
        let child = start + i;
        (child < end).then(|| (list.values().clone(), child))
    } else {
        None
    }
}

/// Classify an array's element without reading it.
fn kind_of(array: &ArrayRef) -> NodeKind {
    match array.data_type() {
        DataType::Struct(_) => NodeKind::Struct,
        DataType::List(_) | DataType::LargeList(_) => NodeKind::List,
        _ => NodeKind::Leaf,
    }
}

/// The kind of the node at `rel` within `row`.
pub fn node_kind(batches: &[RecordBatch], row: usize, rel: &str) -> NodeKind {
    resolve(batches, row, rel)
        .map(|(array, _)| kind_of(&array))
        .unwrap_or(NodeKind::Leaf)
}

/// The children of the node at `rel` within `row`.
///
/// Reads only the cells it returns. Null struct fields are skipped — see the
/// module docs.
pub fn children(batches: &[RecordBatch], row: usize, rel: &str) -> Vec<ArrowNode> {
    let Some((array, index)) = resolve(batches, row, rel) else {
        return Vec::new();
    };

    if let Some(structs) = array.as_any().downcast_ref::<StructArray>() {
        let fields = match structs.data_type() {
            DataType::Struct(fields) => fields.clone(),
            _ => return Vec::new(),
        };
        return fields
            .iter()
            .enumerate()
            .filter_map(|(i, field)| {
                let child = structs.column(i);
                if child.is_null(index) {
                    return None; // absent key for this record
                }
                Some(node(field.name().clone(), format!(".{}", field.name()), child, index))
            })
            .collect();
    }

    if let Some(len) = list_len(&array, index) {
        return (0..len)
            .filter_map(|i| {
                let (child, child_index) = list_element(&array, index, i)?;
                Some(node(i.to_string(), format!("[{i}]"), &child, child_index))
            })
            .collect();
    }

    Vec::new()
}

fn list_len(array: &ArrayRef, index: usize) -> Option<usize> {
    if let Some(list) = array.as_any().downcast_ref::<ListArray>() {
        let offsets = list.value_offsets();
        Some((*offsets.get(index + 1)? - *offsets.get(index)?) as usize)
    } else if let Some(list) = array.as_any().downcast_ref::<LargeListArray>() {
        let offsets = list.value_offsets();
        Some((*offsets.get(index + 1)? - *offsets.get(index)?) as usize)
    } else {
        None
    }
}

fn node(label: String, segment: String, array: &ArrayRef, index: usize) -> ArrowNode {
    let kind = kind_of(array);
    let (preview, token) = if kind.is_expandable() {
        (String::new(), TextToken::Bracket)
    } else {
        leaf(array, index)
    };
    ArrowNode {
        label,
        segment,
        kind,
        preview,
        token,
    }
}

/// Format a single leaf cell the way the JSON view would render it.
fn leaf(array: &ArrayRef, index: usize) -> (String, TextToken) {
    if array.is_null(index) {
        return ("null".to_string(), TextToken::Boolean);
    }

    match array.data_type() {
        DataType::Boolean => (format_raw(array, index), TextToken::Boolean),
        dt if dt.is_numeric() => (format_raw(array, index), TextToken::Number),
        DataType::Utf8 | DataType::LargeUtf8 | DataType::Utf8View => {
            (quote(&format_raw(array, index)), TextToken::Str)
        }
        // Dates, timestamps, blobs and anything else render as strings, which
        // is what the Arrow JSON writer does too.
        _ => (quote(&format_raw(array, index)), TextToken::Str),
    }
}

fn format_raw(array: &ArrayRef, index: usize) -> String {
    let opts = FormatOptions::default().with_null("null");
    match ArrayFormatter::try_new(array.as_ref(), &opts) {
        Ok(formatter) => formatter.value(index).to_string(),
        Err(_) => String::new(),
    }
}

/// Escape and truncate a string the way the row view expects.
fn quote(raw: &str) -> String {
    let escaped = raw.replace('\\', "\\\\").replace('"', "\\\"");
    if escaped.len() > MAX_PREVIEW {
        let cut = escaped
            .char_indices()
            .take_while(|(i, _)| *i < MAX_PREVIEW)
            .last()
            .map(|(i, c)| i + c.len_utf8())
            .unwrap_or(0);
        format!("\"{}…\"", &escaped[..cut])
    } else {
        format!("\"{escaped}\"")
    }
}

/// The formatted text of a leaf node, without building its parent.
pub fn node_preview(batches: &[RecordBatch], row: usize, rel: &str) -> String {
    match resolve(batches, row, rel) {
        Some((array, index)) => leaf(&array, index).0,
        None => "null".to_string(),
    }
}

/// The subtree at `rel` as JSON.
///
/// This is the edge conversion — clipboard, export, plugin rendering. The
/// viewer never calls it to draw a row.
pub fn node_to_json(batches: &[RecordBatch], row: usize, rel: &str) -> Option<Value> {
    let (array, index) = resolve(batches, row, rel)?;
    Some(to_json(&array, index))
}

fn to_json(array: &ArrayRef, index: usize) -> Value {
    if array.is_null(index) {
        return Value::Null;
    }

    if let Some(structs) = array.as_any().downcast_ref::<StructArray>() {
        let DataType::Struct(fields) = structs.data_type() else {
            return Value::Null;
        };
        let mut map = Map::new();
        for (i, field) in fields.iter().enumerate() {
            let child = structs.column(i);
            if child.is_null(index) {
                continue; // absent key
            }
            map.insert(field.name().clone(), to_json(child, index));
        }
        return Value::Object(map);
    }

    if let Some(len) = list_len(array, index) {
        let items = (0..len)
            .filter_map(|i| {
                let (child, child_index) = list_element(array, index, i)?;
                Some(to_json(&child, child_index))
            })
            .collect();
        return Value::Array(items);
    }

    match array.data_type() {
        DataType::Boolean => Value::Bool(format_raw(array, index) == "true"),
        dt if dt.is_numeric() => serde_json::from_str(&format_raw(array, index))
            .unwrap_or_else(|_| Value::String(format_raw(array, index))),
        _ => Value::String(format_raw(array, index)),
    }
}

// ── Path parsing ─────────────────────────────────────────────────────────────

enum Segment {
    Field(String),
    Index(usize),
}

/// Split a relative path into its segments: `user.tags[2]` →
/// `[Field("user"), Field("tags"), Index(2)]`.
fn parse_path(rel: &str) -> Vec<Segment> {
    let mut segments = Vec::new();
    let mut current = String::new();
    let mut chars = rel.chars().peekable();

    while let Some(ch) = chars.next() {
        match ch {
            '.' => {
                if !current.is_empty() {
                    segments.push(Segment::Field(std::mem::take(&mut current)));
                }
            }
            '[' => {
                if !current.is_empty() {
                    segments.push(Segment::Field(std::mem::take(&mut current)));
                }
                let mut digits = String::new();
                for d in chars.by_ref() {
                    if d == ']' {
                        break;
                    }
                    digits.push(d);
                }
                if let Ok(i) = digits.parse::<usize>() {
                    segments.push(Segment::Index(i));
                }
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        segments.push(Segment::Field(current));
    }
    segments
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::file::loaders::{DuckdbConnection, FileLoader};
    use std::io::Write;
    use tempfile::NamedTempFile;

    fn ndjson(lines: &str) -> NamedTempFile {
        let mut tmp = tempfile::Builder::new()
            .suffix(".ndjson")
            .tempfile()
            .unwrap();
        tmp.write_all(lines.as_bytes()).unwrap();
        tmp.flush().unwrap();
        tmp
    }

    fn batches(lines: &str) -> Vec<RecordBatch> {
        let file = ndjson(lines);
        let db = DuckdbConnection::open_path(file.path()).unwrap();
        db.fetch(Vec::new(), None, None).unwrap()
    }

    #[test]
    fn record_children_are_its_columns() {
        let b = batches("{\"a\":1,\"b\":\"x\"}\n");
        let kids = children(&b, 0, "");

        assert_eq!(
            kids.iter().map(|n| n.label.as_str()).collect::<Vec<_>>(),
            ["a", "b"]
        );
        assert_eq!(kids[0].preview, "1");
        assert_eq!(kids[0].token, TextToken::Number);
        assert_eq!(kids[1].preview, "\"x\"");
        assert_eq!(kids[1].token, TextToken::Str);
        assert!(kids.iter().all(|n| n.kind == NodeKind::Leaf));
    }

    #[test]
    fn structs_and_lists_are_expandable_and_walkable() {
        let b = batches("{\"user\":{\"name\":\"ada\"},\"tags\":[\"x\",\"y\"]}\n");

        let kids = children(&b, 0, "");
        assert_eq!(kids[0].kind, NodeKind::Struct);
        assert_eq!(kids[0].segment, ".user");
        assert_eq!(kids[1].kind, NodeKind::List);

        let user = children(&b, 0, "user");
        assert_eq!(user[0].label, "name");
        assert_eq!(user[0].preview, "\"ada\"");

        let tags = children(&b, 0, "tags");
        assert_eq!(tags.len(), 2);
        assert_eq!(tags[0].segment, "[0]");
        assert_eq!(tags[1].preview, "\"y\"");

        assert_eq!(node_kind(&b, 0, "user"), NodeKind::Struct);
        assert_eq!(node_kind(&b, 0, "tags"), NodeKind::List);
        assert_eq!(node_kind(&b, 0, "user.name"), NodeKind::Leaf);
    }

    #[test]
    fn nested_paths_resolve() {
        let b = batches("{\"user\":{\"address\":{\"city\":\"NYC\"}},\"m\":[{\"k\":7}]}\n");

        assert_eq!(children(&b, 0, "user.address")[0].preview, "\"NYC\"");
        assert_eq!(children(&b, 0, "m[0]")[0].label, "k");
        assert_eq!(children(&b, 0, "m[0]")[0].preview, "7");
    }

    #[test]
    fn absent_keys_are_skipped_per_record() {
        // DuckDB unions the schema, so row 0 has a null `b` — which is the
        // same thing as `b` being absent from that record.
        let b = batches("{\"a\":1}\n{\"b\":\"x\"}\n");

        assert_eq!(
            children(&b, 0, "")
                .iter()
                .map(|n| n.label.as_str())
                .collect::<Vec<_>>(),
            ["a"]
        );
        assert_eq!(
            children(&b, 1, "")
                .iter()
                .map(|n| n.label.as_str())
                .collect::<Vec<_>>(),
            ["b"]
        );
    }

    #[test]
    fn json_extraction_matches_the_tree() {
        let b = batches("{\"user\":{\"name\":\"ada\",\"age\":36},\"tags\":[\"x\",\"y\"]}\n");

        let whole = node_to_json(&b, 0, "").unwrap();
        assert_eq!(whole["user"]["name"], "ada");
        assert_eq!(whole["user"]["age"], 36);
        assert_eq!(whole["tags"][1], "y");

        let subtree = node_to_json(&b, 0, "user").unwrap();
        assert_eq!(subtree["name"], "ada");
        assert!(subtree.get("tags").is_none());
    }

    #[test]
    fn json_extraction_omits_absent_keys() {
        let b = batches("{\"a\":1}\n{\"b\":\"x\"}\n");
        let row = node_to_json(&b, 0, "").unwrap();
        assert_eq!(row, serde_json::json!({"a": 1}));
    }

    #[test]
    fn long_strings_are_truncated_in_previews_but_not_in_json() {
        let long = "z".repeat(300);
        let b = batches(&format!("{{\"s\":\"{long}\"}}\n"));

        let preview = &children(&b, 0, "")[0].preview;
        assert!(preview.ends_with("…\""), "got: {preview}");
        assert!(preview.len() < 200);

        // The clipboard/export path keeps the whole value.
        assert_eq!(node_to_json(&b, 0, "s").unwrap(), Value::String(long));
    }

    #[test]
    fn a_row_beyond_the_batches_yields_nothing() {
        let b = batches("{\"a\":1}\n");
        assert!(children(&b, 9, "").is_empty());
        assert!(node_to_json(&b, 9, "").is_none());
    }

    #[test]
    fn paths_parse_into_fields_and_indices() {
        let parsed = parse_path("user.tags[2].name");
        assert_eq!(parsed.len(), 4);
        assert!(matches!(&parsed[0], Segment::Field(f) if f == "user"));
        assert!(matches!(&parsed[1], Segment::Field(f) if f == "tags"));
        assert!(matches!(parsed[2], Segment::Index(2)));
        assert!(matches!(&parsed[3], Segment::Field(f) if f == "name"));
        assert!(parse_path("").is_empty());
    }
}
