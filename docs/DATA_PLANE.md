# Papyrus — the data plane

Status: accepted · 2026-11-19 · tracking issue #152

Papyrus is the host's single owned view of tabular data. Everything that
displays data — the grid, the JSON tree, Map/Chart Studio, plugin renderers and
exporters — reads it **by handle**, in pages, from one registry.

```
 producer                        Papyrus registry                consumer
─────────                        ────────────────                ────────
 DuckDB / file  ─ publish_arrow ─▶  Source::Arrow  ◀─ read ───── DataView
 plugin push    ─ publish ───────▶  Source::Rows   ◀─ read ───── JSON tree
 text / lines   ─ publish_text ──▶  Source::Text   ◀─ read ───── Chart Studio
 plugin pull    ─ provide-dataset▶                  ◀─ dataset-read ─ renderer
                                   handles: ds-N                └ exporter
                                   revision · pins · LRU
```

## Why

A plugin must never receive a copy of the rows it is being shown. Before the
bus, export/render paths serialised the whole dataset into a JSON string, Chart
Studio resolved producers directly, and every consumer had its own data path.
Papyrus gives one place to own data, one way to read it, and one lifecycle.

## Sheets

A handle points at one of three kinds of sheet (`thoth-core/src/papyrus/mod.rs`):

| Source | Owner | Read | Cost |
|---|---|---|---|
| `Arrow` | a live `FileLoader` (DuckDB) | window `[offset, offset+limit)` | nothing materialized; one `RecordWindow` |
| `Rows` | plugin push (`dataset-bus`) | owned string rows + type hints | LRU-bounded: 32 sheets / 128 MB |
| `Text` | a `TextIndex` line index | pages of lines | index only; leaves, no children |

Arrow keeps real types, nulls and nesting. Pushed rows are strings with a
per-column SQL hint, so nested values arrive flattened. Text has no null
concept (`Page::nulls` is empty, read as “present”).

## Lifecycle

- `publish` (plugin push) replaces that **instance's** previous dataset.
- `publish_arrow` / `publish_arrow_relation` add a live engine, or one named
  relation of it (a document's collection, a database's table). Relation
  handles are independent: re-publishing one leaves its siblings live.
- `update(handle)` replaces columns+rows in place, keeping the handle stable.
- `append(handle)` adds rows; the sheet keeps a 50,000-row ring buffer.
- `release(handle)` drops a dataset early; `retain_instances` drops everything
  from a producer tab that closed.
- Every mutation bumps `revision`. Consumers use it to invalidate caches and
  re-read (Chart Studio, the render cache).
- `pin(handle)` / `unpin(handle)` are reference counts. Budget eviction skips
  pinned sheets; an explicit `release` still drops them.

Pushed rows count against the 128 MB budget; Arrow and Text sheets hold an
index/window and contribute ~nothing. LRU evicts down to at most 32 datasets.

## Producers

- **File engine.** Indexing opens DuckDB off-thread; the tab publishes with
  `publish_arrow_with_total` (row count computed once, not re-scanned).
- **Plugin push.** A plugin imports `dataset-bus` and calls `publish(name,
  columns, rows)`; it gets a handle to embed in a `data-view` node.
- **Plugin pull.** A plugin exports `data-producer.provide-dataset`; the host
  calls it on demand and publishes the result, so consumers see a handle.
- **Typed push.** A plugin may send an Arrow IPC stream with
  `dataset-bus.publish-arrow`; the host parses it once into record batches and
  serves windows like an engine-backed sheet. `dataset-read.read-arrow` returns
  a window as Arrow IPC, preserving types, nulls and nesting; string-only
  sheets return an error there and are read with `read`.
- **Relations.** `FileViewer::handle_for_relation` returns the tab's current
  handle or publishes the requested relation as its own sheet.

## Consumers

- **DataView / JSON tree** read by handle through the host-installed resolver
  and `dataset_access` (`thoth-ui/src/main.rs`). The tree asks for a node's
  children, never the whole document.
- **Chart Studio** binds `{handle, revision}` and re-reads when the revision
  moves. It reads up to the chart row cap; large engine-backed sources should
  aggregate/sample rather than materialize.
- **Renderers and exporters** import `dataset-read` (WIT v0.3.0):
  `info(handle)`, `read(handle, offset, limit)` with a NULL mask,
  `read-arrow(handle, offset, limit)`, `pin`/`unpin`. The host hands them a
  handle; the plugin pages the rows. The CSV exporter streams pages straight to
  its output.
- **Consent.** Reading another producer's handle is gated by `dataset_grants`:
  the host asks before handing the data to a renderer/exporter that is not the
  producer.

## Limits

| Constant | Value | Meaning |
|---|---|---|
| `MAX_READ_LIMIT` | 1,000 | rows per `read` |
| `MAX_DATASETS` | 32 | datasets before LRU |
| `MAX_BYTES` | 128 MB | pushed-row budget |
| `MAX_STREAM_ROWS` | 50,000 | `append` ring buffer |
| `to_dataset::CAP` | 5,000 | rows Chart Studio reads |

## Decisions

- **Handles, not copies.** Plugins never receive rows to display; they receive
  an opaque handle and page it.
- **Revisioned.** In-place `update` and `append` must invalidate caches; a
  row-count check is not enough.
- **Pinnable.** A consumer holding a handle must be able to stop eviction;
  otherwise a chart can lose its source mid-session.
- **Paged.** Consumers read only what they draw (or stream, for exports).
- **Process-local.** Each Thoth window is a separate process with its own
  registry; handles do not cross windows.
- **Consent-gated.** Cross-producer reads reuse the existing grant flow.

## Follow-ups

- **Chart aggregation/sampling.** Push large engine-backed charts to DuckDB
  instead of materializing the first N rows.
- **DataView pinning.** Pin the handle a file tab is displaying, or re-publish
  it if evicted.
- **Arrow IPC compression.** `lz4`/`zstd` for large typed pushes.

## Map

- Registry, sources, limits: `thoth-core/src/papyrus/mod.rs`
- Relation-scoped loaders: `thoth-core/src/file/loaders/mod.rs`
- WIT: `wit/thoth-plugin.wit` (`dataset-bus`, `data-producer`, `dataset-read`)
- Host read bridge: `thoth-ui/src/app/thoth_app.rs` (`resolve_dataset_for_view`,
  `set_dataset_access` in `thoth-ui/src/main.rs`)
- Plugin runners: `thoth-core/src/plugin/wasm_{exporter,renderer}.rs`
