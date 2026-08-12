//! WIT-backed adapter for a plugin's optional display-free CLI facet.
//!
//! The SDK's [`PluginCli`](thoth_plugin_sdk::cli::PluginCli) trait is an
//! authoring API. This module is the actual process boundary: schemas and
//! invocations cross the WASM component interface as versioned JSON payloads.

use std::{
    collections::HashMap,
    io::{Read, Write},
    path::Path,
    sync::Mutex,
};

use thoth_plugin_sdk::cli::{CliInvocation, CliOutput, CliSchema, PluginCli};
use wasmtime::component::{Component, HasSelf, Linker};
use wasmtime::{Engine, Store};
use wasmtime_wasi::{ResourceTable, WasiCtx, WasiCtxBuilder, WasiCtxView};

use crate::error::{Result, ThothError};
use crate::{
    app::persistent_state::PersistentState,
    plugin::{
        network_policy::{CheckOutcome, NetworkPolicy},
        wasm_data_source::{BoxIo, ReadWrite, TCP_READ_CAP, secret_store, tcp_connect, tcp_tls},
    },
};

wasmtime::component::bindgen!({
    path: "wit/thoth-plugin.wit",
    world: "cli-plugin",
});

const CLI_FUEL_BUDGET: u64 = 5_000_000_000;

struct CliState {
    wasi: WasiCtx,
    table: ResourceTable,
    plugin_id: String,
    policy: NetworkPolicy,
    tcp_streams: HashMap<u64, Box<dyn ReadWrite>>,
    next_tcp_id: u64,
}

impl wasmtime_wasi::WasiView for CliState {
    fn ctx(&mut self) -> WasiCtxView<'_> {
        WasiCtxView {
            ctx: &mut self.wasi,
            table: &mut self.table,
        }
    }
}

struct Inner {
    store: Store<CliState>,
    bindings: CliPlugin,
}

/// Live adapter whose calls are backed exclusively by `plugin-cli` WIT exports.
pub struct WasmCliLoader {
    inner: Mutex<Inner>,
    schema: CliSchema,
}

impl WasmCliLoader {
    /// Instantiate a component that exports the opt-in `cli-plugin` world and
    /// validate its schema before registering any clap commands.
    pub fn open(
        engine: &Engine,
        wasm_path: &Path,
        plugin_id: &str,
        policy: NetworkPolicy,
        _settings: &[crate::settings::PluginSettingData],
    ) -> Result<Self> {
        let load_error = |reason: String| ThothError::PluginLoadError {
            path: wasm_path.to_path_buf(),
            reason,
        };
        let mut store = Store::new(
            engine,
            CliState {
                wasi: WasiCtxBuilder::new().inherit_stderr().build(),
                table: ResourceTable::new(),
                plugin_id: plugin_id.to_string(),
                policy,
                tcp_streams: HashMap::new(),
                next_tcp_id: 1,
            },
        );
        store
            .set_fuel(CLI_FUEL_BUDGET)
            .map_err(|error| load_error(error.to_string()))?;
        let component = Component::from_file(engine, wasm_path)
            .map_err(|error| load_error(error.to_string()))?;
        let mut linker = Linker::<CliState>::new(engine);
        // A combined plugin world can contain imports irrelevant to schema
        // discovery. They remain traps; a concrete adapter such as Seshat's
        // supplies the imports its CLI execution actually needs.
        linker.allow_shadowing(true);
        linker
            .define_unknown_imports_as_traps(&component)
            .map_err(|error| load_error(error.to_string()))?;
        // Register real services after the catch-all traps so their compatible
        // interface versions shadow exact-version traps generated for the
        // component. This matters for WASI: components currently import 0.2.3,
        // while wasmtime-wasi exposes the compatible 0.2.6 host implementation.
        wasmtime_wasi::p2::add_to_linker_sync(&mut linker)
            .map_err(|error| load_error(error.to_string()))?;
        add_wasi_random_0_2_3(&mut linker).map_err(|error| load_error(error.to_string()))?;
        thoth::plugin::plugin_storage::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)
            .map_err(|error| load_error(error.to_string()))?;
        thoth::plugin::secure_storage::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)
            .map_err(|error| load_error(error.to_string()))?;
        thoth::plugin::http_client::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)
            .map_err(|error| load_error(error.to_string()))?;
        thoth::plugin::tcp_client::add_to_linker::<_, HasSelf<_>>(&mut linker, |state| state)
            .map_err(|error| load_error(error.to_string()))?;
        let bindings = CliPlugin::instantiate(&mut store, &component, &linker)
            .map_err(|error| load_error(error.to_string()))?;
        let schema_json = bindings
            .thoth_plugin_plugin_cli()
            .call_schema(&mut store)
            .map_err(|error| load_error(error.to_string()))?
            .map_err(|error| load_error(error.message))?;
        let schema: CliSchema = serde_json::from_str(&schema_json).map_err(|error| {
            load_error(format!("plugin returned an invalid CLI schema: {error}"))
        })?;
        schema.validate().map_err(|error| {
            load_error(format!("plugin returned an invalid CLI schema: {error}"))
        })?;

        Ok(Self {
            inner: Mutex::new(Inner { store, bindings }),
            schema,
        })
    }
}

