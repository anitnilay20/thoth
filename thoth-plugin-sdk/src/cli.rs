//! Display-independent command-line contract for plugins.
//!
//! CLI support is opt-in: ordinary plugins do not implement [`PluginCli`]. A
//! host reads [`CliSchema`] to construct its native argument parser, then sends
//! a validated [`CliInvocation`] across the component boundary. Results remain
//! structured until the host renders the records for its output surface.
//!
//! # Minimal plugin
//!
//! ```
//! use serde_json::json;
//! use thoth_plugin_sdk::cli::{
//!     CliInvocation, CliOutput, CliSchema, CliSubcommand, PluginCli,
//! };
//!
//! struct StatusCli;
//!
//! impl PluginCli for StatusCli {
//!     fn cli_schema(&self) -> CliSchema {
//!         CliSchema {
//!             id: "status".into(),
//!             about: "Inspect service status".into(),
//!             examples: vec!["thoth status ping".into()],
//!             subcommands: vec![CliSubcommand {
//!                 name: "ping".into(),
//!                 about: "Check connectivity".into(),
//!                 args: vec![],
//!                 examples: vec!["thoth status ping".into()],
//!             }],
//!         }
//!     }
//!
//!     fn run_cli(&self, invocation: &CliInvocation) -> Result<CliOutput, String> {
//!         match invocation.subcommand.as_str() {
//!             "ping" => Ok(CliOutput::one(json!({ "status": "ok" }))),
//!             command => Err(format!("unsupported command '{command}'")),
//!         }
//!     }
//! }
//! ```

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashSet;

/// A plugin's complete top-level CLI declaration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliSchema {
    /// Stable plugin command id, such as `seshat`.
    pub id: String,
    /// One-line description shown in top-level help.
    pub about: String,
    /// Complete invocations shown at the bottom of plugin help.
    #[serde(default)]
    pub examples: Vec<String>,
    /// Commands offered beneath the plugin id.
    pub subcommands: Vec<CliSubcommand>,
}

impl CliSchema {
    /// Validate names and uniqueness before a host constructs native parser
    /// objects from untrusted component data.
    pub fn validate(&self) -> Result<(), String> {
        validate_name("plugin id", &self.id)?;
        if self.subcommands.is_empty() {
            return Err(format!("plugin '{}' declares no CLI subcommands", self.id));
        }
        let mut subcommands = HashSet::new();
        for subcommand in &self.subcommands {
            validate_name("subcommand", &subcommand.name)?;
            if !subcommands.insert(&subcommand.name) {
                return Err(format!("duplicate subcommand '{}'", subcommand.name));
            }
            let mut ids = HashSet::new();
            let mut longs = HashSet::new();
            let mut shorts = HashSet::new();
            for arg in &subcommand.args {
                validate_name("argument id", &arg.id)?;
                if !ids.insert(&arg.id) {
                    return Err(format!(
                        "duplicate argument '{}' in subcommand '{}'",
                        arg.id, subcommand.name
                    ));
                }
                let (long, short) = match &arg.kind {
                    CliArgKind::Positional { .. } => (None, None),
                    CliArgKind::Option { long, short, .. } | CliArgKind::Flag { long, short } => {
                        (Some(long), *short)
                    }
                };
                if let Some(long) = long {
                    validate_name("long option", long)?;
                    if !longs.insert(long) {
                        return Err(format!(
                            "duplicate option '--{long}' in subcommand '{}'",
                            subcommand.name
                        ));
                    }
                }
                if let Some(short) = short
                    && (!short.is_ascii_alphanumeric() || !shorts.insert(short))
                {
                    return Err(format!(
                        "invalid or duplicate short option '-{short}' in subcommand '{}'",
                        subcommand.name
                    ));
                }
            }
        }
        Ok(())
    }
}

