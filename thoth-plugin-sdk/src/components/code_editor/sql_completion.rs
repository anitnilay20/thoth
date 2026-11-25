//! Context-aware SQL autocomplete classifier, alias resolver, and ranker (#121).
//!
//! Runs host-side in `CodeEditor::show` with zero WASM round-trips.

use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

/// A table and its known columns, passed from the plugin to `CodeEditor`.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionTable {
    /// Table name.
    pub name: String,
    /// Known column names for this table.
    #[serde(default)]
    pub columns: Vec<String>,
}

/// Structured schema for context-ranked SQL completion.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompletionSchema {
    /// Tables and their columns in the active database.
    #[serde(default)]
    pub tables: Vec<CompletionTable>,
    /// SQL function names.
    #[serde(default)]
    pub functions: Vec<String>,
    /// SQL keyword names.
    #[serde(default)]
    pub keywords: Vec<String>,
}

/// Category of a SQL completion item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionKind {
    /// Database table.
    Table,
    /// Table column.
    Column,
    /// SQL function.
    Function,
    /// SQL keyword.
    Keyword,
}

impl CompletionKind {
    /// Short monospace badge text for the popup row.
    pub fn badge(self) -> &'static str {
        match self {
            Self::Table => "TBL",
            Self::Column => "COL",
            Self::Function => "FN",
            Self::Keyword => "KW",
        }
    }
}

/// A single ranked completion candidate.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CompletionItem {
    /// Suggested identifier or keyword.
    pub label: String,
    /// Candidate category.
    pub kind: CompletionKind,
    /// Optional table name shown next to column candidates.
    pub detail: Option<String>,
}

/// Syntactic SQL context at the caret.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompletionContext {
    /// After `FROM`, `JOIN`, `UPDATE`, `INTO` — suggest tables.
    TableRef,
    /// After `alias.` or `table.` — suggest that table's columns.
    ColumnOf(String),
    /// After `SELECT`, `WHERE`, `ON`, `AND`, `OR`, `ORDER BY`, `GROUP BY`, `,` — suggest in-scope columns + functions.
    ColumnInScope,
    /// Statement start or fallback — suggest SQL keywords.
    Keyword,
    /// Inside a string literal or comment — suppress completion.
    Suppress,
}

/// Return the `;`-delimited statement slice containing `cursor_byte` and the
/// cursor offset relative to that slice.
fn current_statement(text: &str, cursor_byte: usize) -> (&str, usize) {
    let clamped = cursor_byte.min(text.len());
    let before = &text[..clamped];
    let start = before.rfind(';').map(|i| i + 1).unwrap_or(0);
    let end = text[clamped..]
        .find(';')
        .map(|i| clamped + i)
        .unwrap_or(text.len());
    (&text[start..end], clamped - start)
}

/// True when `cursor` inside `stmt` is inside a `'...'` string or `--` / `/* */` comment.
fn in_string_or_comment(stmt: &str, cursor: usize) -> bool {
    let bytes = stmt.as_bytes();
    let mut i = 0;
    let mut in_single = false;
    let mut in_line_comment = false;
    let mut in_block_comment = false;

    while i < cursor && i < bytes.len() {
        let b = bytes[i];
        if in_line_comment {
            if b == b'\n' {
                in_line_comment = false;
            }
            i += 1;
            continue;
        }
        if in_block_comment {
            if b == b'*' && bytes.get(i + 1) == Some(&b'/') {
                in_block_comment = false;
                i += 2;
            } else {
                i += 1;
            }
            continue;
        }
        if in_single {
            if b == b'\'' {
                if bytes.get(i + 1) == Some(&b'\'') {
                    i += 2;
                    continue;
                }
                in_single = false;
            }
            i += 1;
            continue;
        }
        if b == b'\'' {
            in_single = true;
            i += 1;
        } else if b == b'-' && bytes.get(i + 1) == Some(&b'-') {
            in_line_comment = true;
            i += 2;
        } else if b == b'/' && bytes.get(i + 1) == Some(&b'*') {
            in_block_comment = true;
            i += 2;
        } else {
            i += 1;
        }
    }

    in_single || in_line_comment || in_block_comment
}

