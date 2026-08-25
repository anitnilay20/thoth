use serde_json::Value;

use crate::cli::CliOutput;

pub fn format_value(value: &Value) -> String {
    match value {
        Value::Null => "—".to_string(),
        Value::Bool(value) => value.to_string(),
        Value::Number(value) => value.to_string(),
        Value::String(value) => single_line(value),
        Value::Array(values) => values
            .iter()
            .map(format_value)
            .collect::<Vec<_>>()
            .join(", "),
        Value::Object(values) => values
            .iter()
            .map(|(key, value)| format!("{key}={}", format_value(value)))
            .collect::<Vec<_>>()
            .join("; "),
    }
}

pub fn single_line(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

pub fn clap_error(error: clap::Error) -> CliOutput {
    let exit_code = error.exit_code();
    let message = error.to_string();
    if error.use_stderr() {
        CliOutput {
            exit_code,
            stdout: String::new(),
            stderr: message,
        }
    } else {
        CliOutput {
            exit_code,
            stdout: message,
            stderr: String::new(),
        }
    }
}
