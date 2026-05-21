# RFC: MCP Server Integration for Thoth

> **Status:** Draft
> **Author:** Shubham Raizada
> **Created:** 2026-05-20
> **Last Updated:** 2026-05-21

## Table of Contents

- [Summary](#summary)
- [Motivation](#motivation)
- [Design Philosophy](#design-philosophy)
- [Architecture](#architecture)
- [Technical Specification](#technical-specification)
- [Tool Definitions](#tool-definitions)
- [Implementation Plan](#implementation-plan)
- [Security Considerations](#security-considerations)
- [Design Decisions](#design-decisions)
- [Alternatives Considered](#alternatives-considered)

---

## Summary

This RFC proposes adding a **Model Context Protocol (MCP) server** to Thoth, enabling AI assistants (Claude Code, Claude Desktop, Cursor, etc.) to programmatically interact with JSON and NDJSON files through Thoth's high-performance file loading, search, and query engines.

The server is exposed as a subcommand (`thoth mcp serve`) that starts a headless process communicating over the **stdio transport** (JSON-RPC 2.0 over stdin/stdout). It reuses Thoth's existing core library (file loaders, SIMD-accelerated search, JSONPath engine, LRU cache) without any GUI dependency.

---

## Motivation

### The Problem

AI coding assistants are increasingly used to analyze data files - log files, API responses, configuration dumps, test fixtures. Today, when an AI agent needs to explore a large JSON or NDJSON file, it has limited options:

1. **Read the raw file** - Works for small files, but a 500MB NDJSON log is too large for any context window.
2. **Use generic shell tools** (`jq`, `grep`) - Requires the AI to compose complex pipelines and parse raw output.
3. **Write throwaway scripts** - Slow iteration, no caching, no incremental exploration.

### The Opportunity

Thoth already solves these problems for human users - lazy loading, indexed access, SIMD search, JSONPath queries, LRU caching. An MCP server exposes these same capabilities to AI agents, making Thoth the **bridge between large structured data files and AI assistants**.

### Use Cases

| Scenario | How the AI Uses Thoth |
|----------|----------------------|
| **Log analysis** | "Open this 200MB NDJSON log and find all records where `status >= 500`" |
| **Schema discovery** | "What's the structure of this API response file? What keys exist?" |
| **Data quality** | "Are there records with missing required fields or null values?" |
| **Data extraction** | "Get me all unique user IDs from this dataset" |
| **Debugging** | "Compare records 42 and 1337 - what changed between them?" |
| **Test fixtures** | "Open this test fixture and give me the third element" |

---

## Design Philosophy

1. **Subcommand, not separate binary** - `thoth mcp serve` keeps distribution simple (one binary), versioning in sync, and follows the pattern established by Claude Code itself (`claude mcp serve`).

2. **Reuse, don't rewrite** - Every MCP tool maps directly to existing Thoth library code. The MCP layer is a thin adapter over `src/file/`, `src/search/`, and `src/helpers/`.

3. **stdout is sacred** - In the MCP code path, stdout is exclusively reserved for JSON-RPC protocol messages. All logging goes to stderr. No GUI code is initialized.

4. **Lazy by default** - Files are opened and indexed but not fully parsed into memory. Records are parsed on demand, matching Thoth's core lazy-loading architecture.

5. **Multiple files, single server** - The server can hold multiple files open simultaneously. Each file is identified by a handle returned from `open_file`.

---

## Architecture

### High-Level Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        thoth (single binary)                    │
│                                                                 │
│  main()                                                         │
│    ├── args == "mcp serve"?                                     │
│    │     YES ──► run_mcp_server()          [headless, no GUI]   │
│    │              ├── tracing → stderr                          │
│    │              ├── tokio runtime                              │
│    │              ├── ThothMcpServer                             │
│    │              └── stdio transport (stdin/stdout JSON-RPC)   │
│    │                                                            │
│    │     NO ───► existing GUI startup      [unchanged]          │
│    │              ├── eframe::run_native()                      │
│    │              └── egui event loop                            │
│    │                                                            │
│  ┌─────────────────────────────────────────────────────────┐    │
│  │              Shared Library (src/lib.rs)                 │    │
│  │                                                         │    │
│  │  file/         search/         helpers/                  │    │
│  │  ├── loaders/  ├── engine.rs   ├── lru_cache.rs         │    │
│  │  │  ├── ndjson │  (SIMD text)  ├── json_copy_to_        │    │
│  │  │  ├── json_  ├── jsonpath.rs │    clipboard.rs        │    │
│  │  │  │  array   │  (JSONPath)   │   (path walking)       │    │
│  │  │  ├── single ├── results.rs  └── format.rs            │    │
│  │  │  └── wasm   │  (SearchHit)                           │    │
│  │  └── detect_   └───────────────                         │    │
│  │     file_type.rs                                        │    │
│  └─────────────────────────────────────────────────────────┘    │
└─────────────────────────────────────────────────────────────────┘
```

### Communication Flow

```
AI Client (Claude Code, Cursor, etc.)
    │
    ├── spawns: thoth mcp serve
    │
    ├── stdin  ──────────►  ThothMcpServer reads JSON-RPC requests
    │                         ├── tools/call → dispatch to tool handler
    │                         ├── tool handler calls into src/file/, src/search/
    │                         └── returns result
    │
    └── stdout ◄──────────  ThothMcpServer writes JSON-RPC responses
    
    stderr ────────────►  (logging only, client ignores)
```

### Module Structure

```
src/
  mcp/                        # NEW - MCP server module
    mod.rs                    # ThothMcpServer struct, #[tool_router] impl
    state.rs                  # ServerState - loaded files, caches
    tools/
      mod.rs                  # Tool registry and shared utilities
      file_ops.rs             # open_file, get_record, get_record_count, get_file_info
      search.rs               # search (text + JSONPath, auto-detected)
      data.rs                 # get_value_at_path, extract_keys, sample_records, get_schema
      advanced.rs             # filter_records, aggregate, compare_records
    resources.rs              # MCP resource providers
    prompts.rs                # MCP prompt templates

  main.rs                     # MODIFIED - add mcp subcommand branch
  lib.rs                      # MODIFIED - add `pub mod mcp;`
```

### What the MCP Module Does NOT Touch

The MCP server has **zero dependency** on GUI code. It never imports from:
- `src/app/` (ThothApp, egui application logic)
- `src/components/` (UI components, egui widgets)
- `src/theme/` (visual theming)
- `src/notification/` (toast notifications)
- `src/consent/` (GUI permission dialogs)
- `src/update/` (self-update system)

---

## Technical Specification

### New Dependencies

```toml
[dependencies]
rmcp = { version = "0.16", features = ["server", "transport-io"] }
schemars = "1.0"  # JSON Schema derivation for tool parameter types
```

Both `tokio` and `serde_json` are already in the dependency tree. The `rmcp` crate is the official Rust SDK for the Model Context Protocol.

### Entry Point

The MCP subcommand is handled at the top of `main()`, **before** any GUI initialization:

```rust
fn main() -> Result<()> {
    let args: Vec<String> = std::env::args().collect();

    // MCP subcommand - must be checked before any GUI init
    if args.get(1).map(|s| s.as_str()) == Some("mcp") {
        match args.get(2).map(|s| s.as_str()) {
            Some("serve") => return run_mcp_server(),
            Some("list-tools") => return list_mcp_tools(),
            _ => {
                eprintln!("Usage: thoth mcp <serve|list-tools>");
                std::process::exit(1);
            }
        }
    }

    // ... existing GUI startup (unchanged) ...
}
```

This branching is critical. By intercepting before `eframe::run_native()`, we guarantee:
- No egui initialization (no window, no display server required)
- No plugin manager startup
- No notification/consent manager globals
- stdout remains clean for JSON-RPC

### Server State

```rust
/// Holds all state for the MCP server across tool invocations.
pub struct ServerState {
    /// Open files, keyed by handle (auto-incrementing ID).
    files: HashMap<String, OpenFile>,
    /// Counter for generating unique file handles.
    next_handle: u64,
}

/// A file that has been opened and is ready for querying.
pub struct OpenFile {
    pub path: PathBuf,
    pub detected_type: DetectedFileType,
    pub file_type: FileType,
    pub opened_at: Instant,
}
```

Files are opened via the `open_file` tool and assigned a string handle (e.g., `"file_1"`). Subsequent tools reference files by handle. This allows multiple files to be open simultaneously.

### Server Handler

The server uses `rmcp`'s `#[tool_router]` macro to declaratively define tools:

```rust
use rmcp::{ServerHandler, tool, tool_router, model::*};

#[derive(Clone)]
pub struct ThothMcpServer {
    state: Arc<Mutex<ServerState>>,
}

#[tool_router]
impl ThothMcpServer {
    #[tool(description = "Open a JSON, NDJSON, or GeoJSON file for querying")]
    async fn open_file(
        &self,
        #[tool(param, description = "Absolute or relative path to the file")]
        path: String,
    ) -> Result<CallToolResult, McpError> {
        // sniff_file_type() → load_file_auto() → store in state → return handle
    }

    #[tool(description = "Get a record by index from an open file")]
    async fn get_record(
        &self,
        #[tool(param, description = "File handle returned by open_file")]
        file: String,
        #[tool(param, description = "Zero-based record index")]
        index: usize,
    ) -> Result<CallToolResult, McpError> {
        // state.files[handle].file_type.get(index) → serialize to JSON
    }

    // ... additional tools ...
}

impl ServerHandler for ThothMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            name: "thoth".into(),
            version: env!("CARGO_PKG_VERSION").into(),
            ..Default::default()
        }
    }
}
```

### Error Handling

MCP tools return errors as `CallToolResult` with `is_error: true`, not by failing the JSON-RPC call. This follows MCP conventions - the protocol call succeeds, but the tool reports an application-level error:

```rust
// File not found → tool error (not protocol error)
CallToolResult::error(vec![Content::text("File not found: /path/to/missing.json")])

// Invalid handle → tool error
CallToolResult::error(vec![Content::text("No file open with handle 'file_99'")])

// Success
CallToolResult::success(vec![Content::text(serde_json::to_string_pretty(&record)?)])
```

---

## Tool Definitions

### Phase 1 - Core Tools

#### `open_file`

Opens a JSON, NDJSON, JSONL, or GeoJSON file and returns a handle for subsequent operations.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `path` | `string` | yes | Path to the file (absolute or relative to CWD) |

**Returns:** A JSON object with `handle` (string), `record_count` (number), `file_type` (string), `file_size_bytes` (number).

**Internals:** Calls `sniff_file_type()` to auto-detect format, then `load_file_auto()` to create the appropriate loader (`NdjsonFile`, `JsonArrayFile`, or `SingleValueFile`). Stores the loader in `ServerState.files`.

#### `get_record`

Retrieves a single record by zero-based index.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file` | `string` | yes | File handle from `open_file` |
| `index` | `number` | yes | Zero-based record index |

**Returns:** The JSON record as pretty-printed text.

**Internals:** Calls `FileType::get(idx)` which lazily parses only the requested record. Leverages the LRU cache for repeated access.

#### `get_record_count`

Returns the total number of records in an open file.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file` | `string` | yes | File handle from `open_file` |

**Returns:** A JSON object with `count` (number).

**Internals:** Calls `FileType::len()`, which is O(1) - the count is computed at index time, not by scanning.

#### `get_file_info`

Returns metadata about an open file.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file` | `string` | yes | File handle from `open_file` |

**Returns:** A JSON object with `path`, `file_type` ("ndjson" | "json_array" | "json_object"), `record_count`, `file_size_bytes`.

#### `search`

Searches across all records in an open file. Automatically selects text search or JSONPath based on query syntax.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file` | `string` | yes | File handle from `open_file` |
| `query` | `string` | yes | Search query. Prefix with `$` for JSONPath (e.g., `$.user.name == "alice"`), otherwise performs full-text substring search |
| `match_case` | `boolean` | no | Case-sensitive matching (default: `false`) |
| `limit` | `number` | no | Maximum number of results to return (default: `100`) |

**Returns:** A JSON object with `total_matches` (number), `results` (array of objects with `record_index`, `preview`, `matched_path`).

**Internals:**
- **Text mode:** Uses Thoth's SIMD-accelerated `memmem` substring search with `rayon` parallelization. Only records whose raw bytes match the substring are parsed into JSON.
- **JSONPath mode:** Parses the query with `JsonPathQuery::parse()`, then evaluates against each record with `JsonPathQuery::evaluate()`. Supports dot notation, bracket notation, array indices, wildcards, and equality filters.

#### `close_file`

Closes an open file and releases its resources (memory, file handle, cached records).

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file` | `string` | yes | File handle from `open_file` |

**Returns:** A confirmation message indicating the file was closed.

**Internals:** Removes the entry from `ServerState.files`, dropping the loader and freeing all associated memory.

### Phase 2 - Data Tools

#### `get_value_at_path`

Retrieves a specific value by JSON path from a record.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file` | `string` | yes | File handle |
| `index` | `number` | yes | Record index |
| `path` | `string` | yes | Dot-separated JSON path (e.g., `user.address.city`) |

**Returns:** The value at the specified path, or an error if the path doesn't exist.

**Internals:** Uses `split_root_rel()` + `walk_rel()` from `src/helpers/json_copy_to_clipboard.rs`.

#### `extract_keys`

Lists all unique keys found across records at a given depth.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file` | `string` | yes | File handle |
| `depth` | `number` | no | Key depth to extract (default: `1` = top-level keys) |
| `sample_size` | `number` | no | Number of records to sample (default: `100`) |

**Returns:** Array of unique key names found at the specified depth.

#### `sample_records`

Returns a sample of records from the file.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file` | `string` | yes | File handle |
| `count` | `number` | no | Number of records to return (default: `5`) |
| `strategy` | `string` | no | `"first"`, `"last"`, or `"evenly_spaced"` (default: `"first"`) |

**Returns:** Array of sampled records with their indices.

#### `get_schema`

Infers a JSON schema from a sample of records.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file` | `string` | yes | File handle |
| `sample_size` | `number` | no | Records to sample for inference (default: `100`) |

**Returns:** An inferred JSON schema object describing the structure, types, and optionality of fields.

### Phase 3 - Advanced Tools

#### `filter_records`

Filters records by a JSONPath predicate and returns matching records.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file` | `string` | yes | File handle |
| `predicate` | `string` | yes | JSONPath filter expression (e.g., `$.status >= 500`) |
| `limit` | `number` | no | Max results (default: `100`) |

#### `aggregate`

Computes aggregations over a field across all records.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file` | `string` | yes | File handle |
| `field` | `string` | yes | Dot-path to the field (e.g., `response.duration_ms`) |
| `operations` | `string[]` | no | Operations to compute (default: `["count", "min", "max", "avg"]`) |

**Returns:** Object with computed aggregation values and `null_count`.

#### `compare_records`

Diffs two records and highlights differences.

| Parameter | Type | Required | Description |
|-----------|------|----------|-------------|
| `file` | `string` | yes | File handle |
| `index_a` | `number` | yes | First record index |
| `index_b` | `number` | yes | Second record index |

**Returns:** A structured diff showing added, removed, and changed fields.

---

## Implementation Plan

### Phase 1 - Core Scaffolding

- Add `rmcp` and `schemars` to `Cargo.toml`
- Create `src/mcp/` module structure
- Add `mcp serve` and `mcp list-tools` subcommand branches in `main.rs`
- Implement `ServerState` and `ThothMcpServer`
- Implement 6 core tools: `open_file`, `close_file`, `get_record`, `get_record_count`, `get_file_info`, `search`
- Integration test: register with Claude Code and run a basic query

**Deliverable:** A fully functional MCP server that can open files and search them.

### Phase 2 - Data Tools

- Implement `get_value_at_path`, `extract_keys`, `sample_records`, `get_schema`
- Schema inference logic (type union across sampled records)

### Phase 3 - Advanced Tools

- Implement `filter_records`, `aggregate`, `compare_records`
- Record diffing algorithm

### Phase 4 - Polish

- Error handling edge cases (corrupt files, permission errors, OOM on huge records)
- Documentation: README section, `--help` output
- Unit and integration tests for each tool
- CI: add MCP server tests to GitHub Actions

---

## Security Considerations

1. **File system access** - The MCP server can open any file the process has permission to read. This matches the threat model of other file-oriented MCP servers (e.g., `@modelcontextprotocol/server-filesystem`). The server does not write files.

2. **No network access** - The MCP server operates entirely locally. It does not make network requests (unlike WASM data source plugins, which have a network policy). It communicates only via stdin/stdout with its parent process.

3. **Resource limits** - Opening very large files consumes memory for the line/element index (not for parsed records, which are lazy). A future enhancement could add configurable limits on the number of simultaneous open files or total index memory.

4. **No code execution** - The server does not evaluate arbitrary code. JSONPath queries are parsed by Thoth's own safe parser, not by `eval()` or similar. Search queries are substring matches, not regex (avoiding ReDoS).

---

## Design Decisions

1. **File handle lifetime** - Handles persist for the lifetime of the server process. Inactivity-based cleanup may be added in a future phase if memory pressure becomes an issue in practice.

2. **Native formats only** - The MCP server only handles formats Thoth loads natively (JSON, NDJSON, JSONL, GeoJSON). WASM plugin loaders (e.g., CSV loader) are excluded because the `PluginManager` has a GUI-oriented lifecycle (background thread init, `OnceLock` globals, slow WASM runtime startup) that adds complexity and latency to the MCP path. Plugin support can be layered in as a future phase if there is demand.

3. **Limit-based result sets** - Search and filter tools use a `limit` parameter to cap results. Streaming via MCP's `notifications/progress` is a future enhancement for very large result sets.

4. **Configuration inheritance** - The MCP server loads and respects `~/.config/thoth/settings.toml` by default (LRU cache size, search parallelism, etc.). Individual settings can be overridden via CLI flags on `thoth mcp serve` (e.g., `--cache-size 500`).

5. **Explicit file closing** - A `close_file` tool is provided so that memory-conscious users and agents can release file handles without waiting for the server process to exit.

---

## Alternatives Considered

### Separate Binary (`thoth-mcp`)

One option would be to ship a separate `thoth-mcp` binary alongside the GUI app. This is the most common pattern in the MCP ecosystem, but it adds overhead for Thoth's use case. You'd need to distribute and version two binaries independently, and users would have to make sure both are on their PATH and kept in sync. The subcommand approach gives us the same technical result without any of that friction.

### Embedded MCP in the GUI (HTTP/SSE Transport)

Another option would be to run the MCP server as a thread inside the GUI app, using an HTTP/SSE transport instead of stdio. This would open the door to bidirectional features where an AI agent could, for example, ask Thoth to highlight a specific record in the open viewer. The tradeoff is significant added complexity: shared mutable state between the GUI and MCP threads, a more involved transport setup, and the requirement that the GUI must be running for the server to work at all. This could be worth revisiting later once the basic MCP server has proven its value, but it's not the right starting point.
