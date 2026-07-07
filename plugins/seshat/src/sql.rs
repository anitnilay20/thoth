//! Minimal SQL statement splitting for "run the statement under the cursor" and
//! the per-statement ▶ run-markers. Offsets are CHARACTER indices, matching the
//! editor's caret / marker offsets.

/// A top-level statement's character range: `start` is its first non-whitespace
/// char (the ▶ / caret anchor), `end` is one past its `;` terminator (or EOF).
pub(crate) struct Stmt {
    pub start: usize,
    pub end: usize,
}

#[derive(PartialEq)]
enum State {
    Normal,
    Single,      // '...'
    Double,      // "..."
    Line,        // -- ...
    Block,       // /* ... */
    Dollar,      // $tag$ ... $tag$
}

/// Split `sql` into top-level statements on `;`, ignoring separators inside
/// string/identifier literals, comments, and Postgres dollar-quoted bodies.
/// Whitespace-only segments are dropped.
pub(crate) fn statements(sql: &str) -> Vec<Stmt> {
    let chars: Vec<char> = sql.chars().collect();
    let n = chars.len();
    let mut out = Vec::new();
    let mut seg_start = 0usize;
    let mut state = State::Normal;
    let mut tag: String = String::new();
    let mut i = 0usize;

    while i < n {
        let c = chars[i];
        match state {
            State::Normal => match c {
                '\'' => state = State::Single,
                '"' => state = State::Double,
                '-' if chars.get(i + 1) == Some(&'-') => {
                    state = State::Line;
                    i += 1;
                }
                '/' if chars.get(i + 1) == Some(&'*') => {
                    state = State::Block;
                    i += 1;
                }
                '$' => {
                    if let Some((t, len)) = dollar_open(&chars, i) {
                        tag = t;
                        state = State::Dollar;
                        i += len - 1;
                    }
                }
                ';' => {
                    push(&chars, seg_start, i + 1, &mut out);
                    seg_start = i + 1;
                }
                _ => {}
            },
            State::Single => {
                if c == '\'' {
                    if chars.get(i + 1) == Some(&'\'') {
                        i += 1; // escaped ''
                    } else {
                        state = State::Normal;
                    }
                }
            }
            State::Double => {
                if c == '"' {
                    if chars.get(i + 1) == Some(&'"') {
                        i += 1; // escaped ""
                    } else {
                        state = State::Normal;
                    }
                }
            }
            State::Line => {
                if c == '\n' {
                    state = State::Normal;
                }
            }
            State::Block => {
                if c == '*' && chars.get(i + 1) == Some(&'/') {
                    state = State::Normal;
                    i += 1;
                }
            }
            State::Dollar => {
                if c == '$' {
                    if let Some(len) = dollar_close(&chars, i, &tag) {
                        state = State::Normal;
                        i += len - 1;
                    }
                }
            }
        }
        i += 1;
    }
    push(&chars, seg_start, n, &mut out);
    out
}

/// The trimmed text of the statement containing `offset` (caret or marker).
pub(crate) fn statement_at(sql: &str, offset: usize) -> Option<String> {
    let stmts = statements(sql);
    let stmt = stmts
        .iter()
        .find(|s| offset <= s.end)
        .or_else(|| stmts.last())?;
    Some(slice(sql, stmt.start, stmt.end))
}

/// If `sql` is a single `SELECT`/`WITH` statement without its own `LIMIT`,
/// return it with `LIMIT n` appended (trailing `;` stripped) so the server caps
/// the returned rows. Returns `None` for anything else — multiple statements, a
/// non-read statement, or a query that already contains a `LIMIT` — which run
/// unchanged (appending would risk a syntax error or altering intent).
pub(crate) fn add_limit(sql: &str, n: usize) -> Option<String> {
    if statements(sql).len() != 1 {
        return None;
    }
    let trimmed = sql.trim().trim_end_matches(';').trim();
    let lower = trimmed.to_lowercase();
    if !(lower.starts_with("select") || lower.starts_with("with")) {
        return None;
    }
    // Conservative: any existing `limit` token (even in a subquery) means we
    // leave the query alone rather than risk a double `LIMIT`.
    if lower.split(|c: char| !c.is_alphanumeric() && c != '_').any(|w| w == "limit") {
        return None;
    }
    Some(format!("{trimmed} LIMIT {n}"))
}