/// cargo-component's Preview 1 adapter currently imports this exact WASI
/// interface version. Wasmtime 43 registers 0.2.6, which is API-compatible but
/// does not replace an exact-version fallback trap, so provide the two random
/// functions under the component's exact import name as well.
fn add_wasi_random_0_2_3(linker: &mut Linker<CliState>) -> wasmtime::Result<()> {
    const MAX_RANDOM_BYTES: u64 = 64 << 20;
    let mut random = linker.instance("wasi:random/random@0.2.3")?;
    random.func_wrap(
        "get-random-bytes",
        |_store, (len,): (u64,)| -> wasmtime::Result<(Vec<u8>,)> {
            if len > MAX_RANDOM_BYTES {
                return Err(wasmtime::Error::msg(format!(
                    "requested {len} random bytes, maximum is {MAX_RANDOM_BYTES}"
                )));
            }
            let mut bytes = vec![0; len as usize];
            getrandom::fill(&mut bytes).map_err(|error| wasmtime::Error::msg(error.to_string()))?;
            Ok((bytes,))
        },
    )?;
    random.func_wrap(
        "get-random-u64",
        |_store, (): ()| -> wasmtime::Result<(u64,)> {
            getrandom::u64()
                .map(|value| (value,))
                .map_err(|error| wasmtime::Error::msg(error.to_string()))
        },
    )?;
    Ok(())
}

impl thoth::plugin::plugin_storage::Host for CliState {
    fn read(&mut self) -> String {
        PersistentState::plugin_state_path(&self.plugin_id)
            .ok()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .unwrap_or_default()
    }

    fn write(&mut self, data: String) -> std::result::Result<(), String> {
        let path = PersistentState::plugin_state_path(&self.plugin_id)
            .map_err(|error| error.to_string())?;
        std::fs::write(path, data).map_err(|error| error.to_string())
    }
}

fn secure_error(message: impl Into<String>) -> thoth::plugin::secure_storage::PluginError {
    thoth::plugin::secure_storage::PluginError {
        code: 1,
        message: message.into(),
    }
}

impl thoth::plugin::secure_storage::Host for CliState {
    fn write(
        &mut self,
        key: String,
        secret: String,
    ) -> std::result::Result<(), thoth::plugin::secure_storage::PluginError> {
        secret_store::write(&format!("{}:{key}", self.plugin_id), &secret).map_err(secure_error)
    }

    fn read(
        &mut self,
        key: String,
    ) -> std::result::Result<Option<String>, thoth::plugin::secure_storage::PluginError> {
        secret_store::read(&format!("{}:{key}", self.plugin_id)).map_err(secure_error)
    }

    fn delete(
        &mut self,
        key: String,
    ) -> std::result::Result<(), thoth::plugin::secure_storage::PluginError> {
        secret_store::delete(&format!("{}:{key}", self.plugin_id)).map_err(secure_error)
    }
}

fn http_error(code: u32, message: impl Into<String>) -> thoth::plugin::http_client::PluginError {
    thoth::plugin::http_client::PluginError {
        code,
        message: message.into(),
    }
}

fn execute_http_request(
    req: thoth::plugin::http_client::HttpRequest,
) -> std::result::Result<thoth::plugin::http_client::HttpResponse, String> {
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|error| error.to_string())?;
    let mut builder = match req.method.to_uppercase().as_str() {
        "POST" => client.post(&req.url),
        "PUT" => client.put(&req.url),
        "PATCH" => client.patch(&req.url),
        "DELETE" => client.delete(&req.url),
        _ => client.get(&req.url),
    };
    for (key, value) in req.headers {
        builder = builder.header(key, value);
    }
    if let Some(body) = req.body {
        builder = builder.body(body);
    }
    let response = builder.send().map_err(|error| error.to_string())?;
    let status = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .map(|(key, value)| {
            (
                key.to_string(),
                value.to_str().unwrap_or_default().to_string(),
            )
        })
        .collect();
    let body = response
        .bytes()
        .map_err(|error| error.to_string())?
        .to_vec();
    Ok(thoth::plugin::http_client::HttpResponse {
        status,
        headers,
        body,
    })
}

