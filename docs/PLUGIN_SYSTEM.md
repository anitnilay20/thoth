# Plugin System

This document describes Thoth's plugin architecture — how it works, how plugins are discovered and loaded, and how to implement your own plugin.

---

## Table of Contents

- [Why a Plugin System](#why-a-plugin-system)
- [Architecture Overview](#architecture-overview)
- [Technology Choice: WebAssembly](#technology-choice-webassembly)
- [Plugin Types](#plugin-types)
- [Plugin Storage and Discovery](#plugin-storage-and-discovery)
- [Plugin Structure](#plugin-structure)
- [WIT Interface Definitions](#wit-interface-definitions)
- [How the Runtime Works](#how-the-runtime-works)
- [Implementing a File Loader Plugin](#implementing-a-file-loader-plugin)
- [Implementing a File Viewer Plugin](#implementing-a-file-viewer-plugin)
- [Implementing a Data Source Plugin](#implementing-a-data-source-plugin)
- [Async HTTP — Submitting Requests Without Blocking](#async-http--submitting-requests-without-blocking)
- [UiNode DSL Reference](#uinode-dsl-reference)
- [RenderNode Schema Reference](#rendernode-schema-reference)
- [Security Model](#security-model)
- [Integration with Core](#integration-with-core)
- [Roadmap: Database Plugins via WASI Sockets](#roadmap-database-plugins-via-wasi-sockets)

---

## Why a Plugin System

Features like API/network support, database connectivity, and additional file formats (CSV, YAML, XML) were initially planned as core additions. Instead, these are implemented as **plugins** for the following reasons:

- **Core stays fast and minimal** — users only pay for what they install
- **Independent release cycles** — plugins ship on their own schedule
- **Third-party extensibility** — anyone can write a plugin without touching core
- **Clean separation of concerns** — each plugin owns its domain fully

---

## Architecture Overview

```text
┌─────────────────────────────────────────────────┐
│                  ThothApp                        │
│                                                  │
│  ┌──────────────┐     ┌──────────────────────┐  │
│  │ PluginManager│────▶│  PluginRegistry       │  │
│  │              │     │  (in-memory, runtime) │  │
│  └──────┬───────┘     └──────────────────────┘  │
│         │                                        │
│         │ loads & sandboxes                      │
│         ▼                                        │
│  ┌──────────────────────────────────────────┐   │
│  │           Wasmtime Runtime               │   │
│  │                                          │   │
│  │  ┌────────────┐  ┌────────────────────┐  │   │
│  │  │ csv-loader │  │ postgres-source     │  │   │
│  │  │ plugin.wasm│  │ plugin.wasm        │  │   │
│  │  └────────────┘  └────────────────────┘  │   │
│  └──────────────────────────────────────────┘   │
│                                                  │
│  ┌───────────────┐  ┌──────────────────────────┐ │
│  │ WasmFileLoader│  │  WasmFileViewerLoader    │ │
│  │ (file-loader  │  │  (file-loader +          │ │
│  │  world)       │  │   file-viewer world)     │ │
│  └───────────────┘  └──────────────────────────┘ │
└─────────────────────────────────────────────────┘
```

The `PluginManager` is initialized once at app startup, scans all plugin directories, loads each `.wasm` file into its own sandboxed Wasmtime instance, and registers it against the capabilities it declares in `plugin.toml`.

When a file is opened the host checks whether the matching plugin also implements `file-viewer`. If it does, a `WasmFileViewerLoader` is used and the viewer renders either a native table (when the plugin returns `preferred-display: table`) or a custom `RenderNode` tree (when it returns `custom`).

---

## Technology Choice: WebAssembly

Plugins are compiled to **WebAssembly (WASM)** and run inside a **[Wasmtime](https://wasmtime.dev/)** sandbox. The interface between Thoth and plugins is defined using **[WIT (WebAssembly Interface Types)](https://component-model.bytecodealliance.org/design/wit.html)**.

### Comparison with alternatives

| Concern            | WASM + Wasmtime | Dynamic libs (.so/.dylib) | Subprocess (JSON-RPC) |
|--------------------|-----------------|---------------------------|-----------------------|
| Safety / sandbox   | Sandboxed       | Full process access       | Isolated process      |
| Cross-platform     | One binary      | Per-platform build        | Any language          |
| Language agnostic  | Any WASM target | Rust only (stable ABI)    | Any language          |
| Performance        | Near-native     | Native                    | IPC overhead          |
| ABI stability      | Stable via WIT  | Breaks across Rust ver.   | JSON is stable        |
| Distribution       | Single `.wasm`  | Complex packaging         | Single binary         |

WASM wins on portability, safety, and distribution simplicity. A plugin author compiles once and the `.wasm` file runs on macOS, Windows, and Linux without modification.

---

## Plugin Types

| Type | Capability key | What it does |
|---|---|---|
| **File Loader** | `file-loader` | Teaches Thoth to open a new file format (CSV, YAML, Parquet, etc.) |
| **File Viewer** | `file-viewer` | Controls how records are *rendered* — table mode or custom RenderNode tree |
| **Data Source** | `data-source` | Connects to an external source — REST API, database, message queue |
| **UI Component** | `new-ui-component` | Renders a fully interactive panel — owns its own state machine |
| **Exporter** | `exporter` | Adds new export formats or destinations |
| **Search Provider** | `search-provider` | Extends the search experience with custom indexing or remote results |

A single plugin can declare multiple capabilities. `file-viewer` always pairs with `file-loader` — use the `file-viewer-plugin` world for this combination. `data-source` always pairs with `ui-component` — use the `data-source-plugin` world.

---

## Plugin Storage and Discovery

Thoth scans three locations at startup:

| Scope | Path | Purpose |
|---|---|---|
| Bundled | Next to the Thoth binary / inside the app bundle | First-party plugins shipped with Thoth |
| User | `~/.config/thoth/plugins/` (Linux/macOS)  `%APPDATA%\thoth\plugins\` (Windows) | Personal plugins installed by the user |
| Debug | `<workspace>/assets/plugins/` | Dev-only: found by cargo run without a full install |

Discovery algorithm:

1. Walk each directory in order.
2. For each subdirectory that contains `plugin.toml` + `plugin.wasm`, attempt to load it.
3. Read `plugin.toml` to extract `id`, `name`, `version`, and `capabilities`.
4. Validate that the WASM component exports `thoth:plugin/plugin-meta`.
5. Register the plugin under each capability it declares.
6. Skip and log a warning for any plugin that fails validation or sandbox initialization.

Plugins installed via the Settings → Plugins UI land in the user directory. Bundled plugins cannot be uninstalled but can be disabled via the toggle in Settings → Plugins.

---

## Plugin Structure

A plugin is a **directory** containing:

```text
~/.config/thoth/plugins/
└── csv-loader/
    ├── plugin.toml   ← required metadata
    ├── plugin.wasm   ← compiled WASM component
    └── icon.png      ← optional 64×64 icon shown in Settings
```

### `plugin.toml` format

```toml
id          = "com.example.csv-loader"   # Reverse-domain unique identifier
name        = "CSV Loader"
version     = "0.2.1"
description = "Load CSV and TSV files as tabular JSON records"
author      = "Your Name <you@example.com>"
homepage    = "https://github.com/example/csv-loader"   # optional

capabilities = ["file-loader", "file-viewer"]

# One [[file-loader]] block per distinct MIME type / extension group
[[file-loader]]
file-type            = "text/csv"
supported-extensions = ["csv"]

[[file-loader]]
file-type            = "text/tab-separated-values"
supported-extensions = ["tsv"]
```

---

## WIT Interface Definitions

The full interface is defined in `wit/thoth-plugin.wit` at the repository root. This is the **single source of truth** — all language toolchains generate their bindings from this file.

### Shared types

```wit
interface types {
    enum capability {
        file-loader,
        file-viewer,
        data-source,
        exporter,
        search-provider,
        new-ui-component,
    }

    record plugin-info {
        id:           string,
        name:         string,
        version:      string,
        description:  string,
        capabilities: list<capability>,
        author:       option<string>,
        homepage:     option<string>,
        icon:         option<string>,   // Phosphor icon Unicode glyph
    }

    record plugin-error {
        code:    u32,
        message: string,
    }

    record setting-data {
        key:   string,
        value: string,
    }
}
```

### `plugin-meta` — required by every plugin

```wit
interface plugin-meta {
    use types.{plugin-info};

    get-info: func() -> plugin-info;
}
```

### `plugin-lifecycle` — required by every plugin

```wit
interface plugin-lifecycle {
    /// Called after the plugin is fully loaded and registered.
    /// `setting` is a JSON array of {key, value} objects with the plugin's
    /// persisted settings, e.g. [{"key":"url","value":"https://..."}].
    /// Parse it to restore saved state.
    on-load: func(setting: string);

    /// Called before the plugin is unloaded. Release held resources.
    on-close: func();

    /// Called automatically by the host whenever the user saves settings while
    /// this plugin's pane is active. `setting` is the same JSON array format
    /// as on-load. This covers both plugin-specific settings changes and any
    /// other settings save while the plugin is open.
    on-setting-change: func(setting: string);
}
```

### `plugin-settings` — required by every plugin

```wit
interface plugin-settings {
    use types.{plugin-error};

    record settings-output {
        node-json:   string,   // JSON-encoded UiNode tree
        height-hint: u32,
    }

    /// Render the settings UI. Called when the user opens Settings → Plugins
    /// for this plugin. Return a UiNode tree using the same DSL as ui-component.
    /// Return a plain text node if the plugin has no configurable settings.
    render-settings: func() -> result<settings-output, plugin-error>;
}
```

### `http-client` — host-provided import for data-source plugins

Data-source plugins get outbound HTTP access via this host-provided import. All requests pass through the host's network-policy layer (domain allowlist, SSRF guard) before being forwarded.

```wit
interface http-client {
    use types.{plugin-error};

    record http-request {
        url:     string,
        method:  string,                        // "GET" | "POST" | "PUT" | "PATCH" | "DELETE"
        headers: list<tuple<string, string>>,
        body:    option<list<u8>>,
    }

    record http-response {
        status:  u16,
        headers: list<tuple<string, string>>,
        body:    list<u8>,
    }

    /// Synchronous fetch. Blocks until the response arrives.
    /// Use for programmatic paths (schema discovery, initial data load) where
    /// showing a spinner is not required.
    fetch: func(req: http-request) -> result<http-response, plugin-error>;

    /// Asynchronous submit. Returns a request-id string immediately without
    /// blocking. The host dispatches the request on a background thread and
    /// delivers the result by calling handle-event with:
    ///   widget-id = <request-id>
    ///   kind      = "http-response"
    ///   value     = JSON: {"ok":{"status":200,"headers":[["k","v"]],"body":"...","duration_ms":42}}
    ///            or JSON: {"err":{"code":"error","message":"..."}}
    ///            or JSON: {"err":{"code":"consent_pending","message":"domain '...' not approved"}}
    ///
    /// The "consent_pending" code means the user has not yet approved the
    /// domain. A consent popup is shown; once approved, the host re-dispatches
    /// the request and delivers a new http-response event. While waiting, keep
    /// loading=true and show a "Waiting for consent approval…" spinner.
    ///
    /// Use submit() when you want to show a spinner while waiting. Store the
    /// request-id, set a loading flag in your state, and return a UI tree with
    /// a {"type":"spinner"} node. When handle-event fires for that request-id,
    /// clear the loading flag and render the result.
    submit: func(req: http-request) -> string;
}
```

### `file-loader` — capability: `file-loader`

```wit
interface file-loader {
    use types.{plugin-error};

    supported-extensions: func() -> list<string>;
    open:      func(path: string)  -> result<u64, plugin-error>;
    get:       func(idx: u64)      -> result<string, plugin-error>;
    raw-bytes: func(idx: u64)      -> result<list<u8>, plugin-error>;
}
```

`open` indexes the file and returns the total record count. `get` returns a single record as a JSON object string. `raw-bytes` returns the same record as raw UTF-8 bytes (used by copy-to-clipboard and exporters).

### `file-viewer` — capability: `file-viewer`

```wit
interface file-viewer {
    use types.{plugin-error};

    enum display-mode {
        table,    // host renders a native table; render-record() never called
        custom,   // host calls render-record() for every visible row
    }

    preferred-display:  func() -> display-mode;
    column-headers:     func() -> option<list<string>>;
    render-record:      func(record-json: string) -> result<render-output, plugin-error>;

    record render-output {
        node-json:   string,   // JSON-encoded RenderNode tree
        height-hint: u32,
    }
}
```

### `data-source` — capability: `data-source`

```wit
interface data-source {
    use types.{plugin-error};

    record config-entry   { name: string, description: string, required: bool, value: string }
    record field-schema   { name: string, type-hint: string, nullable: bool }
    record source-schema  { name: string, fields: list<field-schema> }
    record pane-output    { node-json: string, height-hint: u32 }

    required-config: func() -> list<config-entry>;
    connect:         func(config: list<config-entry>) -> result<string, plugin-error>;
    schema:          func(handle: string)             -> result<list<source-schema>, plugin-error>;
    query:           func(handle: string, q: string)  -> result<string, plugin-error>;
    close:           func(handle: string);
    render-pane:     func(handle: string)             -> result<pane-output, plugin-error>;
}
```

### `ui-component` — capability: `new-ui-component`

```wit
interface ui-component {
    use types.{plugin-error};

    record ui-event {
        widget-id: string,
        /// "click"         — button / icon-button pressed (value is empty)
        /// "change"        — any input value changed (JSON-encoded new value)
        /// "http-response" — async HTTP result from submit() (JSON payload)
        /// "notify"        — host notification, e.g. "consent-approved"
        kind:      string,
        value:     string,
    }

    record ui-output {
        node-json:   string,
        height-hint: u32,
    }

    render-ui:      func()               -> result<ui-output, plugin-error>;
    handle-event:   func(event: ui-event) -> result<ui-output, plugin-error>;
    /// Optional sidebar panel. Return some(output) to show a sidebar slot.
    /// Return none if this plugin has no sidebar content.
    /// Called after every handle-event to keep the sidebar in sync.
    render-sidebar: func()               -> result<option<ui-output>, plugin-error>;
}
```

### Worlds — pick the one that matches your plugin

```wit
world base-plugin {           // every plugin satisfies this
    export plugin-meta;
    export plugin-lifecycle;
    export plugin-settings;
}

world file-loader-plugin {    // file format support only
    include base-plugin;
    export file-loader;
}

world file-viewer-plugin {    // file format + custom rendering
    include base-plugin;
    export file-loader;
    export file-viewer;
}

world ui-component-plugin {   // standalone interactive panel
    include base-plugin;
    export ui-component;
}

world data-source-plugin {    // external data source + interactive UI
    include base-plugin;
    import http-client;        // host provides outbound HTTP
    export data-source;
    export ui-component;
}

world exporter-plugin {
    include base-plugin;
    export exporter;
}
```

---

## How the Runtime Works

### Startup sequence

```text
ThothApp::new()
    └── PluginManager::init()
            ├── scan bundled_plugins_dir
            ├── scan user_plugins_dir
            └── (debug) scan assets/plugins/
                    └── for each plugin directory:
                            ├── read plugin.toml  → Plugin struct
                            ├── set plugin.bundled = true/false
                            ├── compile .wasm with wasmtime Engine
                            ├── validate: component exports thoth:plugin/plugin-meta
                            ├── link WASI host functions
                            ├── instantiate (validates all imports satisfied)
                            └── register in PluginRegistry
```

### Per-call flow (file-loader example)

```text
FileViewer::open(path)
    └── PluginManager::plugin_has_capability("csv", FileViewer) → true
    └── PluginManager::open_file_with_viewer("csv", path)
            └── WasmFileViewerLoader::open(engine, wasm_path, path)
                    ├── WASI: preopened dir = file's parent (read-only)
                    ├── set fuel = u64::MAX / 2
                    └── wasmtime call: file-loader.open(path) → record_count

Per frame (virtual scrolling — only visible rows):
    PluginTableViewer::render()
        ├── loader.column_headers()    → ["Name", "Age", ...]  [called once, cached]
        ├── loader.preferred_display() → Table                  [called once, cached]
        └── for each visible row idx:
                loader.get(idx)        → serde_json::Value      [cached in LRU]
                (Table mode: host renders cells natively)
```

### Data-source plugin open flow

```text
PluginManager::open_data_source(plugin_id)
    └── WasmDataSourceLoader::open(engine, wasm_path, policy, plugin_id, settings)
            ├── instantiate component
            ├── if settings non-empty: call on-load(settings_json)
            │       └── plugin parses JSON, initialises internal state
            └── return WasmDataSourceLoader

ThothApp::ui() — called every frame:
    └── poll_plugin_http_results(ctx)
            ├── drain_http_results() from background threads
            ├── for each (request_id, outcome):
            │       build UiEvent { widget_id: request_id, kind: "http-response", value: json }
            │       loader.handle_event(event) → new UiOutput
            └── if has_pending_http(): ctx.request_repaint()
```

### Settings-change notification flow

```text
ThothApp — user clicks "Save" in Settings dialog
    ├── settings saved to disk
    ├── self.settings updated in memory
    ├── PluginManager::update_plugin_settings(new_plugin_settings)
    │       └── overwrites internal RwLock<HashMap> with latest per-plugin values
    └── if active_plugin_pane exists:
            └── WasmDataSourceLoader::on_setting_change(updated_settings)
                    └── wasmtime call: plugin-lifecycle.on-setting-change(settings_json)
                            └── plugin re-parses JSON, updates internal state
```

> If `plugins.enabled` changed, a toast notification is shown and no further plugin calls
> are made — the change takes effect only after the app is restarted.

### Fuel replenishment

Each WIT call replenishes fuel to **5,000,000,000 units** (`PLUGIN_FUEL_BUDGET`) before calling into the plugin via the `refuel()` helper in `wasm_data_source.rs`. This gives plugins ample budget for serialising large JSON responses while still bounding runaway infinite loops. Fuel exhaustion surfaces as `ThothError::Unknown` with a "all fuel consumed" message.

---

## Implementing a File Loader Plugin

This walkthrough creates a CSV plugin in Rust using `cargo-component`.

### 1. Install tooling

```bash
cargo install cargo-component
rustup target add wasm32-wasip1
```

### 2. Scaffold the plugin

```bash
cargo component new --lib csv-loader
cd csv-loader
```

### 3. Configure `Cargo.toml`

```toml
[package]
name    = "csv-loader"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
wit-bindgen-rt = "0.41"
serde_json     = "1"

[package.metadata.component]
package = "com.example:csv-loader"

[package.metadata.component.target]
path  = "../../wit"
world = "file-loader-plugin"
```

### 4. Implement the plugin (`src/lib.rs`)

```rust
mod bindings;

use std::cell::RefCell;
use bindings::exports::thoth::plugin::{
    file_loader::Guest as FileLoaderGuest,
    plugin_lifecycle::Guest as LifecycleGuest,
    plugin_meta::Guest as MetaGuest,
    plugin_settings::{Guest as SettingsGuest, SettingsOutput},
};
use bindings::thoth::plugin::types::{Capability, PluginError};

struct CsvPlugin;

struct State {
    headers: Vec<String>,
    records: Vec<Vec<String>>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = const { RefCell::new(None) };
}

impl MetaGuest for CsvPlugin {
    fn get_info() -> bindings::exports::thoth::plugin::plugin_meta::PluginInfo {
        bindings::exports::thoth::plugin::plugin_meta::PluginInfo {
            id:           "com.example.csv-loader".to_string(),
            name:         "CSV Loader".to_string(),
            version:      "0.1.0".to_string(),
            description:  "Load CSV files as JSON records".to_string(),
            capabilities: vec![Capability::FileLoader],
            author:       Some("Your Name <you@example.com>".to_string()),
            homepage:     None,
            icon:         None,
        }
    }
}

impl LifecycleGuest for CsvPlugin {
    fn on_load(_setting: String) {}   // no saved settings for this plugin
    fn on_close() {
        STATE.with(|s| *s.borrow_mut() = None);
    }
    fn on_setting_change(_setting: String) {}
}

impl SettingsGuest for CsvPlugin {
    fn render_settings() -> Result<SettingsOutput, PluginError> {
        Ok(SettingsOutput {
            node_json: r#"{"type":"text","value":"No configurable settings.","muted":true}"#.into(),
            height_hint: 0,
        })
    }
}

impl FileLoaderGuest for CsvPlugin {
    fn supported_extensions() -> Vec<String> {
        vec!["csv".to_string(), "tsv".to_string()]
    }

    fn open(path: String) -> Result<u64, PluginError> {
        // ... parse and index the file
        Ok(record_count)
    }

    fn get(idx: u64) -> Result<String, PluginError> {
        // ... return record at idx as JSON object string
    }

    fn raw_bytes(idx: u64) -> Result<Vec<u8>, PluginError> {
        // ... return raw bytes for clipboard/export
    }
}

bindings::export!(CsvPlugin with_types_in bindings);
```

### 5. Build and install

```bash
cargo component build --release
mkdir -p ~/.config/thoth/plugins/csv-loader
cp target/wasm32-wasip1/release/csv_loader.wasm ~/.config/thoth/plugins/csv-loader/plugin.wasm
cp plugin.toml ~/.config/thoth/plugins/csv-loader/plugin.toml
```

Restart Thoth — opening a `.csv` file will now use your plugin.

---

## Implementing a File Viewer Plugin

A file viewer plugin controls *how* records are displayed in the viewer panel. It always pairs with `file-loader` — use the `file-viewer-plugin` world.

Change the world in `Cargo.toml`:

```toml
[package.metadata.component.target]
path  = "../../wit"
world = "file-viewer-plugin"
```

**Table mode** (simplest — host renders the table natively):

```rust
impl FileViewerGuest for MyPlugin {
    fn preferred_display() -> DisplayMode { DisplayMode::Table }

    fn column_headers() -> Option<Vec<String>> {
        STATE.with(|s| s.borrow().as_ref().map(|st| st.headers.clone()))
    }

    fn render_record(_record_json: String) -> Result<RenderOutput, PluginError> {
        Err(PluginError { code: 0, message: "not used in table mode".into() })
    }
}
```

**Custom mode** (full control via RenderNode tree):

```rust
impl FileViewerGuest for MyPlugin {
    fn preferred_display() -> DisplayMode { DisplayMode::Custom }

    fn column_headers() -> Option<Vec<String>> { None }

    fn render_record(record_json: String) -> Result<RenderOutput, PluginError> {
        let map: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&record_json).map_err(|e| PluginError {
                code: 1, message: e.to_string(),
            })?;

        let children: Vec<serde_json::Value> = map.iter().map(|(_k, v)| {
            let text = v.as_str().unwrap_or("").to_string();
            if text.contains("ERROR") {
                serde_json::json!({"type":"colored","color":"#ff6b6b",
                                   "child":{"type":"text","value":text}})
            } else {
                serde_json::json!({"type":"text","value":text})
            }
        }).collect();

        let node_json = serde_json::json!({"type":"row","children":children}).to_string();
        Ok(RenderOutput { node_json, height_hint: 0 })
    }
}
```

Update `plugin.toml`:

```toml
capabilities = ["file-loader", "file-viewer"]
```

---

## Implementing a Data Source Plugin

A data source plugin uses the `data-source-plugin` world which combines `data-source` + `ui-component` + the `http-client` host import.

The plugin renders its own connection/query UI via `render-ui` / `handle-event`, and exposes data via `connect` / `query` / `close`.

### Settings initialisation via `on-load`

The host calls `on-load(setting)` after instantiation, passing the plugin's persisted settings as a JSON array. Parse it to restore saved state:

```rust
fn on_load(setting: String) {
    if setting.is_empty() { return; }
    if let Ok(entries) = serde_json::from_str::<Vec<serde_json::Value>>(&setting) {
        STATE.with(|s| {
            let mut st = s.borrow().clone();
            for entry in entries {
                let key   = entry["key"].as_str().unwrap_or("");
                let value = entry["value"].as_str().unwrap_or("").to_string();
                match key {
                    "url"    => st.url    = value,
                    "method" => st.method = value,
                    _        => {}
                }
            }
            *s.borrow_mut() = st;
        });
    }
}
```

### Minimal data-source skeleton

```rust
impl DataSourceGuest for MyPlugin {
    fn required_config() -> Vec<ConfigEntry> {
        vec![ConfigEntry {
            name: "url".into(), description: "Base URL".into(),
            required: true,     value: "".into(),
        }]
    }

    fn connect(config: Vec<ConfigEntry>) -> Result<String, PluginError> {
        let url = config.iter().find(|e| e.name == "url")
            .ok_or(PluginError { code: 1, message: "missing url".into() })?;
        STATE.with(|s| s.borrow_mut().base_url = url.value.clone());
        Ok("handle-1".to_string())
    }

    fn schema(_handle: String) -> Result<Vec<SourceSchema>, PluginError> {
        Ok(vec![]) // return table/endpoint list
    }

    fn query(_handle: String, _q: String) -> Result<String, PluginError> {
        Ok("[]".to_string())
    }

    fn close(_handle: String) {}

    fn render_pane(_handle: String) -> Result<PaneOutput, PluginError> {
        // return a RenderNode tree for the main pane
        Ok(PaneOutput { node_json: r#"{"type":"text","value":"No data"}"#.into(), height_hint: 0 })
    }
}
```

---

## Async HTTP — Submitting Requests Without Blocking

`http_client::fetch()` is synchronous and blocks the render loop — the UI freezes while waiting for the response. For requests where you want to show a spinner, use `http_client::submit()` instead.

### How it works

1. Plugin calls `submit(req)` → gets a `request_id` string back immediately
2. Host spawns a background thread to execute the request
3. Plugin stores `request_id`, sets `loading = true`, returns a UI tree with a `{"type":"spinner"}` node
4. Host polls each frame; when the response arrives it calls `handle_event` with `kind = "http-response"` and `widget_id = request_id`
5. Plugin matches the request_id, clears `loading`, stores the response, returns updated UI

### Plugin-side pattern

```rust
// In handle_event, "click" on the send button:
"send" => {
    let req = build_request(&st);
    let request_id = http_client::submit(&req);
    st.pending_request_id = Some(request_id);
    st.loading = true;
    st.response = None;
}

// Route http-response events before normal widget dispatch:
fn handle_event(event: UiEvent) -> Result<UiOutput, PluginError> {
    if event.kind == "http-response" {
        handle_http_response(&mut st, &event);
        return Ok(ui_out(build_ui(&st)));
    }
    // ... normal event dispatch
}

// Build UI with spinner while loading:
fn build_ui(st: &State) -> Value {
    if st.loading {
        return json!({
            "type": "column", "gap": 12,
            "children": [
                {"type": "spinner"},
                {"type": "text", "value": "Sending request…", "muted": true}
            ]
        });
    }
    // ... normal UI
}

// Handle the response event:
fn handle_http_response(st: &mut State, event: &UiEvent) {
    if st.pending_request_id.as_deref() != Some(&event.widget_id) { return; }
    st.loading = false;
    st.pending_request_id = None;

    let parsed: serde_json::Value = serde_json::from_str(&event.value).unwrap_or_default();
    if let Some(ok) = parsed.get("ok") {
        let body = ok["body"].as_str().unwrap_or("").to_string();
        st.response = Some(Ok(body));
    } else if let Some(err) = parsed.get("err") {
        let msg = err["message"].as_str().unwrap_or("unknown error").to_string();
        st.response = Some(Err(msg));
    }
}
```

The host automatically calls `ctx.request_repaint()` while any `submit()` request is in flight, so the spinner animates without any extra work.

---

## Building Plugin UI with the SDK (recommended)

Plugins describe their UI as a tree of nodes serialized to JSON (the `node-json`
string returned by `render-ui` / `render-record` / `render-settings` /
`render-pane` / `handle-event`). Rather than hand-writing that JSON, depend on
the **`thoth-plugin-sdk`** crate and build the tree with type-safe builders. The
host renders the *same* `RenderNode` types, so the SDK is a single source of
truth for both sides — plugin authors get autocomplete, compile-time checking,
and no field-name typos.

### Add the dependency

```toml
[dependencies]
# Plugins only *describe* UI; they never render, so no egui feature.
thoth-plugin-sdk = { path = "../../thoth-plugin-sdk", features = ["plugin"] }
```

Cargo features:
- **default** — the DSL component types + `bon` builders (all a wasm plugin needs).
- **`plugin`** — adds the `ToNodeJson` wire-protocol trait (`node.to_json()`) and the `PluginMeta` derive.
- **`egui`** — the host-only renderer; **do not** enable it in plugins.

One glob brings in everything a plugin author needs (components + builders,
`RenderNode`, `PluginState`, `SettingsMap`, and — with the `plugin` feature —
`PluginMeta` and `ToNodeJson`):

```rust
use thoth_plugin_sdk::prelude::*;
```

### Build a tree and serialize it

```rust
use thoth_plugin_sdk::components::{Button, ButtonColor, Column, Input, Typography};
use thoth_plugin_sdk::render_node::RenderNode;

fn build_ui(state: &State) -> RenderNode {
    RenderNode::Column(
        Column::builder()
            .gap(8.0)
            .children(vec![
                RenderNode::Text(Typography::builder().text("Endpoint").build()),
                RenderNode::Input(
                    Input::builder().id("url").value(state.url.clone()).grow(true).build(),
                ),
                RenderNode::Button(
                    Button::builder().id("send").label("Send").color(ButtonColor::Primary).build(),
                ),
            ])
            .build(),
    )
}

// In render-ui / handle-event:
fn ui_out(node: RenderNode) -> UiOutput {
    UiOutput { node_json: serde_json::to_string(&node).unwrap_or_default(), height_hint: 0 }
}
```

`RenderNode` is an internally-tagged enum (`{"type":"button", ...}`), so a node
serializes to the same JSON the host deserializes. Containers (`Row`, `Column`,
`Split`, `Scroll`, `Tabs`, `Modal`, …) hold `children: Vec<RenderNode>`; leaf
widgets wrap a component struct (`RenderNode::Button(Button)`).

### Interaction & events

Interactive widgets carry an `id`. When the user interacts, the host renders the
tree, collects events, and calls your `handle-event` with a `ui-event`
(`widget-id`, `kind`, `value`):

| Widget | `kind` | `value` |
|---|---|---|
| `Button`, `IconButton`, list item, `DataRow` body, `Modal` close | `click` | empty (list item: index) |
| `Input`, `Select`, `Checkbox`, `Slider`, `NumberInput`, `Radio`, `Toggle`, `CodeEditor` | `change` | the new value |
| `MultiSelect`, `KeyValueList` | `change` | JSON (array / entries) |
| `ButtonGroups`, `Tabs` | `change` | selected index / header label |
| `DataRow` caret | `toggle` | empty |
| list action button | `action` | JSON `{"item":i,"action":j}` |

Your `handle-event` matches on `widget_id`, mutates state, and re-renders the
full tree (the standard immediate-mode flow):

```rust
fn handle_event(event: UiEvent) -> Result<UiOutput, PluginError> {
    match event.widget_id.as_str() {
        "url"  => state.url = event.value.clone(),
        "send" => { /* submit */ }
        _ => {}
    }
    Ok(ui_out(build_ui(&state)))
}
```

### Worked examples

The bundled plugins are built entirely with the SDK and are the best reference:
- **`plugins/csv-loader`** — minimal (`render-settings` only).
- **`plugins/url-source`** — a full request/response UI: rows with layout props,
  tabs with actions, modals (`open`/`close-id`/`width-pct`), inputs, key-value
  lists, a code editor, badges, and a JSON-tree response view.
- **`plugins/seshat`** — a Postgres browser: a tabbed sidebar, a schema tree of
  `DataRow`s, a SQL `CodeEditor`, a results table, and connection modals.

The component builders and a live gallery of every widget are in the
`thoth-plugin-sdk` crate (`cargo run -p thoth-plugin-sdk --example gallery
--features egui`).

### Plugin state (`thoth_plugin_sdk::state`)

WIT exports are free functions with no `self`, so plugins keep their runtime
state in a global. Instead of hand-rolling `thread_local! { static STATE:
RefCell<Option<T>> }` and repeating the borrow dance, use `PluginState<T>` — a
lazily-initialised cell usable directly as a `static`:

```rust
use thoth_plugin_sdk::state::PluginState;

#[derive(Default)]
struct State { url: String }

static STATE: PluginState<State> = PluginState::new();

// read (auto-inits from Default) / mutate:
let url = STATE.with(|s| s.url.clone());
STATE.with_mut(|s| s.url = new_url);

// when "absent" is meaningful (e.g. a file that hasn't been opened):
STATE.set(State { url });          // store explicitly
STATE.try_with(|s| s.url.clone()); // -> Option, None if unset
STATE.reset();                     // drop it (e.g. from on-close)
```

`with`/`with_mut`/`get` require `T: Default`; `set`/`reset`/`try_with`/
`is_initialised` do not. Don't call `with`/`with_mut` re-entrantly (they borrow
internally, like `RefCell`). All three bundled plugins use this.

### Settings (`thoth_plugin_sdk::settings`)

The host passes settings to `on-load` / `on-setting-change` as a JSON array of
`{key, value}` records. `SettingsMap` reads and builds that payload:

```rust
use thoth_plugin_sdk::settings::SettingsMap;

fn on_load(setting: String) {
    let s = SettingsMap::from_json(&setting);
    let url    = s.get("url").unwrap_or_default();
    let method = s.get_or("method", "GET");
    // ...
}

// build a payload back out
let json = SettingsMap::new().with("url", &url).with("method", &method).to_json();
```

### Plugin metadata (`#[derive(PluginMeta)]`)

Every plugin must implement `plugin-meta`'s `get_info()` — pure boilerplate. The
`PluginMeta` derive generates it from a declarative attribute:

```rust
use thoth_plugin_sdk::PluginMeta;

#[derive(PluginMeta)]
#[plugin(
    id          = "com.example.my-plugin",
    name        = "My Plugin",
    version     = env!("CARGO_PKG_VERSION"),
    description = "Does useful things",
    capabilities = [DataSource, NewUiComponent],
    author      = "Me",        // optional
    icon        = "\u{E28C}",  // optional (homepage also optional)
)]
struct MyPlugin;
```

`id`/`name`/`version`/`description` are required; `author`/`homepage`/`icon` are
optional. Values are any `ToString` expression (string literal, `env!(...)`, or a
glyph const). `capabilities` lists the binding `Capability` variant names
(`FileLoader`, `FileViewer`, `DataSource`, `Exporter`, `SearchProvider`,
`NewUiComponent`). The derive expands to the `get_info()` impl against your
`cargo component` bindings, so it expects the conventional top-level
`mod bindings;`. (Lifecycle hooks — `on-load`/`on-close`/`on-setting-change` —
are still written by hand; only the static metadata is derived.)

## UiNode DSL Reference

> The tables below document the on-the-wire JSON shape. With the SDK
> (recommended, see above) you build these via component builders rather than
> writing JSON by hand — the field names map 1:1 to the builder methods.

Used by `ui-component` (via `render-ui` / `handle-event`) and `plugin-settings` (via `render-settings`). Every node is a JSON object with a mandatory `"type"` field.

### Layout

| Node | Fields | Notes |
|---|---|---|
| `row` | `children`, `gap?: number`, `align?: "start\|center\|end\|fill"`, `padding?: number`, `max-width?: bool` | Horizontal |
| `column` | `children`, `gap?: number`, `padding?: number`, `bg-color?: string` | Vertical |
| `split` | `children` (exactly 2), `gap?: number`, `separator?: bool` | Equal-width columns; `separator: true` draws a 1px divider |
| `group` | `label: string`, `children` | Labelled section box |
| `scroll` | `child`, `height?: number` | Scrollable area |
| `footer` | `children`, `gap?: number`, `padding?: number` | Sticky bottom panel (rendered before content) |
| `modal` | `id`, `title`, `open: bool`, `children`, `close-id?: string`, `width-pct?: number`, `height-pct?: number` | Overlay dialog with backdrop; `close-id` emits a click event when × is pressed |
| `tabs` | `id`, `header: string[]`, `children` | Tabbed container; one child per header entry |
| `spacer` | `height?: number` | Empty vertical space |
| `separator` | — | Horizontal rule |

### Display (no interaction)

| Node | Fields |
|---|---|
| `text` | `value: string`, `size?: "sm\|md\|lg"`, `muted?: bool` |
| `heading` | `value: string`, `level?: 1–4` |
| `badge` | `label: string`, `color?: "#rrggbb"` |
| `code` | `value: string`, `language?: string` |
| `markdown` | `value: string` |
| `progress` | `value: number` (0.0–1.0) |
| `spinner` | `size?: number` | Animated loading indicator |
| `json-tree` | `value: any` | Virtualised interactive JSON tree |

### Inputs (all fire `"change"` event with JSON-encoded new value)

All inputs take `id: string`, `label: string`, `disabled?: bool`.

| Node | Extra fields | Event value |
|---|---|---|
| `text-input` | `value`, `placeholder`, `required`, `grow?: bool`, `multiline?: bool`, `rows?: number` | JSON string |
| `number-input` | `value`, `min`, `max` | JSON number |
| `password-input` | `value` | JSON string |
| `textarea` | `value`, `rows` | JSON string |
| `select` | `value`, `options: [{value,label}]` | JSON string |
| `multi-select` | `value: string[]`, `options` | JSON array |
| `checkbox` | `checked: bool` | JSON bool |
| `toggle` | `checked: bool` | JSON bool |
| `radio` | `value`, `options` | JSON string |
| `slider` | `value`, `min`, `max` | JSON number |
| `key-value-list` | `entries: [{key,value}]`, `add-label` | JSON array of `{key,value}` |
| `button-group` | `id`, `options: [{value,label}]`, `value: string` | JSON string (selected value) |

### Actions (fire `"click"` event with empty value)

| Node | Fields | Notes |
|---|---|---|
| `button` | `id`, `props: {label, button-type, color, enabled}`, `copy?: string` | `copy` writes text to clipboard on the host side without a plugin round-trip |
| `icon-button` | `id`, `icon: string`, `tooltip`, `enabled?: bool`, `frame?: bool` | Uses Phosphor icon glyphs |

### List items

`list` nodes support a `badge` field on each item for coloured pill labels (e.g. HTTP method badges):

```json
{
  "type": "list",
  "id": "saved-requests",
  "items": [
    {
      "title": "My Request",
      "description": "https://api.example.com",
      "badge": { "text": "GET", "color": "blue" },
      "actions": [{ "icon": "x", "tooltip": "Delete" }]
    }
  ]
}
```

Badge colour values: `"blue"`, `"green"`, `"red"`, `"orange"`, `"purple"`, `"gray"`.

> **Note:** The `actions` array is parsed by the host but action buttons are not yet rendered. Declaring them now is forward-compatible — they will become hover-revealed icon buttons in a future release.

---

## RenderNode Schema Reference

Used by `file-viewer` (`render-record`) and `data-source` (`render-pane`). WIT does not support recursive types, so the entire tree is a JSON string.

| Node type | JSON fields | Notes |
|---|---|---|
| `text` | `value: string` | Plain label |
| `bold` | `child: RenderNode` | Bold child |
| `italic` | `child: RenderNode` | Italic child |
| `colored` | `color: "#rrggbb"`, `child: RenderNode` | Hex-colored child |
| `badge` | `label: string`, `color: "#rrggbb"` | Filled pill badge |
| `link` | `label: string`, `url: string` | Clickable hyperlink |
| `row` | `children: RenderNode[]` | Horizontal layout |
| `column` | `children: RenderNode[]`, `gap?: number` | Vertical layout |
| `key-value` | `key: string`, `value: RenderNode` | `key: value` pair |
| `collapsible` | `label: string`, `children: RenderNode[]` | Collapsing section |
| `table` | `headers: string[]`, `rows: RenderNode[][]` | Inline table with header row |
| `json-tree` | `value: any` | Interactive JSON tree |
| `spinner` | — | Animated loading indicator |
| `spacer` | `height?: number` | Empty vertical space |
| `heading` | `value: string`, `level?: 1–4` | Section heading |

### Example

```json
{
  "type": "column",
  "children": [
    { "type": "heading", "value": "Response", "level": 3 },
    {
      "type": "row",
      "children": [
        { "type": "text", "value": "Status:" },
        { "type": "badge", "label": "200 OK", "color": "#10b981" }
      ]
    },
    { "type": "json-tree", "value": { "id": 1, "name": "Alice" } }
  ]
}
```

---

## Implementing a Theme Plugin

Theme plugins change Thoth's colour scheme without WASM. A theme is a directory containing `plugin.toml` + `theme.json`.

### `plugin.toml`

`family` groups related variants (shown as a section header in the theme picker). `catalog` is a list of `[variant-name, is-dark]` pairs — one entry per variant in `theme.json`.

```toml
id           = "com.example.gruvbox"
name         = "Gruvbox"
version      = "1.0.0"
description  = "Gruvbox colour schemes"
capabilities = ["theme"]

[theme]
family  = "Gruvbox"
catalog = [
  ["Gruvbox Dark",  true],
  ["Gruvbox Light", false],
]
```

### `theme.json` — colour token map

The file is a JSON object keyed by the variant names listed in `catalog`. Each variant must include all tokens below.

```json
{
  "Gruvbox Dark": {
    "name":               "Gruvbox Dark",
    "dark_mode":          true,
    "bg":                 "#282828",
    "bg_panel":           "#1d2021",
    "bg_sunken":          "#191b1c",
    "surface":            "#3c3836",
    "surface_raised":     "#504945",
    "surface_active":     "#665c54",
    "fg":                 "#ebdbb2",
    "fg_muted":           "#928374",
    "syntax_key":         "#83a598",
    "syntax_string":      "#b8bb26",
    "syntax_number":      "#d79921",
    "syntax_bool":        "#8ec07c",
    "syntax_punctuation": "#ebdbb2",
    "success":            "#98971a",
    "warning":            "#d65d0e",
    "error":              "#cc241d",
    "info":               "#689d6a",
    "accent":             "#d79921",
    "accent_secondary":   "#458588",
    "sidebar_hover":      "#3c383680",
    "sidebar_header":     "#ebdbb2",
    "indent_guide":       "#504945"
  },
  "Gruvbox Light": {
    "name":               "Gruvbox Light",
    "dark_mode":          false,
    "bg":                 "#fbf1c7",
    "bg_panel":           "#f2e5bc",
    "bg_sunken":          "#ebdbb2",
    "surface":            "#d5c4a1",
    "surface_raised":     "#bdae93",
    "surface_active":     "#a89984",
    "fg":                 "#3c3836",
    "fg_muted":           "#7c6f64",
    "syntax_key":         "#076678",
    "syntax_string":      "#79740e",
    "syntax_number":      "#b57614",
    "syntax_bool":        "#427b58",
    "syntax_punctuation": "#3c3836",
    "success":            "#79740e",
    "warning":            "#b57614",
    "error":              "#9d0006",
    "info":               "#427b58",
    "accent":             "#b57614",
    "accent_secondary":   "#076678",
    "sidebar_hover":      "#d5c4a133",
    "sidebar_header":     "#3c3836",
    "indent_guide":       "#d5c4a1"
  }
}
```

All tokens map 1-to-1 to fields in `ThemeColors` (`src/theme.rs`). Missing tokens fall back to the built-in defaults.

A dynamic WASM variant can expose a `theme-provider` interface that returns the colour map at runtime — useful for adaptive themes or palettes generated from user preferences. See [#70](https://github.com/anitnilay20/thoth/issues/70) for the full spec.

---

## Security Model

Each plugin runs in a fully isolated Wasmtime instance:

| Protection | Mechanism |
|---|---|
| Memory isolation | Each plugin has its own WASM linear memory; cannot read host or other plugin memory |
| Filesystem access | WASI preopened-dir grants read access to the **file's parent directory only** (set at open time) |
| No direct network access | Plugins cannot open sockets directly; all HTTP goes through the host's `http-client` import |
| Network policy | Per-plugin allowlist (intersection of plugin-declared and user-approved domains), SSRF guard (all DNS answers checked, IPv4 + IPv6 private ranges), and rate limiter (minimum of plugin and user RPM caps) |
| User consent | First request to an unlisted domain triggers a consent popup; the plugin receives `{"err":{"code":"consent_pending"}}` until the user approves; the host re-dispatches the request automatically on approval |
| CPU budget | Fuel is replenished to 5,000,000,000 units before every WIT call — infinite loops are caught and surfaced as a recoverable error |
| Bundled vs user | Bundled plugins (shipped with Thoth) set `plugin.bundled = true` at scan time; the UI prevents uninstalling them |
| Download integrity | The `sha256` field in the marketplace manifest is **always** verified against the downloaded archive; installation is aborted if the checksum is absent or does not match |

---

## Integration with Core

The bridge between plugins and core lives in `src/plugin/`:

```
src/plugin/
├── mod.rs                      ← Plugin, FileLoaderMeta, Capability types; pub mod re-exports
├── manager.rs                  ← PluginManager: discovery, loading, scan_directory
├── plugin_registry.rs          ← PluginRegistry: capability_index + plugin_key maps
├── wasm_loader.rs              ← WasmFileLoader        (file-loader-plugin world)
├── wasm_file_viewer_loader.rs  ← WasmFileViewerLoader  (file-viewer-plugin world)
├── wasm_data_source.rs         ← WasmDataSourceLoader  (data-source-plugin world)
│                                    async HTTP: background thread + mpsc channel polling
├── network_policy.rs           ← Per-plugin domain allowlist + SSRF guard
├── wasm_plugin_settings.rs     ← Settings rendering + persistence bridge
└── render_node.rs              ← RenderNode enum + render_node() egui walker
```

`FileType` in `src/file/loaders/mod.rs` has a variant for each loader kind:

```rust
pub enum FileType {
    Ndjson(NdjsonFile),
    JsonArray(JsonArrayFile),
    Single(SingleValueFile),
    Plugin(WasmFileLoader),
    PluginWithViewer(WasmFileViewerLoader),
}
```

`WasmDataSourceLoader` in `src/plugin/wasm_data_source.rs` holds the complete state for an active data-source pane:

```rust
pub struct WasmDataSourceLoader {
    inner: Mutex<WasmDataSourceInner>,         // store + bindings
    consent_rx: Receiver<ConsentRequest>,       // domain consent requests
    http_rx: Receiver<(String, HttpCallResult)>,// async HTTP results
    pending_count: Arc<AtomicUsize>,            // in-flight request counter
    plugin_id: String,
}
```

`ThothApp::poll_plugin_http_results()` is called at the top of every frame to drain completed HTTP results and feed them back into the plugin as `handle_event` calls.

---

## Roadmap: Database Plugins via WASI Sockets

> **See also:** [DATABASE_PLUGINS.md](DATABASE_PLUGINS.md) for the full design
> exploration. The recommended path is a host-provided `tcp-client` import on the
> current `wasip1` target (reusing `network_policy.rs`) plus sync drivers driven from
> a per-instance owner thread — which supersedes the WASI-sockets plan below.

The current `http-client` WIT import covers REST/HTTP data sources. For native database protocols (PostgreSQL wire protocol, MySQL, Redis RESP, etc.) the plan is to expose WASI socket access to `data-source-plugin` components.

Wasmtime supports `wasi:sockets/tcp` in WASI Preview 2. When Thoth migrates to `wasm32-wasip2`, database plugins will be able to open their own TCP connections and implement the full wire protocol in pure Rust compiled to WASM — no host changes needed for new databases.

### Target database strategy

| Database | Approach | Status |
|---|---|---|
| PostgreSQL | `postgres` crate (sync, pure Rust) over WASI TCP | Feasible today |
| MySQL / MariaDB | `mysql` crate (sync, pure Rust) over WASI TCP | Feasible today |
| Redis | RESP protocol is trivial sync over WASI TCP | Feasible today |
| Elasticsearch | REST API → existing `http-client` WIT | Works today |
| ClickHouse | HTTP interface → existing `http-client` WIT | Works today |
| MongoDB | Sync OP_MSG + BSON over WASI TCP, or Atlas Data API via HTTP | Feasible |
| Cassandra | `cdrs` (sync) over WASI TCP | Feasible |
| Kafka | Kafka binary protocol over WASI TCP | Significant work |
| SQLite | Pure-Rust SQLite engine (`limbo`) compiled to WASM | Tracking `limbo` maturity |
| Oracle | OCI is a proprietary C library — no pure-Rust driver exists | Native dylib exception |

The key principle: **drivers live in the plugin, not the host**. Adding support for a new database requires writing a new plugin, never changing Thoth core. Users download only the plugins they need.
