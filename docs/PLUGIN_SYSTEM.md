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
- [Implementing a Data Source Plugin](#implementing-a-data-source-plugin)
- [Security Model](#security-model)
- [Integration with Core](#integration-with-core)
- [Step-by-Step: Adding the Plugin Runtime to Thoth](#step-by-step-adding-the-plugin-runtime-to-thoth)

---

## Why a Plugin System

Features like API/network support, database connectivity, and additional file formats (CSV, YAML, XML) were initially planned as core additions. Instead, these are implemented as **plugins** for the following reasons:

- **Core stays fast and minimal** — users only pay for what they install
- **Independent release cycles** — plugins ship on their own schedule
- **Third-party extensibility** — anyone can write a plugin without touching core
- **Clean separation of concerns** — each plugin owns its domain fully

Issues #14 (network/API support) and #35 (additional file formats) are the first candidates to land as first-party plugins once this foundation is in place.

---

## Architecture Overview

```
┌─────────────────────────────────────────────────┐
│                  ThothApp                        │
│                                                  │
│  ┌──────────────┐     ┌──────────────────────┐  │
│  │ PluginManager│────▶│  Plugin Registry      │  │
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
│  ┌──────────────┐  ┌──────────────────────────┐ │
│  │ FileLoader   │  │  DataSource              │ │
│  │ (core trait) │  │  (core trait)            │ │
│  │              │  │                          │ │
│  │  bridges to  │  │  bridges to              │ │
│  │  WASM plugins│  │  WASM plugins            │ │
│  └──────────────┘  └──────────────────────────┘ │
└─────────────────────────────────────────────────┘
```

The `PluginManager` is initialized once at app startup, scans all plugin directories, loads each `.wasm` file into its own sandboxed Wasmtime instance, and registers it against the capabilities it declares in `plugin.toml`.

Core code then talks to plugins through thin bridge types that implement the existing core traits (`FileLoader`, `DataSource`), so the rest of the app is unaware of whether it is talking to a built-in loader or a WASM plugin.

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
| **File Viewer** | `file-viewer` | Controls how records are *rendered* — custom virtual DOM tree returned to the host |
| **Data Source** | `data-source` | Connects to an external source — REST API, database, message queue |
| **Exporter** | `exporter` | Adds new export formats or destinations |

A single plugin can declare multiple capabilities. `file-viewer` always pairs with `file-loader` — use the `file-viewer-plugin` world.

---

## Plugin Storage and Discovery

Thoth scans three locations at startup, in the following order (higher scope wins on conflict):

| Scope | Path | Purpose |
|---|---|---|
| Bundled | Next to the Thoth binary / inside the app bundle | First-party plugins shipped with Thoth |
| User | `~/.config/thoth/plugins/` (Linux/macOS) `%APPDATA%\thoth\plugins\` (Windows) | Personal plugins installed by the user |
| Project | `.thoth/plugins/` relative to the opened file | Per-project plugins committed to a repo |

Discovery algorithm:

1. Walk each directory above in order.
2. For each entry that is either a `.wasm` file or a directory containing `plugin.toml` + `plugin.wasm`, attempt to load it.
3. Read `plugin.toml` to extract `id`, `name`, `version`, and `capabilities`.
4. Register the plugin under each capability it declares.
5. Skip and log a warning for any plugin that fails validation or sandbox initialization.

---

## Plugin Structure

A plugin is either:

**A single file** (simple plugins with no metadata override):
```
~/.config/thoth/plugins/my-loader.wasm
```
The plugin ID defaults to the filename stem. Metadata is read from exported WASM functions.

**A directory** (recommended for non-trivial plugins):
```
~/.config/thoth/plugins/
└── csv-loader/
    ├── plugin.toml   ← required metadata
    └── plugin.wasm   ← compiled WASM component
```

### `plugin.toml` format

```toml
id          = "com.example.csv-loader"  # Reverse-domain unique identifier
name        = "CSV Loader"
version     = "0.2.1"
description = "Load CSV and TSV files as tabular JSON records"
author      = "Your Name <you@example.com>"

capabilities = ["file-loader"]

# Capability-specific metadata
[file-loader]
supported-extensions = ["csv", "tsv"]

# WASI permissions this plugin needs (user is prompted to approve on first load)
[permissions]
# filesystem = ["read:~/.config/myapp"]  # Example; most plugins need none
```

---

## WIT Interface Definitions

The full interface is defined in `wit/thoth-plugin.wit` at the repository root. Below is the complete reference — this is the single source of truth that all language toolchains generate bindings from.

### Shared types

Every interface uses the types defined here. `plugin-error` is the standard error type across all `result<T, E>` returns.

```wit
interface types {
    enum capability {
        file-loader,
        file-viewer,
        data-source,
        exporter,
    }

    record plugin-info {
        id:           string,
        name:         string,
        version:      string,
        description:  string,
        capabilities: list<capability>,
        author:       option<string>,
        homepage:     option<string>,
    }

    record plugin-error {
        code:    u32,
        message: string,
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

Maps directly from `src/wit/mod.rs` `PluginLifeCycle` trait.

```wit
interface plugin-lifecycle {
    on-load:  func();   // called after plugin is registered
    on-close: func();   // called before unload — release held resources
}
```

### `file-loader` — capability: `file-loader`

Maps to the core `FileLoader` trait in `src/file/loaders/mod.rs`. `raw-bytes` is required by the copy-to-clipboard and exporter paths.

```wit
interface file-loader {
    use types.{plugin-error};

    supported-extensions: func() -> list<string>;
    open:      func(path: string)  -> result<u64, plugin-error>;
    get:       func(idx: u64)      -> result<string, plugin-error>;
    raw-bytes: func(idx: u64)      -> result<list<u8>, plugin-error>;
}
```

### `file-viewer` — capability: `file-viewer`

Returns a virtual render tree the host draws with egui. Because WASM cannot call egui directly and WIT does not support recursive types, the tree is serialised as a JSON string following this schema:

```
{ "type": "text",        "value": "hello" }
{ "type": "bold",        "child": <node> }
{ "type": "italic",      "child": <node> }
{ "type": "colored",     "color": "#ff6b6b", "child": <node> }
{ "type": "badge",       "label": "WARN", "color": "#fbca04" }
{ "type": "link",        "label": "docs", "url": "https://..." }
{ "type": "row",         "children": [<node>, ...] }
{ "type": "column",      "children": [<node>, ...] }
{ "type": "key-value",   "key": "id", "value": <node> }
{ "type": "collapsible", "label": "...", "children": [<node>, ...] }
```

```wit
interface file-viewer {
    use types.{plugin-error};

    record render-output {
        node-json:   string,   // JSON-encoded render tree (schema above)
        height-hint: u32,      // logical pixels; 0 = auto
    }

    render-record:  func(record-json: string) -> result<render-output, plugin-error>;
    column-headers: func() -> option<list<string>>;
}
```

### `data-source` — capability: `data-source`

`required-config` lets the host auto-generate the "Connect to source" form. `schema` populates the source explorer sidebar after connecting.

```wit
interface data-source {
    use types.{plugin-error};

    record config-entry {
        key:   string,
        value: string,
    }

    record field-schema {
        name:      string,
        type-hint: string,   // "string" | "number" | "boolean" | "object"
        nullable:  bool,
    }

    record source-schema {
        name:   string,
        fields: list<field-schema>,
    }

    required-config: func() -> list<config-entry>;
    connect:         func(config: list<config-entry>)    -> result<string, plugin-error>;
    schema:          func(handle: string)                -> result<list<source-schema>, plugin-error>;
    query:           func(handle: string, q: string)     -> result<string, plugin-error>;
    close:           func(handle: string);
}
```

### `exporter` — capability: `exporter`

`available-options` drives a dynamic export dialog. `export` returns raw file bytes the host writes to disk.

```wit
interface exporter {
    use types.{plugin-error};

    record export-option {
        key:           string,
        label:         string,
        default-value: string,
        input-type:    string,   // "text" | "bool" | "select"
        choices:       list<string>,
    }

    name:              func() -> string;
    output-extension:  func() -> string;
    available-options: func() -> list<export-option>;
    run:               func(records-json: string, options: list<tuple<string, string>>)
                           -> result<list<u8>, plugin-error>;
}
```

### Worlds — pick the one that matches your plugin

```wit
world base-plugin {
    export plugin-meta;
    export plugin-lifecycle;
}

world file-loader-plugin {
    include base-plugin;
    export file-loader;
}

world file-viewer-plugin {       // file-viewer always pairs with file-loader
    include base-plugin;
    export file-loader;
    export file-viewer;
}

world data-source-plugin {
    include base-plugin;
    export data-source;
}

world exporter-plugin {
    include base-plugin;
    export exporter;
}
```

---

## How the Runtime Works

### Startup sequence

```
ThothApp::new()
    └── PluginManager::init()
            ├── scan_directory(bundled_plugins_dir)
            ├── scan_directory(user_plugins_dir)
            └── scan_directory(project_plugins_dir)
                    └── for each plugin:
                            ├── read plugin.toml
                            ├── wasmtime::Engine::new()       ← one engine, shared
                            ├── wasmtime::Module::from_file() ← compile .wasm
                            ├── wasmtime::Linker::new()       ← wire host functions
                            ├── wasmtime::Store::new()        ← per-plugin state + fuel limit
                            ├── validate WIT exports
                            └── register in PluginRegistry
```

### Per-call flow (file-loader example)

```
LazyJsonFile::open(path)
    └── PluginManager::find_loader_for_extension("csv")
            └── WasmFileLoader::open(path)
                    └── wasmtime call: plugin.file_loader_open(path)
                            └── [inside sandbox] CSV plugin indexes the file
                                returns record count
```

```
LazyJsonFile::get(idx)
    └── WasmFileLoader::get(idx)
            └── wasmtime call: plugin.file_loader_get(idx)
                    └── [inside sandbox] plugin reads row, serializes to JSON string
                            └── core deserializes JSON → serde_json::Value
```

The `WasmFileLoader` struct implements the core `FileLoader` trait, so the viewer, search engine, and all other components are completely unaware they are talking to a plugin.

---

## Implementing a File Loader Plugin

This walkthrough creates a CSV plugin in Rust.

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

### 3. Add dependencies to `Cargo.toml`

```toml
[package]
name = "csv-loader"
version = "0.1.0"
edition = "2021"

[lib]
crate-type = ["cdylib"]

[dependencies]
csv = "1"
serde_json = "1"

[package.metadata.component]
package = "com.example:csv-loader"
```

### 4. Define the WIT (copy from Thoth's `wit/` directory)

```bash
cp -r /path/to/thoth/wit ./wit
```

### 5. Implement the plugin (`src/lib.rs`)

```rust
use std::cell::RefCell;
use csv::ReaderBuilder;
use serde_json::{Map, Value};

// State stored per plugin instance (inside the WASM sandbox)
struct State {
    records: Vec<Vec<String>>,
    headers: Vec<String>,
}

thread_local! {
    static STATE: RefCell<Option<State>> = RefCell::new(None);
}

// Generated by cargo-component from the WIT definition
wit_bindgen::generate!({
    world: "file-loader-plugin",
    exports: {
        "thoth:plugin/plugin-meta":     CsvPlugin,
        "thoth:plugin/plugin-lifecycle": CsvPlugin,
        "thoth:plugin/file-loader":     CsvPlugin,
    }
});

struct CsvPlugin;

impl exports::thoth::plugin::plugin_meta::Guest for CsvPlugin {
    fn get_info() -> exports::thoth::plugin::plugin_meta::PluginInfo {
        exports::thoth::plugin::plugin_meta::PluginInfo {
            id:           "com.example.csv-loader".to_string(),
            name:         "CSV Loader".to_string(),
            version:      "0.1.0".to_string(),
            description:  "Load CSV and TSV files as JSON records".to_string(),
            capabilities: vec![
                exports::thoth::plugin::plugin_meta::Capability::FileLoader,
            ],
            author:   Some("Your Name <you@example.com>".to_string()),
            homepage: None,
        }
    }
}

impl exports::thoth::plugin::plugin_lifecycle::Guest for CsvPlugin {
    fn on_load() {}    // nothing to initialise for this plugin
    fn on_close() {
        // Clear any state held in memory
        STATE.with(|s| *s.borrow_mut() = None);
    }
}

impl exports::thoth::plugin::file_loader::Guest for CsvPlugin {
    fn supported_extensions() -> Vec<String> {
        vec!["csv".to_string(), "tsv".to_string()]
    }

    fn open(path: String) -> Result<u64, exports::thoth::plugin::types::PluginError> {
        let err = |msg: String| exports::thoth::plugin::types::PluginError { code: 1, message: msg };
        let delimiter = if path.ends_with(".tsv") { b'\t' } else { b',' };

        let mut reader = ReaderBuilder::new()
            .delimiter(delimiter)
            .from_path(&path)
            .map_err(|e| err(e.to_string()))?;

        let headers: Vec<String> = reader
            .headers()
            .map_err(|e| err(e.to_string()))?
            .iter()
            .map(str::to_owned)
            .collect();

        let records: Vec<Vec<String>> = reader
            .records()
            .map(|r| r.map(|row| row.iter().map(str::to_owned).collect()))
            .collect::<Result<_, _>>()
            .map_err(|e| err(e.to_string()))?;

        let count = records.len() as u64;
        STATE.with(|s| *s.borrow_mut() = Some(State { records, headers }));
        Ok(count)
    }

    fn get(idx: u64) -> Result<String, exports::thoth::plugin::types::PluginError> {
        let err = |msg: &str| exports::thoth::plugin::types::PluginError {
            code: 2, message: msg.to_string(),
        };
        STATE.with(|s| {
            let state = s.borrow();
            let state = state.as_ref().ok_or(err("file not opened"))?;
            let row = state.records.get(idx as usize).ok_or(err("index out of range"))?;

            let obj: Map<String, Value> = state.headers
                .iter()
                .zip(row.iter())
                .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                .collect();

            serde_json::to_string(&Value::Object(obj))
                .map_err(|e| exports::thoth::plugin::types::PluginError { code: 3, message: e.to_string() })
        })
    }

    fn raw_bytes(idx: u64) -> Result<Vec<u8>, exports::thoth::plugin::types::PluginError> {
        // Re-use get() and return its UTF-8 bytes — sufficient for CSV
        let json = Self::get(idx)?;
        Ok(json.into_bytes())
    }
}
```

### 6. Build

```bash
cargo component build --release
# Output: target/wasm32-wasip1/release/csv_loader.wasm
```

### 7. Install

```bash
mkdir -p ~/.config/thoth/plugins/csv-loader
cp target/wasm32-wasip1/release/csv_loader.wasm ~/.config/thoth/plugins/csv-loader/plugin.wasm

cat > ~/.config/thoth/plugins/csv-loader/plugin.toml <<EOF
id           = "com.example.csv-loader"
name         = "CSV Loader"
version      = "0.1.0"
description  = "Load CSV and TSV files"
capabilities = ["file-loader"]

[file-loader]
supported-extensions = ["csv", "tsv"]
EOF
```

Restart Thoth. Opening a `.csv` file will now use your plugin automatically.

---

## Implementing a Data Source Plugin

A data source plugin connects Thoth to an external system (REST API, database, etc.) instead of loading a local file. The user triggers it from the toolbar via **"Connect to source"**.

The flow mirrors the file-loader pattern but uses `connect` → `query` → `close` instead of `open` → `get`.

```rust
impl exports::thoth::plugin::data_source::Guest for MyPlugin {
    fn required_config() -> Vec<exports::thoth::plugin::data_source::ConfigEntry> {
        // Tell the host which fields to show in the "Connect to source" form
        vec![
            ConfigEntry { key: "url".into(),   value: "https://api.example.com".into() },
            ConfigEntry { key: "token".into(), value: "".into() },
        ]
    }

    fn connect(config: Vec<ConfigEntry>) -> Result<String, PluginError> {
        let url = config.iter().find(|e| e.key == "url").map(|e| &e.value)
            .ok_or(PluginError { code: 1, message: "missing url".into() })?;
        // ... store connection state keyed by handle UUID
        Ok("handle-uuid-1234".to_string())
    }

    fn schema(handle: String) -> Result<Vec<SourceSchema>, PluginError> {
        // Return available endpoints/tables for the source explorer sidebar
        Ok(vec![
            SourceSchema {
                name: "users".into(),
                fields: vec![
                    FieldSchema { name: "id".into(),   type_hint: "number".into(), nullable: false },
                    FieldSchema { name: "name".into(), type_hint: "string".into(), nullable: false },
                ],
            },
        ])
    }

    fn query(handle: String, q: String) -> Result<String, PluginError> {
        // Execute query, return JSON array string
        // e.g. "[{\"id\":1,\"name\":\"Alice\"}, ...]"
        Ok("[]".to_string())
    }

    fn close(handle: String) {
        // Clean up connection state
    }
}
```

---

## Security Model

Each plugin runs in a fully isolated Wasmtime instance:

| Protection | Mechanism |
|---|---|
| Memory isolation | Each plugin has its own WASM linear memory; cannot read host or other plugin memory |
| No filesystem access | WASI filesystem capability is not granted by default |
| No network access | WASI socket capability is not granted by default |
| CPU budget | Wasmtime fuel limit prevents infinite loops from stalling the UI |
| Memory cap | Wasmtime `max_wasm_stack` + memory limit prevents runaway allocation |
| Permission prompts | Plugins requesting WASI capabilities (e.g. network for a data-source plugin) trigger a one-time approval dialog on first load |

Permissions a plugin has been granted are stored in `~/.config/thoth/plugin-permissions.toml`.

---

## Integration with Core

The bridge between plugins and core lives in `src/plugin/`:

```
src/plugin/
├── mod.rs           ← public re-exports
├── manager.rs       ← PluginManager: discovery, loading, registry
├── registry.rs      ← in-memory map of capability → Vec<PluginHandle>
├── wasm_loader.rs   ← WasmFileLoader: implements FileLoader trait
├── wasm_source.rs   ← WasmDataSource: implements DataSource trait
└── permissions.rs   ← WASI capability approval and persistence
```

`LazyJsonFile` (in `src/file/loaders/mod.rs`) gains a new variant:

```rust
pub enum LazyJsonFile {
    Ndjson(NdjsonFile),
    JsonArray(JsonArrayFile),
    Single(SingleValueFile),
    Plugin(WasmFileLoader),   // ← new
}
```

`load_file_auto` checks the extension against the plugin registry before falling back to the built-in detection logic:

```rust
pub fn load_file_auto(path: &Path) -> Result<(DetectedFileType, LazyJsonFile)> {
    // 1. Check plugin registry first
    if let Some(loader) = PLUGIN_MANAGER.get().and_then(|pm| pm.find_loader(path)) {
        return Ok((DetectedFileType::Plugin, LazyJsonFile::Plugin(loader)));
    }
    // 2. Fall back to built-in sniffing
    let detected = sniff_file_type(path)?;
    // ...
}
```

---

## Step-by-Step: Adding the Plugin Runtime to Thoth

This section is for core contributors wiring the system into Thoth itself.

### Step 1 — Add dependencies

`wasmtime` is already present in `Cargo.toml`. Add the component-model feature and the WASI layer:

```toml
# Cargo.toml
[dependencies]
wasmtime      = { version = "34", features = ["component-model"] }
wasmtime-wasi = "34"
wit-bindgen   = "0.41"
# toml and dirs are already present
```

### Step 2 — WIT interfaces (already done)

`wit/thoth-plugin.wit` already exists at the repository root. See the [WIT Interface Definitions](#wit-interface-definitions) section for the full reference.

### Step 3 — Implement `PluginManager`

Create `src/plugin/manager.rs`:

```rust
use std::path::PathBuf;
use wasmtime::{Engine, Store};
use wasmtime_wasi::WasiCtxBuilder;

pub struct PluginManager {
    engine: Engine,
    registry: PluginRegistry,
}

impl PluginManager {
    pub fn init() -> Self {
        let engine = Engine::default();
        let mut manager = Self { engine, registry: PluginRegistry::default() };
        manager.scan_all_directories();
        manager
    }

    fn scan_all_directories(&mut self) {
        for dir in self.plugin_directories() {
            if dir.exists() {
                self.scan_directory(&dir);
            }
        }
    }

    fn plugin_directories(&self) -> Vec<PathBuf> {
        vec![
            bundled_plugins_dir(),
            user_plugins_dir(),
            project_plugins_dir(),
        ]
    }

    fn scan_directory(&mut self, dir: &Path) {
        // Walk dir, find plugin.toml + plugin.wasm pairs
        // Call self.load_plugin() for each
    }

    fn load_plugin(&mut self, wasm_path: &Path, meta: PluginMeta) -> Result<()> {
        let wasi = WasiCtxBuilder::new().build();
        let mut store = Store::new(&self.engine, wasi);
        store.set_fuel(1_000_000)?; // CPU budget per call (refilled each call)

        let module = wasmtime::Module::from_file(&self.engine, wasm_path)?;
        // Link, instantiate, validate exports, register
        self.registry.register(meta, instance);
        Ok(())
    }
}
```

### Step 4 — Store `PluginManager` as a global

```rust
// src/plugin/mod.rs
use std::sync::OnceLock;

pub static PLUGIN_MANAGER: OnceLock<PluginManager> = OnceLock::new();

pub fn init() {
    PLUGIN_MANAGER.set(PluginManager::init()).ok();
}
```

Call `plugin::init()` early in `main.rs`, before the eframe window is created.

### Step 5 — Bridge into `FileLoader`

Create `src/plugin/wasm_loader.rs` implementing `FileLoader` for `WasmFileLoader`. Wire it into `LazyJsonFile` and `load_file_auto` as shown in [Integration with Core](#integration-with-core).

### Step 6 — Settings UI

Add a **Plugins** tab to `src/components/settings_panel.rs` that lists all loaded plugins, their version, capabilities, and an enable/disable toggle. Disabled plugins are skipped during the scan on next startup.

### Step 7 — Ship a bundled example

Add the CSV loader plugin source under `plugins/csv-loader/` and build it as part of the release CI pipeline. Bundle the resulting `.wasm` alongside the Thoth binary.