impl thoth::plugin::http_client::Host for CliState {
    fn fetch(
        &mut self,
        req: thoth::plugin::http_client::HttpRequest,
    ) -> std::result::Result<
        thoth::plugin::http_client::HttpResponse,
        thoth::plugin::http_client::PluginError,
    > {
        match self.policy.check_data_source(&req.url) {
            Ok(CheckOutcome::Allowed) => {
                execute_http_request(req).map_err(|error| http_error(1, error))
            }
            Ok(CheckOutcome::NeedsConsent { domain }) => Err(http_error(
                403,
                format!(
                    "host '{domain}' is not allowed; approve it in Thoth's plugin network settings"
                ),
            )),
            Err(violation) => Err(http_error(403, format!("blocked: {violation:?}"))),
        }
    }

    fn submit(&mut self, _req: thoth::plugin::http_client::HttpRequest) -> String {
        "unsupported-in-cli".to_string()
    }
}

fn tcp_error(code: u32, message: impl Into<String>) -> thoth::plugin::tcp_client::PluginError {
    thoth::plugin::tcp_client::PluginError {
        code,
        message: message.into(),
    }
}

impl thoth::plugin::tcp_client::Host for CliState {
    fn connect(
        &mut self,
        host: String,
        port: u16,
        tls: bool,
    ) -> std::result::Result<u64, thoth::plugin::tcp_client::PluginError> {
        match self.policy.check_tcp(&host) {
            Ok(CheckOutcome::Allowed) => {}
            Ok(CheckOutcome::NeedsConsent { domain }) => {
                return Err(tcp_error(
                    403,
                    format!(
                        "host '{domain}' is not allowed; approve it in Thoth's plugin network settings"
                    ),
                ));
            }
            Err(violation) => {
                return Err(tcp_error(403, format!("blocked: {violation:?}")));
            }
        }
        let tcp = tcp_connect(&host, port)
            .map_err(|error| tcp_error(1, format!("connect failed: {error}")))?;
        let stream: Box<dyn ReadWrite> = if tls {
            tcp_tls(tcp, &host).map_err(|error| tcp_error(2, error))?
        } else {
            Box::new(tcp)
        };
        let id = self.next_tcp_id;
        self.next_tcp_id += 1;
        self.tcp_streams.insert(id, stream);
        Ok(id)
    }

    fn read(
        &mut self,
        stream: u64,
        max: u32,
    ) -> std::result::Result<Vec<u8>, thoth::plugin::tcp_client::PluginError> {
        let stream = self
            .tcp_streams
            .get_mut(&stream)
            .ok_or_else(|| tcp_error(4, "invalid stream id"))?;
        let mut buffer = vec![0; (max as usize).min(TCP_READ_CAP)];
        let read = stream
            .read(&mut buffer)
            .map_err(|error| tcp_error(2, error.to_string()))?;
        buffer.truncate(read);
        Ok(buffer)
    }

    fn write(
        &mut self,
        stream: u64,
        bytes: Vec<u8>,
    ) -> std::result::Result<u32, thoth::plugin::tcp_client::PluginError> {
        let stream = self
            .tcp_streams
            .get_mut(&stream)
            .ok_or_else(|| tcp_error(4, "invalid stream id"))?;
        stream
            .write_all(&bytes)
            .and_then(|()| stream.flush())
            .map_err(|error| tcp_error(2, error.to_string()))?;
        Ok(bytes.len() as u32)
    }

    fn start_tls(
        &mut self,
        stream: u64,
        host: String,
    ) -> std::result::Result<(), thoth::plugin::tcp_client::PluginError> {
        let plain = self
            .tcp_streams
            .remove(&stream)
            .ok_or_else(|| tcp_error(4, "invalid stream id"))?;
        let tls = tcp_tls(BoxIo(plain), &host).map_err(|error| tcp_error(2, error))?;
        self.tcp_streams.insert(stream, tls);
        Ok(())
    }

    fn close(&mut self, stream: u64) {
        self.tcp_streams.remove(&stream);
    }
}

impl PluginCli for WasmCliLoader {
    fn cli_schema(&self) -> CliSchema {
        self.schema.clone()
    }

    fn run_cli(&self, invocation: &CliInvocation) -> std::result::Result<CliOutput, String> {
        let invocation_json = serde_json::to_string(invocation)
            .map_err(|error| format!("failed to encode CLI invocation: {error}"))?;
        let mut inner = self.inner.lock().unwrap_or_else(|error| error.into_inner());
        let Inner { store, bindings } = &mut *inner;
        store
            .set_fuel(CLI_FUEL_BUDGET)
            .map_err(|error| format!("failed to refuel plugin: {error}"))?;
        let output_json = bindings
            .thoth_plugin_plugin_cli()
            .call_run(store, &invocation_json)
            .map_err(|error| format!("plugin CLI trapped: {error}"))?
            .map_err(|error| error.message)?;
        serde_json::from_str(&output_json)
            .map_err(|error| format!("plugin returned invalid CLI output: {error}"))
    }
}
