//! Command-line entry point for Thoth's display-free runtime.

use std::ffi::OsString;

use clap::{CommandFactory, Parser, Subcommand};
use clap_complete::{Shell, generate};
use serde_json::json;

use crate::{
    headless::{CliCommand, HeadlessRuntime},
    settings::Settings,
};

/// Fully rendered output from one CLI invocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CliOutput {
    pub exit_code: i32,
    pub stdout: String,
    pub stderr: String,
}

#[derive(Debug, Parser)]
#[command(
    name = "thoth",
    version,
    about = "Explore data from the terminal or desktop",
    after_help = "Plugin commands: thoth <plugin-id> <subcommand> [options]"
)]
struct Cli {
    #[command(subcommand)]
    command: TopLevelCommand,
}

#[derive(Debug, Subcommand)]
enum TopLevelCommand {
    /// List discovered plugins as JSON.
    Plugins,
    /// Generate shell completion definitions.
    Completions {
        /// Shell to generate completions for.
        shell: Shell,
    },
    /// A command owned by an installed plugin.
    #[command(external_subcommand)]
    Plugin(Vec<OsString>),
}

enum Action {
    Runtime(CliCommand),
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

/// Parse a CLI invocation and load settings only when it reaches the runtime.
/// Help and completion generation therefore remain side-effect free.
pub fn run_with<I, T, F>(args: I, load_settings: F) -> CliOutput
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
    F: FnOnce() -> Settings,
{
    let action = match parse(args) {
        Ok(action) => action,
        Err(error) => {
            let exit_code = error.exit_code();
            let message = error.to_string();
            return if error.use_stderr() {
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
            };
        }
    };

    match action {
        Action::Completions(shell) => {
            let mut bytes = Vec::new();
            let mut command = Cli::command();
            generate(shell, &mut command, "thoth", &mut bytes);
            CliOutput {
                exit_code: 0,
                stdout: String::from_utf8(bytes)
                    .expect("clap completion generators only emit UTF-8"),
                stderr: String::new(),
            }
        }
        Action::Runtime(command) => {
            let result = HeadlessRuntime::new(load_settings()).run(command);
            CliOutput {
                exit_code: result.exit_code,
                stdout: result
                    .stdout
                    .map(|value| format!("{value}\n"))
                    .unwrap_or_default(),
                stderr: result
                    .stderr
                    .map(|value| format!("{value}\n"))
                    .unwrap_or_default(),
            }
        }
    }
}

fn parse<I, T>(args: I) -> Result<Action, clap::Error>
where
    I: IntoIterator<Item = T>,
    T: Into<OsString> + Clone,
{
    match Cli::try_parse_from(args)?.command {
        TopLevelCommand::Plugins => Ok(Action::Runtime(CliCommand::ListPlugins)),
        TopLevelCommand::Completions { shell } => Ok(Action::Completions(shell)),
        TopLevelCommand::Plugin(parts) => parse_plugin_command(parts),
    }
}

fn parse_plugin_command(parts: Vec<OsString>) -> Result<Action, clap::Error> {
    let mut parts = parts.into_iter();
    let plugin_id = parts
        .next()
        .expect("external subcommands always contain their command name")
        .to_string_lossy()
        .into_owned();
    let Some(command) = parts.next() else {
        return Err(Cli::command().error(
            clap::error::ErrorKind::MissingRequiredArgument,
            format!("plugin '{plugin_id}' requires a subcommand"),
        ));
    };
    let argv: Vec<String> = parts
        .map(|part| part.to_string_lossy().into_owned())
        .collect();

    Ok(Action::Runtime(CliCommand::Plugin {
        plugin_id,
        command: command.to_string_lossy().into_owned(),
        args: json!({ "argv": argv }),
    }))
}

#[cfg(test)]
mod tests {
    use super::{Action, CliOutput, parse, run};
    use crate::{headless::CliCommand, settings::Settings};

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
    fn parses_plugin_commands_and_preserves_options() {
        let action = parse([
            "thoth",
            "seshat",
            "query",
            "--index",
            "logs",
            "-q",
            "level:error",
        ])
        .unwrap();
        let Action::Runtime(CliCommand::Plugin {
            plugin_id,
            command,
            args,
        }) = action
        else {
            panic!("expected a plugin command");
        };

        assert_eq!(plugin_id, "seshat");
        assert_eq!(command, "query");
        assert_eq!(
            args,
            serde_json::json!({ "argv": ["--index", "logs", "-q", "level:error"] })
        );
    }

    #[test]
    fn missing_plugin_subcommand_is_a_clear_error() {
        let output = run(["thoth", "seshat"], disabled_settings());

        assert_ne!(output.exit_code, 0);
        assert!(output.stdout.is_empty());
        assert!(
            output
                .stderr
                .contains("plugin 'seshat' requires a subcommand")
        );
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
}
