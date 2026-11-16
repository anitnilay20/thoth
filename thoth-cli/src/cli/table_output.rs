use duckdb::arrow::{
    array::RecordBatch,
    util::display::{ArrayFormatter, FormatOptions},
};
use serde_json::Value;

use crate::cli::utils::format_value;

pub fn display_width(value: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(value)
}

pub fn table_border(widths: &[usize]) -> String {
    let mut border = String::from("+");
    for width in widths {
        border.push_str(&"-".repeat(width + 2));
        border.push('+');
    }
    border.push('\n');
    border
}

pub fn write_table_row(output: &mut String, cells: &[String], widths: &[usize]) {
    output.push('|');
    for (cell, width) in cells.iter().zip(widths) {
        output.push(' ');
        output.push_str(cell);
        output.push_str(&" ".repeat(width.saturating_sub(display_width(cell)) + 1));
        output.push('|');
    }
    output.push('\n');
}

pub fn print_json(records: &[Value]) -> String {
    if records.is_empty() {
        return "No results.\n".to_string();
    }

    let all_objects = records.iter().all(Value::is_object);
    let columns = if all_objects {
        let mut columns = Vec::new();
        for record in records {
            for key in record.as_object().expect("all records are objects").keys() {
                if !columns.contains(key) {
                    columns.push(key.clone());
                }
            }
        }
        // Sorted, so a record's fields come out in the same order whatever
        // the build did. `serde_json::Map` is a `BTreeMap` — already sorted —
        // until something turns on `preserve_order`, when it becomes an
        // insertion-ordered `IndexMap`; the `url-source` plugin does, and
        // cargo unifies features across a workspace build, so `cargo test`
        // and `cargo test -p thoth` disagreed about the column order.
        columns.sort();
        columns
    } else {
        vec!["value".to_string()]
    };

    if columns.is_empty() {
        return "No fields.\n".to_string();
    }

    let rows: Vec<Vec<String>> = records
        .iter()
        .map(|record| {
            if all_objects {
                let object = record.as_object().expect("all records are objects");
                columns
                    .iter()
                    .map(|column| object.get(column).map(format_value).unwrap_or_default())
                    .collect()
            } else {
                vec![format_value(record)]
            }
        })
        .collect();
    let widths: Vec<usize> = columns
        .iter()
        .enumerate()
        .map(|(index, column)| {
            rows.iter()
                .map(|row| display_width(&row[index]))
                .max()
                .unwrap_or_default()
                .max(display_width(column))
        })
        .collect();

    let border = table_border(&widths);
    let mut output = String::new();
    output.push_str(&border);
    write_table_row(&mut output, &columns, &widths);
    output.push_str(&border);
    for row in &rows {
        write_table_row(&mut output, row, &widths);
    }
    output.push_str(&border);
    output
}

