//! Display-free command runtime for CLI and agent callers.

use std::{collections::HashSet, time::Duration};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use thoth_plugin_sdk::cli::{CliInvocation, CliOutput, CliSchema, PluginCli};

use crate::{
    core::ThothCore,
    plugin::{
        Capability, NetworkDeclarations,
        network_policy::NetworkPolicy,
        plugin_ui_host::{PluginCore, PluginCoreEvent},
        runtime::PluginRuntimeState,
    },
    settings::Settings,
};

/// A command accepted by [`HeadlessRuntime`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CliCommand {
    /// Return metadata for every discovered plugin.
    ListPlugins,
    /// Route a structured command to one live plugin adapter.
    Plugin {
        /// Target plugin id.
        plugin_id: String,
        /// Plugin-defined command name.
        command: String,
        /// Structured command arguments.
        #[serde(default)]
        args: Value,
    },
}

/// Machine-readable result of a headless invocation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HeadlessResult {
    /// Process exit code: zero for success, non-zero for failure.
    pub exit_code: i32,
    /// JSON value intended for stdout on success.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stdout: Option<Value>,
    /// Human-readable diagnostic intended for stderr on failure.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
}

impl HeadlessResult {
    fn success(value: Value) -> Self {
        Self {
            exit_code: 0,
            stdout: Some(value),
            stderr: None,
        }
    }

    fn failure(message: impl Into<String>) -> Self {
        Self {
            exit_code: 1,
            stdout: None,
            stderr: Some(message.into()),
        }
    }
}

/// Runs core and plugin commands without constructing an egui context or window.
pub struct HeadlessRuntime {
    /// Display-independent application state and services.
    pub core: ThothCore,
    plugins: Vec<Box<dyn PluginCore>>,
    cli_plugins: Vec<Box<dyn PluginCli>>,
    initialized: HashSet<String>,
    plugin_init_timeout: Duration,
}

impl HeadlessRuntime {
    /// Bootstrap a headless core and begin plugin discovery.
    pub fn new(settings: Settings) -> Self {
        Self::with_plugins(settings, Vec::new())
    }

    /// Bootstrap with explicit live plugin adapters.
    ///
    /// This is the integration seam used by plugin CLI adapters and tests.
    pub fn with_plugins(settings: Settings, plugins: Vec<Box<dyn PluginCore>>) -> Self {
        Self::with_adapters(settings, plugins, Vec::new())
    }

    /// Bootstrap with explicit core and CLI adapters.
    pub fn with_adapters(
        settings: Settings,
        plugins: Vec<Box<dyn PluginCore>>,
        cli_plugins: Vec<Box<dyn PluginCli>>,
    ) -> Self {
        let core = ThothCore::init(settings);
        core.plugins.install_as_active();
        core.datasets.install_as_active();
        core.plugins.start(
            core.settings.plugins.enabled,
            core.settings.plugins.plugin_settings.clone(),
        );
        Self {
            core,
            plugins,
            cli_plugins,
            initialized: HashSet::new(),
            plugin_init_timeout: Duration::from_secs(10),
        }
    }

    /// Collect the schemas declared by all opt-in CLI adapters.
    pub fn cli_schemas(&self) -> Vec<CliSchema> {
        self.cli_plugins
            .iter()
            .map(|plugin| plugin.cli_schema())
            .collect()
    }