/// The trimmed text of a character range `[start, end)` of `sql`.
pub(crate) fn slice(sql: &str, start: usize, end: usize) -> String {
    sql.chars()
        .skip(start)
        .take(end.saturating_sub(start))
        .collect::<String>()
        .trim()
        .to_string()
}

/// Record a statement's trimmed range, dropping whitespace-only segments.
fn push(chars: &[char], start: usize, end: usize, out: &mut Vec<Stmt>) {
    let content_start = (start..end).find(|&i| !chars[i].is_whitespace());
    if let Some(cs) = content_start {
        out.push(Stmt { start: cs, end });
    }
}

/// If `chars[i]` opens a dollar quote `$tag$`, return `(tag, opener_len)`.
fn dollar_open(chars: &[char], i: usize) -> Option<(String, usize)> {
    let mut j = i + 1;
    let mut tag = String::new();
    while let Some(&c) = chars.get(j) {
        if c == '$' {
            return Some((tag, j - i + 1));
        }
        if c.is_alphanumeric() || c == '_' {
            tag.push(c);
            j += 1;
        } else {
            return None;
        }
    }
    None
}

/// If `chars[i]` starts the closing `$tag$`, return its length.
fn dollar_close(chars: &[char], i: usize, tag: &str) -> Option<usize> {
    let mut j = i + 1;
    for t in tag.chars() {
        if chars.get(j) != Some(&t) {
            return None;
        }
        j += 1;
    }
    (chars.get(j) == Some(&'$')).then_some(j - i + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn texts(sql: &str) -> Vec<String> {
        statements(sql).iter().map(|s| slice(sql, s.start, s.end)).collect()
    }

    #[test]
    fn splits_top_level_statements() {
        assert_eq!(
            texts("SELECT 1;\nSELECT 2;"),
            vec!["SELECT 1;".to_string(), "SELECT 2;".to_string()]
        );
    }

    #[test]
    fn ignores_semicolons_in_strings_and_comments() {
        // The `;` inside the string and the line comment must not split; the
        // second statement keeps its leading comment (harmless to run).
        let sql = "SELECT ';' AS a; -- x; y\nSELECT \"c;d\";";
        assert_eq!(
            texts(sql),
            vec![
                "SELECT ';' AS a;".to_string(),
                "-- x; y\nSELECT \"c;d\";".to_string(),
            ]
        );
    }

    #[test]
    fn ignores_semicolons_in_dollar_quotes() {
        let sql = "CREATE FUNCTION f() AS $$ BEGIN; END; $$ LANGUAGE plpgsql; SELECT 1;";
        assert_eq!(texts(sql).len(), 2);
    }

    #[test]
    fn drops_whitespace_only_trailing_segment() {
        assert_eq!(texts("SELECT 1;\n\n"), vec!["SELECT 1;".to_string()]);
        assert_eq!(texts("SELECT 1"), vec!["SELECT 1".to_string()]);
    }

    #[test]
    fn statement_at_caret_picks_the_containing_statement() {
        let sql = "SELECT 1;\nSELECT 2;";
        assert_eq!(statement_at(sql, 3).as_deref(), Some("SELECT 1;")); // in first
        assert_eq!(statement_at(sql, 14).as_deref(), Some("SELECT 2;")); // in second
        assert_eq!(statement_at(sql, 999).as_deref(), Some("SELECT 2;")); // past end → last
    }

    #[test]
    fn marker_offset_is_first_non_whitespace_char() {
        // Two statements; the second starts after a newline + spaces.
        let stmts = statements("SELECT 1;\n   SELECT 2;");
        assert_eq!(stmts.len(), 2);
        assert_eq!(stmts[0].start, 0);
        assert_eq!(stmts[1].start, 13); // after "SELECT 1;\n   "
    }

    #[test]
    fn add_limit_caps_plain_selects_only() {
        assert_eq!(
            add_limit("SELECT * FROM t;", 101).as_deref(),
            Some("SELECT * FROM t LIMIT 101")
        );
        assert_eq!(
            add_limit("WITH x AS (SELECT 1) SELECT * FROM x", 50).as_deref(),
            Some("WITH x AS (SELECT 1) SELECT * FROM x LIMIT 50")
        );
        // Already limited, non-select, or multi-statement → untouched.
        assert_eq!(add_limit("SELECT * FROM t LIMIT 5", 101), None);
        assert_eq!(add_limit("UPDATE t SET a = 1", 101), None);
        assert_eq!(add_limit("SELECT 1; SELECT 2;", 101), None);
    }
}
