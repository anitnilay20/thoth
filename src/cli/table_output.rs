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
pub fn print_arrow(batches: Vec<RecordBatch>) -> String {
    let headers: Vec<String> = match batches.first() {
        Some(b) => b
            .schema()
            .fields()
            .iter()
            .map(|f| f.name().to_string())
            .collect(),
        None => return "".to_string(),
    };

    if headers.is_empty() {
        return "No results.\n".to_string();
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
            .expect("failed to build formatters");

        // Walk each row index, format every column at that index.
        for row_idx in 0..batch.num_rows() {
            let record: Vec<String> = formatters
                .iter()
                .map(|f| f.value(row_idx).to_string())
                .collect();
            rows.push(record);
        }
    }

    // Compute each column's width = max(header, widest cell).
    let mut widths: Vec<usize> = headers.iter().map(|h| h.len()).collect();
    for row in &rows {
        for (i, cell) in row.iter().enumerate() {
            if cell.len() > widths[i] {
                widths[i] = cell.len();
            }
        }
    }

    let mut output = String::new();

    // Header row.
    let header_line = headers
        .iter()
        .enumerate()
        .map(|(i, h)| format!("{:<width$}", h, width = widths[i]))
        .collect::<Vec<_>>()
        .join(" | ");
    // println!("{header_line}");
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
            .map(|(i, cell)| format!("{:<width$}", cell, width = widths[i]))
            .collect::<Vec<_>>()
            .join(" | ");

        output += &(line + "\n");
    }

    output
        + &format!(
            "\n({} row{})",
            rows.len(),
            if rows.len() == 1 { "" } else { "s" }
        )
}
