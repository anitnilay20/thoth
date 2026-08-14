//! Command-line entry point for Thoth's display-free runtime.

use std::ffi::OsString;

use clap::{Arg, ArgAction, ArgMatches, Command, value_parser};
use clap_complete::{Shell, generate};
use serde_json::{Map, Value};
use thoth_plugin_sdk::cli::{CliArgKind, CliInvocation, CliSchema, PluginCli};

use crate::{headless::HeadlessRuntime, plugin::plugin_ui_host::PluginCore, settings::Settings};

/// Fully rendered output from one CLI invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

/// Return whether an argument selects a built-in or discovered CLI command.
pub fn is_known_command(argument: &str, plugin_ids: &[String]) -> bool {
    argument.starts_with('-')
        || matches!(argument, "plugins" | "completions" | "help")
        || plugin_ids.iter().any(|id| id == argument)
}

/// Discover plugin command IDs for resolving a command/file name collision.
pub fn discover_command_ids(settings: Settings) -> Result<Vec<String>, String> {
    let mut runtime = HeadlessRuntime::new(settings);
    runtime.discover_cli_plugins()?;
    Ok(runtime
        .cli_schemas()
        .into_iter()
        .map(|schema| schema.id)
        .collect())
}

enum Action {
    ListCliPlugins,
    Plugin {
        plugin_id: String,
        invocation: CliInvocation,
    },
    Completions(Shell),
}

/// Parse and run a CLI invocation without initializing a native window.
pub fn run<I, T>(args: I, settings: Settings) -> CliOutput
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    run_with(args, || settings)
}

/// Load settings, discover plugin command schemas, then parse and run a CLI
/// invocation. Discovery also runs for help and completion generation.
pub fn run_with<I, T, F>(args: I, load_settings: F) -> CliOutput
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
    F: FnOnce() -> Settings,
{
    let mut runtime = HeadlessRuntime::new(load_settings());
    if let Err(error) = runtime.discover_cli_plugins() {
        return CliOutput {
            exit_code: 1,
            stdout: String::new(),
            stderr: format!("{error}\n"),
        };
    }
    let schemas = runtime.cli_schemas();
    let parser = command(&schemas);
    let action = match parse(args, parser, &schemas) {
        Ok(action) => action,
        Err(error) => return clap_error(error),
    };

    match action {
        Action::Completions(shell) => completion_output(shell, command(&schemas)),
        Action::ListCliPlugins => cli_plugins_output(&schemas),
        Action::Plugin {
            plugin_id,
            invocation,
        } => plugin_output(runtime.run_cli(&plugin_id, &invocation)),
    }
}

/// Run with explicit live plugin adapters, used by the WASM registry and tests.
pub fn run_with_plugins<I, T>(
    args: I,
    settings: Settings,
    core_plugins: Vec<Box<dyn PluginCore>>,
    cli_plugins: Vec<Box<dyn PluginCli>>,
) -> CliOutput
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let runtime = HeadlessRuntime::with_adapters(settings, core_plugins, cli_plugins);
    let schemas = runtime.cli_schemas();
    let cli = command(&schemas);
    let action = match parse(args, cli, &schemas) {
        Ok(action) => action,
        Err(error) => return clap_error(error),
    };

    match action {
        Action::Completions(shell) => completion_output(shell, command(&schemas)),
        Action::ListCliPlugins => cli_plugins_output(&schemas),
        Action::Plugin {
            plugin_id,
            invocation,
        } => plugin_output(runtime.run_cli(&plugin_id, &invocation)),
    }
}

fn command(schemas: &[CliSchema]) -> Command {
    let mut command = Command::new("thoth")
        .version(env!("CARGO_PKG_VERSION"))
        .about("Explore data from the terminal or desktop")
        .subcommand_required(true)
        .after_help("Plugin commands: thoth <plugin-id> <subcommand> [options]")
        .subcommand(Command::new("plugins").about("List available CLI plugin names"))
        .subcommand(
            Command::new("completions")
                .about("Generate shell completion definitions")
                .arg(
                    Arg::new("shell")
                        .required(true)
                        .value_parser(value_parser!(Shell)),
                ),
        );

    for schema in schemas {
        let mut plugin = Command::new(schema.id.clone())
            .about(schema.about.clone())
            .subcommand_required(true);
        if !schema.examples.is_empty() {
            plugin = plugin.after_help(format_examples(&schema.examples));
        }
        for declared in &schema.subcommands {
            let mut subcommand = Command::new(declared.name.clone()).about(declared.about.clone());
            if !declared.examples.is_empty() {
                subcommand = subcommand.after_help(format_examples(&declared.examples));
            }
            for declared_arg in &declared.args {
                let mut arg = Arg::new(declared_arg.id.clone())
                    .help(declared_arg.help.clone())
                    .required(declared_arg.required);
                arg = match &declared_arg.kind {
                    CliArgKind::Positional { value_name } => arg.value_name(value_name.clone()),
                    CliArgKind::Option {
                        long,
                        short,
                        value_name,
                    } => {
                        let arg = arg.long(long.clone()).value_name(value_name.clone());
                        short.map_or(arg.clone(), |short| arg.short(short))
                    }
                    CliArgKind::Flag { long, short } => {
                        let arg = arg.long(long.clone()).action(ArgAction::SetTrue);
                        short.map_or(arg.clone(), |short| arg.short(short))
                    }
                };
                subcommand = subcommand.arg(arg);
            }
            plugin = plugin.subcommand(subcommand);
        }
        command = command.subcommand(plugin);
    }
    command
}