/// Render headers + rows as a simple aligned ASCII table.
///
/// Padded by *display* width, not byte or `char` count: a column holding
/// `José` or `你好` lines up with the rest rather than being padded short or
/// long. Cells are flattened to one line, because a value with a newline in it
/// splits the row it is in and the table stops being a table.
pub fn print_arrow(batches: Vec<RecordBatch>) -> Result<String, crate::error::ThothError> {
    let headers: Vec<String> = match batches.first() {
        Some(b) => b
            .schema()
            .fields()
            .iter()
            .map(|f| f.name().to_string())
            .collect(),
        // A query that matched nothing usually yields no batches at all. That
        // is an answer, and printing nothing is not one.
        None => return Ok("No results.\n".to_string()),
    };

    if headers.is_empty() {
        return Ok("No results.\n".to_string());
    }

    let opts = FormatOptions::default().with_null("NULL");
    let mut rows: Vec<Vec<String>> = Vec::new();

    for batch in batches {
        // One formatter per column in this batch.
        let formatters: Vec<ArrayFormatter> = batch
            .columns()
            .iter()
            .map(|col| ArrayFormatter::try_new(col.as_ref(), &opts))
            .collect::<Result<_, _>>()
            .map_err(|e| crate::error::ThothError::DatabaseConversionError {
                reason: e.to_string(),
            })?;

        // Walk each row index, format every column at that index.
        for row_idx in 0..batch.num_rows() {
            let record: Vec<String> = formatters
                .iter()
                .map(|f| crate::cli::utils::single_line(&f.value(row_idx).to_string()))
                .collect();
            rows.push(record);
        }
    }

    // Compute each column's width = max(header, widest cell).
    let mut widths: Vec<usize> = headers.iter().map(|h| display_width(h)).collect();
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            if let Some(width) = widths.get_mut(i) {
                *width = (*width).max(display_width(cell));
            }
        }
    }

    /// One cell, padded to `width` columns on the terminal.
    fn pad(cell: &str, width: usize) -> String {
        let mut out = cell.to_string();
        out.push_str(&" ".repeat(width.saturating_sub(display_width(cell))));
        out
    }

    let mut output = String::new();

    // Header row.
    let header_line = headers
        .iter()
        .enumerate()
        .map(|(i, h)| pad(h, widths[i]))
        .collect::<Vec<_>>()
        .join(" | ");
    output += &(header_line + "\n");

    // Separator.
    let sep = widths
        .iter()
        .map(|w| "-".repeat(*w))
        .collect::<Vec<_>>()
        .join("-+-");
    output += &(sep + "\n");

    // Data rows.
    for row in &rows {
        let line = row
            .iter()
            .enumerate()
            .map(|(i, cell)| pad(cell, widths[i]))
            .collect::<Vec<_>>()
            .join(" | ");

        output += &(line + "\n");
    }

    Ok(output
        + &format!(
            "\n({} row{})",
            rows.len(),
            if rows.len() == 1 { "" } else { "s" }
        ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use duckdb::Connection;

    /// The table `sql` prints, run through the same path the CLI takes.
    fn table(sql: &str) -> String {
        let conn = Connection::open_in_memory().expect("in-memory duckdb");
        let mut stmt = conn.prepare(sql).expect("prepared");
        let batches: Vec<RecordBatch> = stmt.query_arrow([]).expect("ran").collect();
        print_arrow(batches).expect("formatted")
    }

    #[test]
    fn a_query_that_matched_nothing_says_so() {
        // No rows usually means no batches at all, and printing an empty
        // string leaves the user unable to tell it from a crash. `print_json`
        // has always said "No results."; this now agrees with it.
        assert_eq!(table("SELECT 1 AS n WHERE false"), "No results.\n");
    }

    #[test]
    fn columns_line_up_when_the_text_is_not_ascii() {
        // Widths counted in bytes pad a multi-byte cell short, and widths
        // counted in `char`s pad a double-width one long. Either way the
        // columns stop being columns.
        let out = table("SELECT * FROM (VALUES ('José', 1), ('你好', 22), ('ab', 333)) t(name, n)");
        let widths: Vec<usize> = out
            .lines()
            .take_while(|l| !l.is_empty())
            .map(display_width)
            .collect();
        assert!(
            widths.windows(2).all(|w| w[0] == w[1]),
            "rows are different widths on screen:\n{out}"
        );
    }

    #[test]
    fn a_cell_with_a_newline_does_not_split_its_row() {
        let out = table("SELECT 'a\nb' AS s, 1 AS n");
        // Header, separator, one row, a blank line and the count — and the
        // row is one line, however many the value had.
        let body: Vec<&str> = out.lines().take_while(|l| !l.is_empty()).collect();
        assert_eq!(body.len(), 3, "{out}");
        assert!(out.contains("(1 row)"));
    }

    #[test]
    fn the_row_count_agrees_with_the_rows() {
        let out = table("SELECT * FROM (VALUES (1), (2), (3)) t(n)");
        assert!(out.ends_with("(3 rows)"), "{out}");
        assert!(table("SELECT 1 AS n").ends_with("(1 row)"));
    }
}
