//! CSV Export — an **exporter** plugin (#135). The host owns the dataset; when
//! the user picks "Export → CSV" in a DataView, the host serialises its single
//! owned copy to a records blob and calls [`run`](CsvExportPlugin::run) here to
//! format it as CSV bytes, which the host then writes to disk. This plugin is
//! stateless and never touches the data bus or the filesystem itself.

#[rustfmt::skip]
mod bindings;

use serde::Deserialize;

use thoth_plugin_sdk::PluginMeta;

use bindings::exports::thoth::plugin::{
    exporter::{ExportOption, Guest as ExporterGuest, PluginError},
    plugin_lifecycle::Guest as LifecycleGuest,
    plugin_settings::{Guest as SettingsGuest, SettingsOutput},
};

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

/// The host-owned dataset shape passed to `run` (see the `exporter.run` WIT doc):
/// column order preserved, every cell a string.
#[derive(Deserialize)]
struct Records {
    #[serde(default)]
    columns: Vec<String>,
    #[serde(default)]
    rows: Vec<Vec<String>>,
}

/// RFC 4180-ish field escaping: quote if the value has a comma, quote, or newline.
fn esc(s: &str) -> String {
    if s.contains([',', '"', '\n', '\r']) {
        format!("\"{}\"", s.replace('"', "\"\""))
    } else {
        s.to_string()
    }
}

/// Serialize `{columns, rows}` into CSV text (header + escaped rows).
fn build_csv(columns: &[String], rows: &[Vec<String>]) -> String {
    let mut out = String::new();
    out.push_str(&columns.iter().map(|c| esc(c)).collect::<Vec<_>>().join(","));
    out.push('\n');
    for row in rows {
        out.push_str(&row.iter().map(|c| esc(c)).collect::<Vec<_>>().join(","));
        out.push('\n');
    }
    out
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

    fn run(records_json: String, _options: Vec<(String, String)>) -> Result<Vec<u8>, PluginError> {
        let recs: Records = serde_json::from_str(&records_json).map_err(|e| PluginError {
            code: 1,
            message: format!("invalid records-json: {e}"),
        })?;

        Ok(build_csv(&recs.columns, &recs.rows).into_bytes())
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
    use super::{build_csv, esc};

    #[test]
    fn esc_quotes_only_when_needed() {
        assert_eq!(esc("plain"), "plain");
        assert_eq!(esc("a,b"), "\"a,b\"");
        assert_eq!(esc("with \"quotes\""), "\"with \"\"quotes\"\"\"");
        assert_eq!(esc("line\nbreak"), "\"line\nbreak\"");
    }

    #[test]
    fn build_csv_writes_header_and_escaped_rows() {
        let cols = vec!["id".to_string(), "note".to_string()];
        let rows = vec![
            vec!["1".to_string(), "x,y".to_string()],
            vec!["2".to_string(), "plain".to_string()],
        ];
        assert_eq!(build_csv(&cols, &rows), "id,note\n1,\"x,y\"\n2,plain\n");
    }
}