fn format_examples(examples: &[String]) -> String {
    format!("Examples:\n  {}", examples.join("\n  "))
}

fn parse<I, T>(args: I, command: Command, schemas: &[CliSchema]) -> Result<Action, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    let matches = command.try_get_matches_from(args)?;
    let Some((top_level, matches)) = matches.subcommand() else {
        unreachable!("clap requires a top-level subcommand");
    };
    match top_level {
        "plugins" => Ok(Action::ListCliPlugins),
        "completions" => Ok(Action::Completions(
            *matches
                .get_one::<Shell>("shell")
                .expect("shell is required by clap"),
        )),
        plugin_id => parse_plugin(plugin_id, matches, schemas),
    }
}

fn parse_plugin(
    plugin_id: &str,
    matches: &ArgMatches,
    schemas: &[CliSchema],
) -> Result<Action, clap::Error> {
    let schema = schemas
        .iter()
        .find(|schema| schema.id == plugin_id)
        .expect("clap only accepts registered plugin ids");
    let (subcommand, matches) = matches
        .subcommand()
        .expect("plugin subcommands are required by clap");
    let declared = schema
        .subcommands
        .iter()
        .find(|declared| declared.name == subcommand)
        .expect("clap only accepts registered plugin subcommands");
    let mut values = Map::new();
    for arg in &declared.args {
        match arg.kind {
            CliArgKind::Flag { .. } => {
                values.insert(arg.id.clone(), Value::Bool(matches.get_flag(&arg.id)));
            }
            _ => {
                if let Some(value) = matches.get_one::<String>(&arg.id) {
                    values.insert(arg.id.clone(), Value::String(value.clone()));
                }
            }
        }
    }
    Ok(Action::Plugin {
        plugin_id: plugin_id.to_string(),
        invocation: CliInvocation {
            subcommand: subcommand.to_string(),
            values,
        },
    })
}

fn completion_output(shell: Shell, mut command: Command) -> CliOutput {
    let mut bytes = Vec::new();
    generate(shell, &mut command, "thoth", &mut bytes);
    CliOutput {
        exit_code: 0,
        stdout: String::from_utf8(bytes).expect("completion generators only emit UTF-8"),
        stderr: String::new(),
    }
}

fn cli_plugins_output(schemas: &[CliSchema]) -> CliOutput {
    CliOutput {
        exit_code: 0,
        stdout: schemas
            .iter()
            .map(|schema| format!("{}\n", schema.id))
            .collect(),
        stderr: String::new(),
    }
}

fn plugin_output(
    result: std::result::Result<thoth_plugin_sdk::cli::CliOutput, String>,
) -> CliOutput {
    match result {
        Ok(output) => CliOutput {
            exit_code: 0,
            stdout: render_table(&output.records),
            stderr: String::new(),
        },
        Err(error) => CliOutput {
            exit_code: 1,
            stdout: String::new(),
            stderr: format!("{error}\n"),
        },
    }
}

