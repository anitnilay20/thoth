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
- [RenderNode Schema Reference](#rendernode-schema-reference)
- [Security Model](#security-model)
- [Integration with Core](#integration-with-core)

---

## Why a Plugin System

Features like API/network support, database connectivity, and additional file formats (CSV, YAML, XML) were initially planned as core additions. Instead, these are implemented as **plugins** for the following reasons:

- **Core stays fast and minimal** — users only pay for what they install
- **Independent release cycles** — plugins ship on their own schedule
- **Third-party extensibility** — anyone can write a plugin without touching core
- **Clean separation of concerns** — each plugin owns its domain fully

---

## Architecture Overview

```
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
| **Exporter** | `exporter` | Adds new export formats or destinations |
| **Search Provider** | `search-provider` | Extends the search experience with custom indexing or remote results |
| **New UI Component** | `new-ui-component` | Registers additional UI panels or toolbar widgets |

A single plugin can declare multiple capabilities. `file-viewer` always pairs with `file-loader` — use the `file-viewer-plugin` world for this combination.

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

```
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

```wit
interface plugin-lifecycle {
    on-load:  func();   // called after plugin is registered
    on-close: func();   // called before unload — release held resources
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

    // How the plugin wants its data displayed.
    // table  — host renders a native table using column-headers() and the raw
    //          JSON values from file-loader.get(). render-record() is never called.
    // custom — host calls render-record() for every visible row and draws the
    //          returned RenderNode tree (see RenderNode Schema Reference below).
    enum display-mode {
        table,
        custom,
    }

    preferred-display:  func() -> display-mode;
    column-headers:     func() -> option<list<string>>;
    render-record:      func(record-json: string) -> result<render-output, plugin-error>;

    record render-output {
        node-json:   string,   // JSON-encoded RenderNode tree
        height-hint: u32,      // logical pixels; 0 = auto
    }
}
```

### `data-source` — capability: `data-source`

```wit
interface data-source {
    use types.{plugin-error};

    record config-entry   { key: string, value: string }
    record field-schema   { name: string, type-hint: string, nullable: bool }
    record source-schema  { name: string, fields: list<field-schema> }

    required-config: func() -> list<config-entry>;
    connect:         func(config: list<config-entry>)    -> result<string, plugin-error>;
    schema:          func(handle: string)                -> result<list<source-schema>, plugin-error>;
    query:           func(handle: string, q: string)     -> result<string, plugin-error>;
    close:           func(handle: string);
}
```

### `exporter` — capability: `exporter`

```wit
interface exporter {
    use types.{plugin-error};

    record export-option {
        key: string, label: string, default-value: string,
        input-type: string,    // "text" | "bool" | "select"
        choices: list<string>,
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
world base-plugin {           // every plugin satisfies this
    export plugin-meta;
    export plugin-lifecycle;
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

```
FileViewer::open(path)
    └── PluginManager::plugin_has_capability("csv", FileViewer) → true
    └── PluginManager::open_file_with_viewer("csv", path)
            └── WasmFileViewerLoader::open(engine, wasm_path, path)
                    ├── WASI: preopened dir = file's parent (read-only)
                    ├── set fuel = u64::MAX / 2
                    └── wasmtime call: file-loader.open(path) → record_count

Per frame (virtual scrolling — only visible rows):
    PluginTableViewer::render()
        ├── loader.column_headers()   → ["Name", "Age", ...]  [called once, cached]
        ├── loader.preferred_display() → Table                 [called once, cached]
        └── for each visible row idx:
                loader.get(idx)       → serde_json::Value      [cached in LRU]
                (Table mode: host renders cells natively — render_record not called)
```

### Fuel replenishment

Each WIT call replenishes fuel to `u64::MAX / 2` before calling into the plugin. This prevents fuel exhaustion across many sequential calls while still bounding any single infinite-loop.

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
crate-type = ["cdylib"]   # required for WASM component output

[dependencies]
# Must match the version cargo-component uses internally.
# Check the generated src/bindings.rs header for the exact version.
wit-bindgen-rt = "0.41"
serde_json     = "1"

[package.metadata.component]
package = "com.example:csv-loader"

[package.metadata.component.target]
path  = "../../wit"          # path to thoth-plugin.wit
world = "file-loader-plugin"
```

### 4. Copy the WIT file

```bash
cp -r /path/to/thoth/wit ./wit
```

### 5. Generate bindings

```bash
cargo component build
# cargo-component generates src/bindings.rs automatically.
# Do NOT call wit_bindgen::generate! manually — cargo-component does it.
```

### 6. Implement the plugin (`src/lib.rs`)

```rust
// cargo-component generates src/bindings.rs — import it here.
mod bindings;

use std::cell::RefCell;
use bindings::exports::thoth::plugin::{
    file_loader::Guest as FileLoaderGuest,
    plugin_lifecycle::Guest as LifecycleGuest,
    plugin_meta::Guest as MetaGuest,
};
use bindings::thoth::plugin::types::{Capability, PluginError};
use serde_json::{Map, Value};

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
        }
    }
}

impl LifecycleGuest for CsvPlugin {
    fn on_load() {}
    fn on_close() {
        STATE.with(|s| *s.borrow_mut() = None);
    }
}

impl FileLoaderGuest for CsvPlugin {
    fn supported_extensions() -> Vec<String> {
        vec!["csv".to_string(), "tsv".to_string()]
    }

    fn open(path: String) -> Result<u64, PluginError> {
        let err = |msg: &str| PluginError { code: 1, message: msg.to_string() };
        let content = std::fs::read_to_string(&path).map_err(|e| PluginError {
            code: 1, message: e.to_string(),
        })?;
        let mut lines = content.lines();
        let headers: Vec<String> = lines
            .next()
            .ok_or(err("empty file"))?
            .split(',')
            .map(str::to_owned)
            .collect();
        let records: Vec<Vec<String>> = lines
            .map(|l| l.split(',').map(str::to_owned).collect())
            .collect();
        let count = records.len() as u64;
        STATE.with(|s| *s.borrow_mut() = Some(State { headers, records }));
        Ok(count)
    }

    fn get(idx: u64) -> Result<String, PluginError> {
        STATE.with(|s| {
            let guard = s.borrow();
            let state = guard.as_ref()
                .ok_or(PluginError { code: 2, message: "file not opened".into() })?;
            let row = state.records.get(idx as usize)
                .ok_or(PluginError { code: 2, message: "index out of range".into() })?;
            let obj: Map<String, Value> = state.headers.iter()
                .zip(row.iter())
                .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                .collect();
            serde_json::to_string(&Value::Object(obj))
                .map_err(|e| PluginError { code: 3, message: e.to_string() })
        })
    }

    fn raw_bytes(idx: u64) -> Result<Vec<u8>, PluginError> {
        Self::get(idx).map(String::into_bytes)
    }
}

// Register all exports with the WASM component runtime
bindings::export!(CsvPlugin with_types_in bindings);
```

### 7. Write `plugin.toml`

```toml
id          = "com.example.csv-loader"
name        = "CSV Loader"
version     = "0.1.0"
description = "Load CSV files as JSON records"
author      = "Your Name <you@example.com>"
capabilities = ["file-loader"]

[[file-loader]]
file-type            = "text/csv"
supported-extensions = ["csv", "tsv"]
```

### 8. Build and install

```bash
cargo component build --release
# Output: target/wasm32-wasip1/release/csv_loader.wasm

mkdir -p ~/.config/thoth/plugins/csv-loader
cp target/wasm32-wasip1/release/csv_loader.wasm ~/.config/thoth/plugins/csv-loader/plugin.wasm
cp plugin.toml ~/.config/thoth/plugins/csv-loader/plugin.toml
# optionally: cp icon.png ~/.config/thoth/plugins/csv-loader/icon.png
```

Restart Thoth — opening a `.csv` file will now use your plugin.

---

## Implementing a File Viewer Plugin

A file viewer plugin controls *how* records are displayed in the viewer panel. It always pairs with `file-loader` — use the `file-viewer-plugin` world.

The plugin declares its preferred rendering mode in `preferred-display()`:

- **`table`** — The host renders a native, horizontally-scrollable table. The plugin only needs to return column headers; the host reads cell values directly from `file-loader.get()`. `render-record()` is never called.
- **`custom`** — The host calls `render-record()` for each visible row and draws the returned `RenderNode` JSON tree. Use this for badges, colours, links, nested tables, or any cell content that goes beyond plain text.

### `Cargo.toml` changes

Change the world to `file-viewer-plugin`:

```toml
[package.metadata.component.target]
path  = "../../wit"
world = "file-viewer-plugin"
```

### Additional imports

```rust
use bindings::exports::thoth::plugin::{
    file_viewer::{DisplayMode, Guest as FileViewerGuest, RenderOutput},
    // ... rest of imports
};
```

### Implement `FileViewerGuest`

**Table mode** (simplest — host renders the table natively):

```rust
impl FileViewerGuest for MyPlugin {
    fn preferred_display() -> DisplayMode {
        DisplayMode::Table
    }

    fn column_headers() -> Option<Vec<String>> {
        // Return None to use the JSON keys from the first record as headers.
        // Return Some(...) to use custom header labels.
        STATE.with(|s| s.borrow().as_ref().map(|st| st.headers.clone()))
    }

    fn render_record(_record_json: String) -> Result<RenderOutput, PluginError> {
        // Not called in table mode.
        Err(PluginError { code: 0, message: "not used in table mode".into() })
    }
}
```

**Custom mode** (full control via RenderNode tree):

```rust
impl FileViewerGuest for MyPlugin {
    fn preferred_display() -> DisplayMode {
        DisplayMode::Custom
    }

    fn column_headers() -> Option<Vec<String>> {
        None  // not used in custom mode
    }

    fn render_record(record_json: String) -> Result<RenderOutput, PluginError> {
        let map: serde_json::Map<String, serde_json::Value> =
            serde_json::from_str(&record_json).map_err(|e| PluginError {
                code: 1, message: e.to_string(),
            })?;

        // Build a row of colored/plain cells
        let children: Vec<serde_json::Value> = map.iter().map(|(k, v)| {
            let text = v.as_str().unwrap_or("").to_string();
            let is_error = text.contains("ERROR");
            if is_error {
                serde_json::json!({
                    "type": "colored",
                    "color": "#ff6b6b",
                    "child": { "type": "text", "value": text }
                })
            } else {
                serde_json::json!({ "type": "text", "value": text })
            }
        }).collect();

        let node_json = serde_json::json!({
            "type": "row",
            "children": children
        }).to_string();

        Ok(RenderOutput { node_json, height_hint: 0 })
    }
}
```

### Update `plugin.toml`

```toml
capabilities = ["file-loader", "file-viewer"]
```

### Build

```bash
cargo component build --release
```

Copy the rebuilt `.wasm` to your plugin directory and restart Thoth.

---

## Implementing a Data Source Plugin

A data source plugin connects Thoth to an external system (REST API, database, etc.). The user triggers it from the toolbar via **"Connect to source"**.

The flow is `connect` → `query` → `close` instead of `open` → `get`.

```rust
impl FileViewerGuest for MyPlugin {
    fn required_config() -> Vec<ConfigEntry> {
        vec![
            ConfigEntry { key: "url".into(),   value: "https://api.example.com".into() },
            ConfigEntry { key: "token".into(), value: "".into() },
        ]
    }

    fn connect(config: Vec<ConfigEntry>) -> Result<String, PluginError> {
        let url = config.iter().find(|e| e.key == "url")
            .ok_or(PluginError { code: 1, message: "missing url".into() })?;
        // store connection state, return an opaque handle
        Ok("handle-uuid-1234".to_string())
    }

    fn schema(handle: String) -> Result<Vec<SourceSchema>, PluginError> {
        Ok(vec![SourceSchema {
            name: "users".into(),
            fields: vec![
                FieldSchema { name: "id".into(),   type_hint: "number".into(), nullable: false },
                FieldSchema { name: "name".into(), type_hint: "string".into(), nullable: false },
            ],
        }])
    }

    fn query(handle: String, q: String) -> Result<String, PluginError> {
        // Execute and return a JSON array string
        Ok("[]".to_string())
    }

    fn close(handle: String) {}
}
```

---

## RenderNode Schema Reference

When `preferred-display` is `custom`, `render-record` must return a JSON string following this schema. The host deserialises it into a `RenderNode` enum and draws it with egui.

WIT does not support recursive types, so the entire tree is encoded as a single JSON string in `render-output.node-json`.

| Node type | JSON fields | Notes |
|---|---|---|
| `text` | `value: string` | Plain label |
| `bold` | `child: RenderNode` | Renders child text in bold |
| `italic` | `child: RenderNode` | Renders child text in italics |
| `colored` | `color: "#rrggbb"`, `child: RenderNode` | Applies hex color to child |
| `badge` | `label: string`, `color: "#rrggbb"` | Filled pill badge |
| `link` | `label: string`, `url: string` | Clickable hyperlink |
| `row` | `children: RenderNode[]` | Horizontal layout |
| `column` | `children: RenderNode[]` | Vertical layout |
| `key-value` | `key: string`, `value: RenderNode` | `key: value` pair |
| `collapsible` | `label: string`, `children: RenderNode[]` | Collapsing section |
| `table` | `headers: string[]`, `rows: RenderNode[][]` | Inline table with header row |
| `json-tree` | `value: any` | Renders an arbitrary JSON value as an interactive tree |

### Example

```json
{
  "type": "row",
  "children": [
    { "type": "text", "value": "Status:" },
    {
      "type": "colored",
      "color": "#ff6b6b",
      "child": { "type": "text", "value": "ERROR" }
    },
    {
      "type": "badge",
      "label": "WARN",
      "color": "#fbca04"
    }
  ]
}
```

---

## Security Model

Each plugin runs in a fully isolated Wasmtime instance:

| Protection | Mechanism |
|---|---|
| Memory isolation | Each plugin has its own WASM linear memory; cannot read host or other plugin memory |
| Filesystem access | WASI preopened-dir grants read access to the **file's parent directory only** (set at open time) |
| No network access | WASI socket capability is not granted by default |
| CPU budget | Fuel is replenished to `u64::MAX / 2` before every WIT call — infinite loops cannot stall the UI indefinitely |
| Bundled vs user | Bundled plugins (shipped with Thoth) set `plugin.bundled = true` at scan time; the UI prevents uninstalling them |

---

## Integration with Core

The bridge between plugins and core lives in `src/plugin/`:

```
src/plugin/
├── mod.rs                    ← Plugin, FileLoaderMeta, Capability types; pub mod re-exports
├── manager.rs                ← PluginManager: discovery, loading, scan_directory
├── plugin_registry.rs        ← PluginRegistry: capability_index + plugin_key maps
├── wasm_loader.rs            ← WasmFileLoader  (file-loader-plugin world)
├── wasm_file_viewer_loader.rs← WasmFileViewerLoader (file-viewer-plugin world)
└── render_node.rs            ← RenderNode enum + render_node() egui walker
```

`FileType` in `src/file/loaders/mod.rs` has a variant for each loader kind:

```rust
pub enum FileType {
    Ndjson(NdjsonFile),
    JsonArray(JsonArrayFile),
    Single(SingleValueFile),
    Plugin(WasmFileLoader),           // file-loader only
    PluginWithViewer(WasmFileViewerLoader), // file-loader + file-viewer
}
```

`FileViewer::open()` in `src/components/file_viewer/mod.rs` checks capability at open time:

```rust
let has_viewer = pm.plugin_has_capability(ext_str, &Capability::FileViewer);

let (loader, kind) = if has_viewer {
    let wfl = pm.open_file_with_viewer(ext_str, path)?;
    (FileType::PluginWithViewer(wfl), FileKind::PluginTable)
} else {
    let wfl = pm.open_file(ext_str, path)?;
    (FileType::Plugin(wfl), FileKind::Plugin)
};
```

The viewer-type selection mirrors this in `src/components/file_viewer/viewer_type.rs`:

```rust
pub enum ViewerType {
    Json(JsonTreeViewer),
    PluginTable(PluginTableViewer),
}
```

`PluginTableViewer` (`src/components/file_viewer/plugin_table_viewer.rs`) uses virtual scrolling via `egui_extras::TableBuilder` — only visible rows trigger WASM calls. Rendered rows are cached in a `HashMap<usize, String>` (node JSON) so each row is only sent to the plugin once.
