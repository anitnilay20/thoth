<div align="center">
  <h1>
    <img src="../assets/thoth_icon_256.png" alt="Thoth Icon" width="75" style="vertical-align: middle;"/>
    Thoth MCP Server
  </h1>
  <p><em>Expose Thoth's JSON/NDJSON inspection capabilities to AI assistants</em></p>
</div>

<div align="center">

[![CI](https://github.com/anitnilay20/thoth/workflows/CI/badge.svg)](https://github.com/anitnilay20/thoth/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](https://opensource.org/licenses/MIT)

</div>

Thoth includes a built-in [Model Context Protocol (MCP)](https://modelcontextprotocol.io/) server that lets AI assistants — Claude, GitHub Copilot, Cursor, and others — open, search, and analyze JSON/NDJSON files through Thoth's high-performance engine.

The MCP server runs as a **headless subprocess** (no GUI) over a stdio JSON-RPC transport. It reuses Thoth's battle-tested file loaders, SIMD-accelerated search, and JSONPath engine — the same code that powers the desktop app.

---

## Quick Start

### 1. Build Thoth

```bash
git clone https://github.com/anitnilay20/thoth.git
cd thoth
cargo build --release
```

### 2. Register with Your AI Client

All MCP clients use the same pattern — point them at the `thoth` binary with `mcp serve` arguments.

#### Claude Desktop

Add to `~/Library/Application Support/Claude/claude_desktop_config.json` (macOS) or `%APPDATA%\Claude\claude_desktop_config.json` (Windows):

```json
{
  "mcpServers": {
    "thoth": {
      "command": "/path/to/thoth",
      "args": ["mcp", "serve"]
    }
  }
}
```

#### Claude Code (CLI)

```bash
claude mcp add --transport stdio thoth -- /path/to/thoth mcp serve
```

#### GitHub Copilot

Add to your VS Code `settings.json` or `.vscode/mcp.json`:

```json
{
  "mcp": {
    "servers": {
      "thoth": {
        "type": "stdio",
        "command": "/path/to/thoth",
        "args": ["mcp", "serve"]
      }
    }
  }
}
```

#### Cursor

Add to `~/.cursor/mcp.json`:

```json
{
  "mcpServers": {
    "thoth": {
      "command": "/path/to/thoth",
      "args": ["mcp", "serve"]
    }
  }
}
```

#### Any MCP Client

The server uses **stdio transport** (stdin/stdout JSON-RPC), which is supported by all MCP-compatible clients. Use the same pattern:

```
command: /path/to/thoth
args:    mcp serve
```

> **Note:** After registering, restart your AI client. MCP servers are loaded at session startup.

### 3. Use It

Start a new session in your AI client and ask:

> "Use thoth to open `data.ndjson`, show me the schema, and search for records containing 'error'"

---

## Available Tools

Thoth exposes **10 tools** organized into two groups:

### Core Tools

| Tool | Description |
|------|-------------|
| `open_file` | Open a JSON, NDJSON, or GeoJSON file. Returns a handle for subsequent operations. |
| `close_file` | Close a previously opened file, freeing its resources. |
| `get_file_info` | Get metadata: file path, detected format, and record count. |
| `get_record` | Retrieve a single record by zero-based index. |
| `get_record_count` | Get the total number of top-level records. |
| `search` | Search records by text substring or JSONPath query. Auto-detects mode from query prefix (`$`). |

### Data Tools

| Tool | Description |
|------|-------------|
| `get_value_at_path` | Extract a nested value using dot-notation (e.g. `user.address.city`, `items[2].name`). |
| `extract_keys` | List all unique keys found across records, optionally at a nested path. |
| `sample_records` | Return a sample of records: `first` (default), `last`, or `even` (evenly spaced). |
| `get_schema` | Infer a JSON schema from sampled records — types, properties, and required fields. |

---

## Tool Reference

### open_file

Opens a file and returns a handle for use with other tools. Thoth automatically detects the format.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `path` | string | ✅ | Absolute or relative path to the file |

**Returns:** `{ handle, path, file_type, record_count }`

**Supported formats:**
- **NDJSON** (`.ndjson`, `.jsonl`) — Newline-delimited JSON
- **JSON Array** (`.json`) — Files containing a top-level array
- **JSON Object** (`.json`, `.geojson`) — Files containing a single object

---

### close_file

Closes a file and frees its resources.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `handle` | string | ✅ | Handle returned by `open_file` |

---

### get_file_info

Returns metadata about an open file.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `handle` | string | ✅ | Handle returned by `open_file` |

**Returns:** `{ handle, path, file_type, record_count }`

---

### get_record

Retrieves a single JSON record by index.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `handle` | string | ✅ | Handle returned by `open_file` |
| `index` | number | ✅ | Zero-based record index |

**Returns:** `{ index, record }` where `record` is the pretty-printed JSON string.

---

### get_record_count

Returns the total number of top-level records.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `handle` | string | ✅ | Handle returned by `open_file` |

**Returns:** `{ record_count }`

---

### search

Search across all records using text or JSONPath.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `handle` | string | ✅ | Handle returned by `open_file` |
| `query` | string | ✅ | Search query. Prefix with `$` for JSONPath (e.g. `$.user.name`) |
| `mode` | string | | `"text"` or `"jsonpath"`. Auto-detected from query prefix if omitted |
| `match_case` | boolean | | Case-sensitive matching. Default: `false` |
| `max_results` | number | | Maximum results to return. Default: `50` |

**Returns:** `{ total_matches, matches: [{ record_index, preview, match_path }], query, mode }`

**Examples:**
```
# Text search
{ "handle": "file_1", "query": "error" }

# JSONPath — find all records with a user.name field
{ "handle": "file_1", "query": "$.user.name" }

# JSONPath with filter
{ "handle": "file_1", "query": "$.status = \"active\"" }
```

---

### get_value_at_path

Extract a specific nested value from a record.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `handle` | string | ✅ | Handle returned by `open_file` |
| `index` | number | ✅ | Zero-based record index |
| `path` | string | ✅ | Dot-notation path (e.g. `user.address.city`, `items[2].name`) |

**Returns:** `{ value, path, value_type }`

---

### extract_keys

Discover all unique keys across records.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `handle` | string | ✅ | Handle returned by `open_file` |
| `path` | string | | Nested path to inspect (e.g. `"user"` for keys under the user object). Empty = top-level |
| `sample_size` | number | | Number of records to sample. Default: `100` |

**Returns:** `{ keys: [...], records_sampled }`

---

### sample_records

Return a representative sample of records.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `handle` | string | ✅ | Handle returned by `open_file` |
| `count` | number | | Number of records. Default: `5` |
| `strategy` | string | | `"first"` (default), `"last"`, or `"even"` (evenly spaced) |

**Returns:** `{ records: [{ index, record }], total_records, strategy }`

---

### get_schema

Infer a JSON schema from the data.

**Parameters:**
| Name | Type | Required | Description |
|------|------|----------|-------------|
| `handle` | string | ✅ | Handle returned by `open_file` |
| `sample_size` | number | | Records to sample. Default: `50` |

**Returns:** `{ schema: { type, properties, required }, records_sampled }`

---

## Architecture

```
┌──────────────────────────────────────────────────────┐
│                    AI Client                         │
│            (Claude / Copilot / Rovo / Cursor)        │
└────────────────────┬─────────────────────────────────┘
                     │ JSON-RPC over stdio
                     ▼
┌──────────────────────────────────────────────────────┐
│              Thoth MCP Server                        │
│                                                      │
│  ┌─────────────┐  ┌──────────┐  ┌─────────────────┐ │
│  │ ServerState  │  │  Tools   │  │  ServerHandler   │ │
│  │ (file mgmt)  │  │ (10 ops) │  │ (rmcp + stdio)  │ │
│  └──────┬──────┘  └────┬─────┘  └─────────────────┘ │
│         │              │                              │
│  ┌──────▼──────────────▼────────────────────────────┐│
│  │           Thoth Core (shared with GUI)            ││
│  │                                                   ││
│  │  File Loaders   Search Engine   JSONPath   Cache  ││
│  │  (NDJSON,JSON)  (SIMD memmem)   Parser    (LRU)  ││
│  └───────────────────────────────────────────────────┘│
└──────────────────────────────────────────────────────┘
```

### Key Design Decisions

- **Early branching**: `thoth mcp serve` branches in `main()` **before** any GUI initialization. No windows open, no GPU context created.
- **stdout discipline**: All diagnostic output goes to stderr. stdout is reserved exclusively for JSON-RPC messages.
- **Shared core**: File loaders, search engine, JSONPath, and caching are identical to the GUI app — no duplication.
- **Thread-safe state**: `ServerState` uses `Arc<Mutex<>>` for safe concurrent access to open files.
- **Handle-based API**: Files are referenced by opaque handles (`file_1`, `file_2`, ...) rather than paths, enabling multiple open files and clean resource management.

---

## CLI Reference

```bash
# Start the MCP server (stdio transport)
thoth mcp serve

# List available tools
thoth mcp list-tools
```

### Environment Variables

| Variable | Description |
|----------|-------------|
| `RUST_LOG` | Set log level: `error`, `warn`, `info` (default), `debug`, `trace` |

```bash
# Run with debug logging (goes to stderr, won't interfere with JSON-RPC)
RUST_LOG=debug thoth mcp serve
```

---

## Usage Examples

### Example 1: Open a File and Read a Record

**You ask:**
> "Use thoth to open `tests/fixtures/ndjson/simple.ndjson` and tell me what's in record 3"

**What the AI does:**

```
→ Called open_file({ path: "tests/fixtures/ndjson/simple.ndjson" })
← { file_type: "ndjson", handle: "file_1", path: "/full/path/to/simple.ndjson", record_count: 10 }

→ Called get_record({ handle: "file_1", index: 3 })
← { index: 3, record: "{\n  \"age\": 28,\n  \"id\": 4,\n  \"name\": \"Diana\"\n}" }
```

**AI responds:**

> Record 3 (zero-based index) contains a person named **Diana**, age 28, with id 4. The file contains 10 records total.

---

### Example 2: Full Data Exploration (Schema + Keys + Samples + Path Lookup)

**You ask:**
> "Use thoth to open `simple.ndjson`, then show me the schema, unique keys, 3 evenly-spaced samples, and the name in record 5"

**What the AI does** (calls tools in parallel for efficiency):

```
→ Called open_file({ path: "tests/fixtures/ndjson/simple.ndjson" })
← { file_type: "ndjson", handle: "file_1", record_count: 10 }

→ Called get_schema({ handle: "file_1" })
→ Called extract_keys({ handle: "file_1" })
→ Called sample_records({ handle: "file_1", count: 3, strategy: "even" })
→ Called get_value_at_path({ handle: "file_1", index: 5, path: "name" })

← get_schema:        { schema: { type: "object", properties: { age: { type: "number" },
                        id: { type: "number" }, name: { type: "string" } },
                        required: ["age", "id", "name"] }, records_sampled: 10 }

← extract_keys:      { keys: ["age", "id", "name"], records_sampled: 10 }

← sample_records:    { records: [
                        { index: 0, record: { id: 1, name: "Alice", age: 30 } },
                        { index: 4, record: { id: 5, name: "Eve",   age: 32 } },
                        { index: 9, record: { id: 10, name: "Jack", age: 41 } }
                      ], total_records: 10, strategy: "even" }

← get_value_at_path: { value: "\"Frank\"", value_type: "string", path: "name" }
```

**AI responds:**

> **Schema** — All 10 records share a consistent structure:
>
> | Field | Type   | Required |
> |-------|--------|----------|
> | id    | number | ✅       |
> | name  | string | ✅       |
> | age   | number | ✅       |
>
> **Unique Keys:** `age`, `id`, `name`
>
> **Evenly-Spaced Samples:**
>
> | Index | Record                               |
> |-------|--------------------------------------|
> | 0     | `{ id: 1, name: "Alice", age: 30 }`  |
> | 4     | `{ id: 5, name: "Eve", age: 32 }`    |
> | 9     | `{ id: 10, name: "Jack", age: 41 }` |
>
> **Value at `name` in record 5:** `"Frank"` (string)

---

### Example 3: Search for Specific Data

**You ask:**
> "Find all records where the status is 'failed'"

**What the AI does:**

```
→ Called open_file({ path: "logs.ndjson" })
← { handle: "file_1", file_type: "ndjson", record_count: 50000 }

→ Called search({ handle: "file_1", query: "failed", max_results: 10 })
← { total_matches: 147, matches: [
     { record_index: 42, preview: "...status\":«failed»,..." },
     { record_index: 891, preview: "...status\":«failed»,..." },
     ...
   ], mode: "text" }

→ Called get_record({ handle: "file_1", index: 42 })
← { index: 42, record: "{ \"id\": 43, \"status\": \"failed\", \"error\": \"timeout\" }" }
```

---

### Typical Workflow Patterns

**Explore an unknown dataset:**
1. `open_file` → learn the format and record count
2. `get_schema` → understand the structure
3. `extract_keys` → see all fields
4. `sample_records` (first + last) → see representative data

**Search and drill down:**
1. `open_file` → open the file
2. `search` with text or JSONPath → find matching records
3. `get_record` → retrieve full details of interesting matches

**Analyze nested structures:**
1. `open_file` → open the file
2. `extract_keys` with `path: "user.address"` → discover nested fields
3. `get_value_at_path` with `path: "user.address.city"` → extract specific values

---

## Troubleshooting

### Server doesn't start

1. Verify the binary path: `which thoth` or use an absolute path
2. Test manually: `echo '{}' | thoth mcp serve 2>&1` — you should see logs on stderr

### Client doesn't see tools

1. Restart your AI client (MCP servers are loaded at session startup)
2. Check the config file syntax — it must be valid JSON
3. Verify `thoth mcp list-tools` shows all 10 tools

### Search returns no results

- Text search is case-insensitive by default. Set `match_case: true` for exact matching.
- JSONPath queries must start with `$` (e.g. `$.user.name`, not `user.name`)

---

## Development

### Running Tests

```bash
# Run all MCP tests
cargo test --lib mcp::tests

# Run the full test suite (including MCP)
cargo test
```

### Project Structure

```
src/mcp/
├── mod.rs       # Module entry point, CLI dispatcher
├── server.rs    # Async server startup, stdio transport wiring
├── state.rs     # ServerState — thread-safe file handle management
├── tools.rs     # 10 MCP tool definitions + schema inference
└── tests.rs     # 40 comprehensive tests
```

---

## Roadmap

Future MCP server enhancements planned:

- **Phase 3**: Advanced tools — `filter_records`, `aggregate`, `compare_records`
- **Plugin support**: Expose WASM plugin file loaders (CSV, etc.) through MCP
- **Streamable HTTP transport**: For remote/cloud deployments
- **Resource support**: Expose open files as MCP resources
- **Prompt templates**: Pre-built analysis prompts for common workflows