fn is_ident_char(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

/// Classify the completion context and extract the identifier prefix at `cursor_byte`.
pub fn classify_context(text: &str, cursor_byte: usize) -> (CompletionContext, String) {
    let (stmt, cursor) = current_statement(text, cursor_byte);
    if in_string_or_comment(stmt, cursor) {
        return (CompletionContext::Suppress, String::new());
    }

    let before = &stmt[..cursor];
    let bytes = before.as_bytes();

    // Extract identifier prefix immediately before the caret.
    let mut prefix_start = bytes.len();
    while prefix_start > 0 && is_ident_char(bytes[prefix_start - 1]) {
        prefix_start -= 1;
    }
    let prefix = before[prefix_start..].to_string();

    // Check for `qualifier.prefix` (e.g. `o.id` or `orders.`).
    if prefix_start > 0 && bytes[prefix_start - 1] == b'.' {
        let dot_pos = prefix_start - 1;
        let mut qual_start = dot_pos;
        while qual_start > 0 && is_ident_char(bytes[qual_start - 1]) {
            qual_start -= 1;
        }
        if qual_start < dot_pos {
            let qualifier = before[qual_start..dot_pos].to_string();
            return (CompletionContext::ColumnOf(qualifier), prefix);
        }
    }

    // Tokenize the preceding text (uppercased) to find the nearest clause keyword.
    let pre = before[..prefix_start].trim_end();
    let ends_with_comma = pre.ends_with(',');
    let tokens: Vec<String> = pre
        .split(|c: char| !c.is_ascii_alphanumeric() && c != '_')
        .filter(|t| !t.is_empty())
        .map(|t| t.to_ascii_uppercase())
        .collect();

    if tokens.is_empty() {
        return (CompletionContext::Keyword, prefix);
    }

    for tok in tokens.iter().rev() {
        match tok.as_str() {
            "FROM" | "JOIN" | "INTO" | "UPDATE" => {
                // If user typed `SELECT * FROM users, ` or `FROM `, it's a table ref.
                if ends_with_comma || tok == tokens.last().unwrap() {
                    return (CompletionContext::TableRef, prefix);
                }
                return (CompletionContext::TableRef, prefix);
            }
            "SELECT" | "WHERE" | "ON" | "AND" | "OR" | "SET" | "HAVING" | "BY" => {
                return (CompletionContext::ColumnInScope, prefix);
            }
            _ => {}
        }
    }

    (CompletionContext::Keyword, prefix)
}

/// Extract `alias -> table_name` (and `table_name -> table_name`) from `FROM`/`JOIN` clauses.
pub fn extract_aliases(stmt: &str) -> HashMap<String, String> {
    let mut map = HashMap::new();
    let words: Vec<&str> = stmt
        .split(|c: char| c.is_whitespace() || matches!(c, ',' | '(' | ')' | ';'))
        .filter(|w| !w.is_empty())
        .collect();

    let mut i = 0;
    while i < words.len() {
        let upper = words[i].to_ascii_uppercase();
        if matches!(upper.as_str(), "FROM" | "JOIN")
            && let Some(&table) = words.get(i + 1)
        {
            let table_clean = table.split('.').next_back().unwrap_or(table);
            map.insert(table_clean.to_ascii_lowercase(), table_clean.to_string());
            if let Some(&next) = words.get(i + 2) {
                let next_upper = next.to_ascii_uppercase();
                if next_upper == "AS" {
                    if let Some(&alias) = words.get(i + 3) {
                        map.insert(alias.to_ascii_lowercase(), table_clean.to_string());
                        i += 4;
                        continue;
                    }
                } else if !matches!(
                    next_upper.as_str(),
                    "WHERE"
                        | "JOIN"
                        | "INNER"
                        | "LEFT"
                        | "RIGHT"
                        | "FULL"
                        | "CROSS"
                        | "ON"
                        | "GROUP"
                        | "ORDER"
                        | "LIMIT"
                        | "UNION"
                        | "HAVING"
                        | "SET"
                ) {
                    map.insert(next.to_ascii_lowercase(), table_clean.to_string());
                    i += 3;
                    continue;
                }
            }
        }
        i += 1;
    }
    map
}

fn matches_prefix(candidate: &str, prefix_lower: &str) -> Option<bool> {
    if prefix_lower.is_empty() {
        return Some(true);
    }
    let cand_lower = candidate.to_ascii_lowercase();
    if cand_lower.starts_with(prefix_lower) {
        Some(true)
    } else if cand_lower.contains(prefix_lower) {
        Some(false)
    } else {
        None
    }
}

fn push_ranked(
    out: &mut Vec<(bool, CompletionItem)>,
    seen: &mut HashSet<String>,
    label: &str,
    kind: CompletionKind,
    detail: Option<String>,
    prefix_lower: &str,
) {
    let key = format!("{}:{label}", kind.badge());
    if let Some(is_prefix) = matches_prefix(label, prefix_lower)
        && seen.insert(key)
    {
        out.push((
            is_prefix,
            CompletionItem {
                label: label.to_string(),
                kind,
                detail,
            },
        ));
    }
}

/// Rank SQL completion candidates for `text` at `cursor_byte`.
pub fn rank_candidates(
    schema: &CompletionSchema,
    text: &str,
    cursor_byte: usize,
) -> Vec<CompletionItem> {
    let (ctx, prefix) = classify_context(text, cursor_byte);
    if ctx == CompletionContext::Suppress {
        return Vec::new();
    }

    let prefix_lower = prefix.to_ascii_lowercase();
    let (stmt, _) = current_statement(text, cursor_byte);
    let aliases = extract_aliases(stmt);

    let mut ranked: Vec<(bool, CompletionItem)> = Vec::new();
    let mut seen = HashSet::new();

    match ctx {
        CompletionContext::Suppress => return Vec::new(),
        CompletionContext::ColumnOf(qualifier) => {
            let q_lower = qualifier.to_ascii_lowercase();
            let target_table = aliases.get(&q_lower).map(String::as_str).unwrap_or(&qualifier);
            for t in &schema.tables {
                if t.name.eq_ignore_ascii_case(target_table) {
                    for col in &t.columns {
                        push_ranked(
                            &mut ranked,
                            &mut seen,
                            col,
                            CompletionKind::Column,
                            Some(t.name.clone()),
                            &prefix_lower,
                        );
                    }
                }
            }
        }
        CompletionContext::TableRef => {
            for t in &schema.tables {
                push_ranked(
                    &mut ranked,
                    &mut seen,
                    &t.name,
                    CompletionKind::Table,
                    None,
                    &prefix_lower,
                );
            }
        }
        CompletionContext::ColumnInScope => {
            let scoped_tables: HashSet<String> = aliases
                .values()
                .map(|t| t.to_ascii_lowercase())
                .collect();
            // Columns of tables in FROM/JOIN first.
            for t in &schema.tables {
                if scoped_tables.is_empty() || scoped_tables.contains(&t.name.to_ascii_lowercase())
                {
                    for col in &t.columns {
                        push_ranked(
                            &mut ranked,
                            &mut seen,
                            col,
                            CompletionKind::Column,
                            Some(t.name.clone()),
                            &prefix_lower,
                        );
                    }
                }
            }
            for f in &schema.functions {
                push_ranked(
                    &mut ranked,
                    &mut seen,
                    f,
                    CompletionKind::Function,
                    None,
                    &prefix_lower,
                );
            }
            for kw in &schema.keywords {
                push_ranked(
                    &mut ranked,
                    &mut seen,
                    kw,
                    CompletionKind::Keyword,
                    None,
                    &prefix_lower,
                );
            }
        }
        CompletionContext::Keyword => {
            for kw in &schema.keywords {
                push_ranked(
                    &mut ranked,
                    &mut seen,
                    kw,
                    CompletionKind::Keyword,
                    None,
                    &prefix_lower,
                );
            }
            for t in &schema.tables {
                push_ranked(
                    &mut ranked,
                    &mut seen,
                    &t.name,
                    CompletionKind::Table,
                    None,
                    &prefix_lower,
                );
            }
        }
    }

    // Exact prefix matches first, preserving category order.
    ranked.sort_by(|(a_pref, _), (b_pref, _)| b_pref.cmp(a_pref));
    ranked.into_iter().map(|(_, item)| item).take(12).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_schema() -> CompletionSchema {
        CompletionSchema {
            tables: vec![
                CompletionTable {
                    name: "orders".to_string(),
                    columns: vec!["id".into(), "user_id".into(), "total_cents".into()],
                },
                CompletionTable {
                    name: "users".to_string(),
                    columns: vec!["id".into(), "email".into()],
                },
            ],
            functions: vec!["COUNT".into(), "COALESCE".into()],
            keywords: vec!["SELECT".into(), "FROM".into(), "WHERE".into()],
        }
    }

    #[test]
    fn classifies_table_ref_and_suppresses_strings() {
        assert_eq!(
            classify_context("SELECT * FROM us", 16),
            (CompletionContext::TableRef, "us".to_string())
        );
        assert_eq!(
            classify_context("SELECT 'FROM ", 13),
            (CompletionContext::Suppress, String::new())
        );
        assert_eq!(
            classify_context("-- FROM us", 10),
            (CompletionContext::Suppress, String::new())
        );
    }

    #[test]
    fn resolves_alias_for_dot_completion() {
        let sql = "SELECT o. FROM orders o JOIN users u ON o.user_id = u.id";
        let items = rank_candidates(&sample_schema(), sql, 9);
        let labels: Vec<&str> = items.iter().map(|i| i.label.as_str()).collect();
        assert_eq!(labels, vec!["id", "user_id", "total_cents"]);
        assert!(items.iter().all(|i| i.kind == CompletionKind::Column));
    }

    #[test]
    fn ranks_in_scope_columns_after_select_and_tables_after_from() {
        let from_items = rank_candidates(&sample_schema(), "SELECT * FROM o", 15);
        assert_eq!(from_items[0].label, "orders");
        assert_eq!(from_items[0].kind, CompletionKind::Table);

        let sel_items = rank_candidates(&sample_schema(), "SELECT em FROM users", 9);
        assert_eq!(sel_items[0].label, "email");
        assert_eq!(sel_items[0].kind, CompletionKind::Column);
    }
}