fn validate_name(kind: &str, value: &str) -> Result<(), String> {
    if value.is_empty()
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "._-".contains(character))
    {
        return Err(format!(
            "invalid {kind} '{value}': use ASCII letters, numbers, '.', '_', or '-'"
        ));
    }
    Ok(())
}

/// One command exposed by a plugin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliSubcommand {
    /// Command name, such as `query`.
    pub name: String,
    /// One-line description shown in help.
    pub about: String,
    /// Arguments accepted by this command.
    #[serde(default)]
    pub args: Vec<CliArg>,
    /// Complete invocations shown at the bottom of command help.
    #[serde(default)]
    pub examples: Vec<String>,
}

/// One positional, option, or boolean flag in a plugin command.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CliArg {
    /// Stable key used in [`CliInvocation::values`].
    pub id: String,
    /// Human-readable help text.
    pub help: String,
    /// Whether this argument must be provided.
    #[serde(default)]
    pub required: bool,
    /// Argument shape and spelling.
    pub kind: CliArgKind,
}

/// Shapes supported by the cross-language CLI schema.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum CliArgKind {
    /// An ordered value without a flag name.
    Positional {
        /// Display name used in generated help.
        value_name: String,
    },
    /// A named option that consumes one value.
    Option {
        /// Long spelling without the leading `--`.
        long: String,
        /// Optional one-character short spelling.
        #[serde(default)]
        short: Option<char>,
        /// Display name used in generated help.
        value_name: String,
    },
    /// A named boolean switch.
    Flag {
        /// Long spelling without the leading `--`.
        long: String,
        /// Optional one-character short spelling.
        #[serde(default)]
        short: Option<char>,
    },
}

/// Validated values delivered to a plugin command.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CliInvocation {
    /// Selected plugin subcommand.
    pub subcommand: String,
    /// Values keyed by [`CliArg::id`]. Flags are booleans; omitted optional
    /// values are absent.
    pub values: serde_json::Map<String, Value>,
}

/// Structured records produced by a plugin invocation.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CliOutput {
    /// Values the host renders as rows, in order.
    pub records: Vec<Value>,
}

impl CliOutput {
    /// Build output containing one structured record.
    pub fn one(record: Value) -> Self {
        Self {
            records: vec![record],
        }
    }
}

/// Optional display-free command facet implemented by a plugin or WASM adapter.
pub trait PluginCli: Send {
    /// Describe commands so the host can build native help and completions.
    fn cli_schema(&self) -> CliSchema;

    /// Execute one host-validated invocation.
    fn run_cli(&self, invocation: &CliInvocation) -> Result<CliOutput, String>;
}

#[cfg(test)]
mod tests {
    use super::{CliArg, CliArgKind, CliSchema, CliSubcommand};

    #[test]
    fn schema_round_trips_over_the_component_boundary() {
        let schema = CliSchema {
            id: "demo".into(),
            about: "Demo commands".into(),
            examples: vec![],
            subcommands: vec![CliSubcommand {
                name: "show".into(),
                about: "Show a record".into(),
                args: vec![CliArg {
                    id: "pretty".into(),
                    help: "Pretty-print output".into(),
                    required: false,
                    kind: CliArgKind::Flag {
                        long: "pretty".into(),
                        short: Some('p'),
                    },
                }],
                examples: vec![],
            }],
        };

        let json = serde_json::to_string(&schema).unwrap();
        assert_eq!(serde_json::from_str::<CliSchema>(&json).unwrap(), schema);
        schema.validate().unwrap();
    }

    #[test]
    fn schema_rejects_duplicate_subcommands() {
        let command = CliSubcommand {
            name: "show".into(),
            about: String::new(),
            args: vec![],
            examples: vec![],
        };
        let schema = CliSchema {
            id: "demo".into(),
            about: String::new(),
            examples: vec![],
            subcommands: vec![command.clone(), command],
        };

        assert!(
            schema
                .validate()
                .unwrap_err()
                .contains("duplicate subcommand")
        );
    }
}
