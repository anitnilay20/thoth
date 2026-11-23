//! CSV Export — an **exporter** plugin (#135). The host owns the dataset; when
//! the user picks "Export → CSV" in a DataView, the host serialises its single
//! owned copy to a records blob and calls [`run`](CsvExportPlugin::run) here to
//! format it as CSV bytes, which the host then writes to disk. This plugin is
//! stateless and never touches the data bus or the filesystem itself.

#[rustfmt::skip]
mod bindings;

use thoth_plugin_sdk::PluginMeta;

use bindings::exports::thoth::plugin::{
    exporter::{ExportOption, Guest as ExporterGuest, PluginError},
    plugin_lifecycle::Guest as LifecycleGuest,
    plugin_settings::{Guest as SettingsGuest, SettingsOutput},
};
use bindings::thoth::plugin::dataset_read::{self, DatasetColumn, DatasetPage};

#[derive(PluginMeta)]
#[plugin(
    id = "com.thoth.csv-export",
    name = "CSV Export",
    version = "0.1.0",
    description = "Export a dataset to a CSV file",
    capabilities = [Exporter],
    author = "Thoth contributors",
)]
struct CsvExportPlugin;

/// RFC 4180-ish field escaping: quote if the value has a comma, quote, or newline.
fn esc(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// One CSV header line, escaped like any other field.
fn header_line(columns: &[DatasetColumn]) -> String {
    let mut out = columns
        .iter()
        .map(|c| esc(&c.name))
        .collect::<Vec<_>>()
        .join(",");
    out.push('\n');
    out
}

/// One escaped CSV row.
fn write_row(out: &mut String, row: &[String]) {
    out.push_str(&row.iter().map(|c| esc(c)).collect::<Vec<_>>().join(","));
    out.push('\n');
}

fn same_columns(a: &[DatasetColumn], b: &[DatasetColumn]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(a, b)| a.name == b.name && a.type_hint == b.type_hint)
}

fn stale_export() -> PluginError {
    PluginError {
        code: 1,
        message: "dataset changed during export".to_string(),
    }
}

/// Append one page: the first page supplies the header, and a later page with a
/// different schema is rejected rather than silently mixing revisions.
fn append_page(
    out: &mut String,
    columns: &mut Option<Vec<DatasetColumn>>,
    page: &DatasetPage,
) -> Result<(), PluginError> {
    match columns {
        None => {
            out.push_str(&header_line(&page.columns));
            *columns = Some(page.columns.clone());
        }
        Some(expected) if !same_columns(expected, &page.columns) => return Err(stale_export()),
        Some(_) => {}
    }
    for row in &page.rows {
        write_row(out, row);
    }
    Ok(())
}

impl ExporterGuest for CsvExportPlugin {
    fn name() -> String {
        "CSV".to_string()
    }

    fn output_extension() -> String {
        "csv".to_string()
    }

    fn available_options() -> Vec<ExportOption> {
        Vec::new()
    }

    fn run(handle: String, _options: Vec<(String, String)>) -> Result<Vec<u8>, PluginError> {
        // Stream pages straight into the output: a large dataset never needs to
        // be held in the plugin's memory at once.
        const PAGE: u32 = 1000;
        let info = dataset_read::info(&handle)?;
        let mut out = String::new();
        let mut columns = None;
        let mut offset = 0u64;
        let mut total = 0u64;
        loop {
            let page = dataset_read::read(&handle, offset, PAGE)?;
            if offset == 0 {
                total = page.total;
            }
            append_page(&mut out, &mut columns, &page)?;
            let got = page.rows.len() as u64;
            if got == 0 {
                break;
            }
            offset += got;
            if offset >= total {
                break;
            }
        }
        // Pages are not bound to `info`'s revision, so reject a dataset that
        // changed underneath the export instead of returning mixed rows.
        if dataset_read::info(&handle)?.revision != info.revision {
            return Err(stale_export());
        }
        Ok(out.into_bytes())
    }
}

impl LifecycleGuest for CsvExportPlugin {
    fn on_load(_settings: String) {}
    fn on_close() {}
    fn on_setting_change(_settings: String) {}
}

impl SettingsGuest for CsvExportPlugin {
    fn render_settings() -> Result<SettingsOutput, PluginError> {
        let node = thoth_plugin_sdk::render_node::RenderNode::Text(
            thoth_plugin_sdk::components::Typography::builder()
                .text("CSV Export has no settings — pick it from a dataset's Export menu.")
                .build(),
        );
        Ok(SettingsOutput {
            node_json: serde_json::to_string(&node).unwrap_or_default(),
            height_hint: 0,
        })
    }
}

bindings::export!(CsvExportPlugin with_types_in bindings);

#[cfg(test)]
mod tests {
    use super::{append_page, esc};
    use crate::bindings::thoth::plugin::dataset_read::{DatasetColumn, DatasetPage};

    fn col(name: &str) -> DatasetColumn {
        DatasetColumn {
            name: name.to_string(),
            type_hint: "text".to_string(),
        }
    }

    fn page(columns: &[DatasetColumn], rows: &[&[&str]], offset: u64, total: u64) -> DatasetPage {
        DatasetPage {
            columns: columns.to_vec(),
            rows: rows
                .iter()
                .map(|row| row.iter().map(|c| c.to_string()).collect())
                .collect(),
            nulls: Vec::new(),
            offset,
            total,
        }
    }

    #[test]
    fn esc_quotes_only_when_needed() {
        assert_eq!(esc("plain"), "plain");
        assert_eq!(esc("a,b"), "\"a,b\"");
        assert_eq!(esc("with \"quotes\""), "\"with \"\"quotes\"\"\"");
        assert_eq!(esc("line\nbreak"), "\"line\nbreak\"");
    }

    #[test]
    fn production_formatting_escapes_the_header_and_spans_pages() {
        let columns = vec![col("id"), col("no,te")];
        let mut out = String::new();
        let mut expected = None;
        append_page(
            &mut out,
            &mut expected,
            &page(&columns, &[&["1", "x,y"]], 0, 2),
        )
        .unwrap();
        append_page(
            &mut out,
            &mut expected,
            &page(&columns, &[&["2", "plain"]], 1, 2),
        )
        .unwrap();
        assert_eq!(out, "id,\"no,te\"\n1,\"x,y\"\n2,plain\n");
    }

    #[test]
    fn a_changed_schema_fails_the_export() {
        let mut out = String::new();
        let mut expected = None;
        append_page(&mut out, &mut expected, &page(&[col("a")], &[], 0, 0)).unwrap();
        let err = append_page(&mut out, &mut expected, &page(&[col("b")], &[], 0, 0)).unwrap_err();
        assert_eq!(err.code, 1);
    }

    #[test]
    fn an_empty_dataset_still_writes_its_header() {
        let mut out = String::new();
        let mut expected = None;
        append_page(&mut out, &mut expected, &page(&[col("id")], &[], 0, 0)).unwrap();
        assert_eq!(out, "id\n");
    }
}