    /// Discover every manifest that declares the CLI capability and load its
    /// adapter through the `plugin-cli` WIT interface.
    pub fn discover_cli_plugins(&mut self) -> std::result::Result<(), String> {
        let deadline = std::time::Instant::now() + self.plugin_init_timeout;
        let manager = loop {
            match self.core.plugins.state() {
                PluginRuntimeState::Ready(manager) => break Some(manager),
                PluginRuntimeState::Disabled => break None,
                PluginRuntimeState::Loading if std::time::Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(1));
                }
                PluginRuntimeState::Loading => {
                    return Err("plugin initialization timed out".into());
                }
            }
        };

        self.cli_plugins.clear();
        let Some(manager) = manager else {
            return Ok(());
        };
        let mut command_ids = HashSet::new();
        for plugin in manager.get_all_plugin_by_capability(Capability::Cli) {
            let user_policy = self
                .core
                .settings
                .plugins
                .network_policies
                .get(&plugin.id)
                .cloned()
                .unwrap_or_default();
            let policy = NetworkPolicy::from_plugin_and_settings(
                plugin
                    .network
                    .as_ref()
                    .unwrap_or(&NetworkDeclarations::default()),
                &user_policy,
            );
            let adapter = manager.open_cli(&plugin.id, policy).map_err(|error| {
                format!("failed to load CLI for plugin '{}': {error}", plugin.id)
            })?;
            let schema = adapter.cli_schema();
            if matches!(schema.id.as_str(), "plugins" | "completions" | "help") {
                return Err(format!(
                    "plugin '{}' uses reserved CLI id '{}'",
                    plugin.id, schema.id
                ));
            }
            if !command_ids.insert(schema.id.clone()) {
                return Err(format!("duplicate plugin CLI id '{}'", schema.id));
            }
            self.cli_plugins.push(Box::new(adapter));
        }
        Ok(())
    }

    /// Route one validated invocation to its plugin CLI adapter.
    pub fn run_cli(
        &self,
        plugin_id: &str,
        invocation: &CliInvocation,
    ) -> std::result::Result<CliOutput, String> {
        let plugin = self
            .cli_plugins
            .iter()
            .find(|plugin| plugin.cli_schema().id == plugin_id)
            .ok_or_else(|| format!("plugin '{plugin_id}' has no CLI commands"))?;
        plugin.run_cli(invocation)
    }

    /// Execute one command to completion without a frame loop.
    pub fn run(&mut self, command: CliCommand) -> HeadlessResult {
        // Drive pending core events before command dispatch. CLI file actions are
        // added by the routing layer in #143; no window-specific action is run here.
        let _ = self.core.tick();

        match command {
            CliCommand::ListPlugins => self.list_plugins(),
            CliCommand::Plugin {
                plugin_id,
                command,
                args,
            } => self.run_plugin_command(&plugin_id, command, args),
        }
    }

    fn list_plugins(&self) -> HeadlessResult {
        let deadline = std::time::Instant::now() + self.plugin_init_timeout;
        loop {
            match self.core.plugins.state() {
                PluginRuntimeState::Ready(manager) => {
                    let plugins: Vec<Value> = manager
                        .get_all_plugin()
                        .into_iter()
                        .map(|plugin| {
                            serde_json::json!({
                                "id": plugin.id,
                                "name": plugin.name,
                                "version": plugin.version,
                                "capabilities": plugin.capabilities,
                            })
                        })
                        .collect();
                    return HeadlessResult::success(serde_json::json!({ "plugins": plugins }));
                }
                PluginRuntimeState::Disabled => {
                    return HeadlessResult::success(serde_json::json!({ "plugins": [] }));
                }
                PluginRuntimeState::Loading if std::time::Instant::now() < deadline => {
                    std::thread::sleep(Duration::from_millis(1));
                }
                PluginRuntimeState::Loading => {
                    return HeadlessResult::failure("plugin initialization timed out");
                }
            }
        }
    }

    fn run_plugin_command(
        &mut self,
        plugin_id: &str,
        command: String,
        args: Value,
    ) -> HeadlessResult {
        let Some(index) = self
            .plugins
            .iter()
            .position(|plugin| plugin.plugin_id() == plugin_id)
        else {
            return HeadlessResult::failure(format!("plugin '{plugin_id}' is not available"));
        };

        if self.initialized.insert(plugin_id.to_string())
            && let Err(error) = self.plugins[index].init()
        {
            self.initialized.remove(plugin_id);
            return HeadlessResult::failure(format!(
                "failed to initialize plugin '{plugin_id}': {error}"
            ));
        }

        let event = PluginCoreEvent::Command {
            name: command,
            args,
        };
        match self.plugins[index].on_event(&event) {
            Ok(Some(value)) => HeadlessResult::success(value),
            Ok(None) => HeadlessResult::failure(format!(
                "plugin '{plugin_id}' does not support this command"
            )),
            Err(error) => {
                HeadlessResult::failure(format!("plugin '{plugin_id}' command failed: {error}"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::{CliCommand, HeadlessRuntime};
    use crate::plugin::plugin_ui_host::{PluginCore, PluginCoreEvent};

    struct MockPlugin {
        init_calls: Arc<AtomicUsize>,
        event_calls: Arc<AtomicUsize>,
    }

    impl PluginCore for MockPlugin {
        fn plugin_id(&self) -> &str {
            "test.mock"
        }

        fn init(&self) -> crate::error::Result<()> {
            self.init_calls.fetch_add(1, Ordering::Relaxed);
            Ok(())
        }

        fn on_event(
            &self,
            event: &PluginCoreEvent,
        ) -> crate::error::Result<Option<serde_json::Value>> {
            self.event_calls.fetch_add(1, Ordering::Relaxed);
            let PluginCoreEvent::Command { name, args } = event;
            Ok(Some(serde_json::json!({ "command": name, "args": args })))
        }
    }

    fn disabled_settings() -> crate::settings::Settings {
        let mut settings = crate::settings::Settings::default();
        settings.plugins.enabled = false;
        settings
    }

    #[test]
    fn runs_plugin_lifecycle_end_to_end_without_a_display() {
        let init_calls = Arc::new(AtomicUsize::new(0));
        let event_calls = Arc::new(AtomicUsize::new(0));
        let plugin = MockPlugin {
            init_calls: Arc::clone(&init_calls),
            event_calls: Arc::clone(&event_calls),
        };
        let mut runtime =
            HeadlessRuntime::with_plugins(disabled_settings(), vec![Box::new(plugin)]);

        let command = || CliCommand::Plugin {
            plugin_id: "test.mock".into(),
            command: "ping".into(),
            args: serde_json::json!({ "value": 42 }),
        };
        let first = runtime.run(command());
        let second = runtime.run(command());

        assert_eq!(first.exit_code, 0);
        assert_eq!(first.stdout.unwrap()["command"], "ping");
        assert_eq!(second.exit_code, 0);
        assert_eq!(init_calls.load(Ordering::Relaxed), 1);
        assert_eq!(event_calls.load(Ordering::Relaxed), 2);
    }

    #[test]
    fn reports_machine_safe_failure_without_stdout() {
        let mut runtime = HeadlessRuntime::new(disabled_settings());
        let result = runtime.run(CliCommand::Plugin {
            plugin_id: "missing".into(),
            command: "ping".into(),
            args: serde_json::json!({}),
        });

        assert_ne!(result.exit_code, 0);
        assert!(result.stdout.is_none());
        assert!(result.stderr.unwrap().contains("missing"));
    }

    #[test]
    fn lists_plugins_with_plugins_disabled() {
        let mut runtime = HeadlessRuntime::new(disabled_settings());
        let result = runtime.run(CliCommand::ListPlugins);

        assert_eq!(result.exit_code, 0);
        assert_eq!(result.stdout.unwrap(), serde_json::json!({ "plugins": [] }));
    }
}