fn render_table(records: &[Value]) -> String {
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

fn format_value(value: &Value) -> String {
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

fn single_line(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn display_width(value: &str) -> usize {
    unicode_width::UnicodeWidthStr::width(value)
}

fn table_border(widths: &[usize]) -> String {
    let mut border = String::from("+");
    for width in widths {
        border.push_str(&"-".repeat(width + 2));
        border.push('+');
    }
    border.push('\n');
    border
}

fn write_table_row(output: &mut String, cells: &[String], widths: &[usize]) {
    output.push('|');
    for (cell, width) in cells.iter().zip(widths) {
        output.push(' ');
        output.push_str(cell);
        output.push_str(&" ".repeat(width.saturating_sub(display_width(cell)) + 1));
        output.push('|');
    }
    output.push('\n');
}

fn clap_error(error: clap::Error) -> CliOutput {
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

#[cfg(test)]
mod tests {
    use serde_json::json;
    use thoth_plugin_sdk::cli::{
        CliArg, CliArgKind, CliInvocation, CliOutput as PluginOutput, CliSchema, CliSubcommand,
        PluginCli,
    };

    use super::{CliOutput, run, run_with_plugins};
    use crate::settings::Settings;

    struct DemoCli;

    impl PluginCli for DemoCli {
        fn cli_schema(&self) -> CliSchema {
            CliSchema {
                id: "demo".into(),
                about: "Demo plugin commands".into(),
                examples: vec!["thoth demo query --index logs".into()],
                subcommands: vec![CliSubcommand {
                    name: "query".into(),
                    about: "Query demo data".into(),
                    args: vec![
                        CliArg {
                            id: "index".into(),
                            help: "Index name".into(),
                            required: true,
                            kind: CliArgKind::Option {
                                long: "index".into(),
                                short: Some('i'),
                                value_name: "INDEX".into(),
                            },
                        },
                        CliArg {
                            id: "pretty".into(),
                            help: "Pretty output".into(),
                            required: false,
                            kind: CliArgKind::Flag {
                                long: "pretty".into(),
                                short: Some('p'),
                            },
                        },
                    ],
                    examples: vec!["thoth demo query --index logs".into()],
                }],
            }
        }

        fn run_cli(&self, invocation: &CliInvocation) -> Result<PluginOutput, String> {
            Ok(PluginOutput::one(json!({
                "command": invocation.subcommand,
                "values": invocation.values,
            })))
        }
    }

    fn disabled_settings() -> Settings {
        let mut settings = Settings::default();
        settings.plugins.enabled = false;
        settings
    }

    #[test]
    fn help_documents_plugin_command_shape() {
        let output = run(["thoth", "--help"], disabled_settings());
        assert_eq!(output.exit_code, 0);
        assert!(
            output
                .stdout
                .contains("thoth <plugin-id> <subcommand> [options]")
        );
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn registered_plugin_appears_in_help_and_routes_values() {
        let help = run_with_plugins(
            ["thoth", "--help"],
            disabled_settings(),
            vec![],
            vec![Box::new(DemoCli)],
        );
        assert!(help.stdout.contains("demo"));
        assert!(help.stdout.contains("Demo plugin commands"));

        let output = run_with_plugins(
            ["thoth", "demo", "query", "--index", "logs", "--pretty"],
            disabled_settings(),
            vec![],
            vec![Box::new(DemoCli)],
        );
        assert_eq!(output.exit_code, 0);
        assert_eq!(
            output.stdout,
            concat!(
                "+---------+-------------------------+\n",
                "| command | values                  |\n",
                "+---------+-------------------------+\n",
                "| query   | index=logs; pretty=true |\n",
                "+---------+-------------------------+\n",
            )
        );
    }

    #[test]
    fn plugins_lists_only_registered_cli_command_names() {
        let output = run_with_plugins(
            ["thoth", "plugins"],
            disabled_settings(),
            vec![],
            vec![Box::new(DemoCli)],
        );

        assert_eq!(output.exit_code, 0);
        assert_eq!(output.stdout, "demo\n");
        assert!(output.stderr.is_empty());
    }

    #[test]
    fn unknown_plugin_is_a_clear_clap_error() {
        let output = run(["thoth", "missing", "query"], disabled_settings());
        assert_ne!(output.exit_code, 0);
        assert!(output.stdout.is_empty());
        assert!(output.stderr.contains("unrecognized subcommand 'missing'"));
    }

    #[test]
    fn registered_plugin_requires_a_subcommand() {
        let output = run_with_plugins(
            ["thoth", "demo"],
            disabled_settings(),
            vec![],
            vec![Box::new(DemoCli)],
        );
        assert_ne!(output.exit_code, 0);
        assert!(output.stderr.contains("subcommand"));
    }

    #[test]
    fn generates_zsh_completions() {
        let CliOutput {
            exit_code,
            stdout,
            stderr,
        } = run(["thoth", "completions", "zsh"], disabled_settings());
        assert_eq!(exit_code, 0);
        assert!(stdout.contains("#compdef thoth"));
        assert!(stderr.is_empty());
    }

    #[test]
    fn plugin_records_render_as_a_table_without_json_syntax() {
        let output = super::plugin_output(Ok(PluginOutput {
            records: vec![
                json!({ "name": "primary", "count": 2, "active": true }),
                json!({ "name": "archive", "count": null, "active": false }),
            ],
        }));

        assert!(output.stdout.contains("| active | count | name"));
        assert!(output.stdout.contains("| true   | 2     | primary"));
        assert!(output.stdout.contains("| false  | —     | archive"));
        assert!(!output.stdout.contains("\"name\""));
        assert!(!output.stdout.contains("{"));
    }

    #[test]
    fn empty_plugin_output_is_human_readable() {
        let output = super::plugin_output(Ok(PluginOutput::default()));
        assert_eq!(output.stdout, "No results.\n");
    }

    #[test]
    fn recognizes_built_in_flags_and_discovered_commands() {
        let plugins = vec!["seshat".to_string()];
        assert!(super::is_known_command("--help", &plugins));
        assert!(super::is_known_command("completions", &plugins));
        assert!(super::is_known_command("seshat", &plugins));
        assert!(!super::is_known_command("data.json", &plugins));
    }

    #[test]
    fn table_width_uses_terminal_cells() {
        assert_eq!(super::display_width("界"), 2);
        assert_eq!(super::display_width("e\u{301}"), 1);
    }
}
