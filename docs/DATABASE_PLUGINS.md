# Database Plugins — Design Notes

This document captures the design exploration for supporting **database data-source
plugins** in Thoth. It complements [PLUGIN_SYSTEM.md](PLUGIN_SYSTEM.md), which
describes the existing WASM-based plugin architecture, and supersedes the brief
["Roadmap: Database Plugins via WASI Sockets"](PLUGIN_SYSTEM.md#roadmap-database-plugins-via-wasi-sockets)
section with a more concrete plan.

---

## Table of Contents

- [The Problem](#the-problem)
- [Two Distinct Needs](#two-distinct-needs)
- [Options Considered](#options-considered)
- [Recommended Path](#recommended-path)
- [Option A — Host-Provided TCP-Socket Import](#option-a--host-provided-tcp-socket-import)
- [Option C — Subprocess Plugins (Native-Only Escape Hatch)](#option-c--subprocess-plugins-native-only-escape-hatch)
- [The Async Question](#the-async-question)
- [The Owner-Thread Model](#the-owner-thread-model)
- [Why Not Simulate Async Inside WASM](#why-not-simulate-async-inside-wasm)
- [Cross-Cutting Concerns](#cross-cutting-concerns)
- [Summary](#summary)

---

## The Problem

The current plugin system compiles plugins to **WebAssembly** and runs them in a
**Wasmtime** sandbox. Data-source plugins reach the outside world through a single
host-provided import, `http-client`, which covers REST/HTTP sources cleanly.

WASM is the right default for portability, safety, and single-binary distribution.
But it **cannot open raw network sockets**, which blocks native database wire
protocols (PostgreSQL, MySQL, Redis RESP, MongoDB, etc.). A naive workaround —
compiling drivers as native dynamic libraries (`.so`/`.dylib`/`.dll`) — gives up
the sandbox and reintroduces Rust ABI fragility and per-platform packaging.

This doc records the alternatives evaluated and the chosen direction.

---

## Two Distinct Needs

The databases split into two groups, and they have **different** answers:

1. **Socket + wire-protocol databases** — Postgres, MySQL, Redis, MongoDB,
   Cassandra. These only need a raw TCP socket; pure-Rust **sync** drivers already
   exist. The sole constraint is "WASM can't open sockets."
2. **Native-client-only databases** — Oracle (OCI is a proprietary C library) and
   anything else with no pure-Rust driver. No amount of socket access helps; these
   need to link native code that will never compile to WASM.

Conflating the two leads to over-engineering. The DLL idea solves group 2 but is a
sandbox regression for group 1.

---

## Options Considered

| Approach | Sandbox | DB driver source | Cross-platform | Reuses `network_policy` | Best for |
|---|---|---|---|---|---|
| **A. Host-provided TCP-socket WIT import** | ✅ kept | Pure-Rust, compiled to WASM, in the plugin | ✅ single `.wasm` | ✅ **yes** | Most DBs (group 1) |
| **B. WASI P2 `wasi:sockets/tcp`** | ✅ kept | Pure-Rust, in the plugin | ✅ single `.wasm` | ⚠️ partial — policy must move into the WASI socket layer | Most DBs (group 1) |
| **C. Subprocess plugin** (stdio / JSON-RPC / gRPC) | ✅ process isolation | **Any** native driver, any language | ❌ per-platform binary | ❌ re-implemented | Oracle / native-only (group 2), perf |
| **D. Native dynamic lib** (`.so`/`.dylib`/`.dll`) | ❌ full process access | Any native Rust driver | ❌ per-platform build | ❌ | Last resort only |

### Why DLLs (D) lose

Dynamic libs give up the single biggest property the plugin system advertises — the
sandbox — and still pay a per-platform packaging cost, plus Rust ABI instability
(requiring a C-ABI boundary via `abi_stable`/`stabby`). They pay the subprocess's
*distribution* cost **and** a security cost. Subprocess (C) dominates them on every
axis except in-process call latency, which is irrelevant next to a network round-trip.

### A vs B

Both keep the sandbox and ship a single `.wasm`. **A is preferred** because:

- It reuses the existing `network_policy.rs` layer (allowlist, SSRF guard, consent
  popup, rate limiter) verbatim — the host still owns the socket.
- It does **not** require migrating the toolchain from `wasm32-wasip1` to
  `wasm32-wasip2`.

With B, all of that policy enforcement would have to be re-imposed inside the WASI
socket layer, which is harder to gate.

---

## Recommended Path

- **Now:** Implement **Option A** (host `tcp-client` import). Unlocks Postgres,
  MySQL, Redis, MongoDB, Cassandra on the current `wasip1` target, reuses
  `network_policy.rs`, and slots into the existing `data-source` handle model. Less
  work and less risk than the `wasip2` migration, with the same outcome.
- **Later / as-needed:** Add a **subprocess** plugin kind (Option C) as the
  documented escape hatch for native-only drivers (Oracle), replacing the "Native
  dylib exception" row in the PLUGIN_SYSTEM roadmap with something that keeps
  isolation.

---

## Option A — Host-Provided TCP-Socket Import

This is the natural extension of the existing `http-client` pattern: the host owns
the network resource; the plugin drives it through WIT. The pure-Rust wire-protocol
driver lives **in the plugin** (consistent with the "drivers live in the plugin, not
the host" principle), running over a `Read + Write` shim wrapped around the import.

### Proposed WIT

```wit
interface tcp-client {
    use types.{plugin-error};

    resource tcp-stream {
        write: func(bytes: list<u8>) -> result<u32, plugin-error>;
        read:  func(max: u32) -> result<list<u8>, plugin-error>;
    }

    /// Host enforces the SAME network-policy layer as http-client:
    /// domain allowlist, SSRF guard, consent popup, rate limiter, timeouts.
    connect: func(host: string, port: u16, tls: bool) -> result<tcp-stream, plugin-error>;
}
```

### Why it fits Thoth specifically

- **Reuses `network_policy.rs`** — the SSRF guard (DNS-answer checking) matters even
  more for raw TCP than for HTTP. Allowlist, consent flow, and rate limiting all
  apply unchanged.
- **Maps onto the `data-source` interface.** A DB connection is long-lived and
  stateful, unlike HTTP request/response. The host holds the real socket; the
  plugin's `connect() -> handle`, `query(handle)`, `close(handle)` already model
  exactly this. The `tcp-stream` resource *is* the handle.
- **Drivers stay in the plugin.** The plugin wraps `tcp-stream` as `Read + Write`
  and runs the **sync** `postgres` / `mysql` / RESP driver over it.

### Gotchas

- **TLS:** `rustls` compiles to WASM; `native-tls`/OpenSSL do not. Either expose
  `tls: bool` and terminate TLS host-side (simplest — keeps cert handling out of the
  sandbox), or bundle `rustls` into the plugin.
- **Async drivers won't work** (`tokio-postgres`). Use the sync crates (`postgres`,
  `mysql`) — see [The Async Question](#the-async-question).

---

## Option C — Subprocess Plugins (Native-Only Escape Hatch)

For the native-only tier, prefer an **out-of-process subprocess** over a dylib.
Same isolation benefit, none of the dylib downsides:

- **Crash containment** — a segfault in the Oracle client kills the subprocess, not
  Thoth.
- **OS-level sandboxing** still possible (`sandbox-exec` on macOS, seccomp on Linux).
- **Language / ABI freedom** — link any C driver; no Rust ABI fragility.
- **Proven model** — LSP, DAP, and HashiCorp's `go-plugin` all do exactly this for
  the same reason.

Cost is IPC serialization + per-platform binaries, but for a DB query path that is
negligible next to network round-trips.

---

## The Async Question

The only real gap with sync drivers is **blocking I/O**. Crucially, the fix is
**not** to simulate async inside the plugin — it's to keep the driver synchronous
and push the blocking off the render thread **host-side**.

Two different "asyncs" must not be conflated:

1. **UI-responsiveness async** — "don't freeze the render loop while waiting." Already
   solved by `http-client::submit()`: host spawns a background thread, plugin polls
   the result via `handle-event`.
2. **Driver-internal async** — `tokio-postgres` wanting a tokio runtime + an I/O
   reactor (epoll/kqueue) + a futures executor inside WASM. **Avoid this entirely.**
   Use the *sync* `postgres`/`mysql`/RESP crates, which do plain blocking reads/writes
   over a `Read + Write` — no runtime needed.

### Difference from the HTTP case

With HTTP `submit()`, the *I/O* (reqwest) runs on a host thread; the WASM call
(`handle_event`) that processes the result is fast JSON parsing on the main thread —
the instance is never blocked for long.

With a DB driver living **inside** the plugin, the blocking now happens *inside WASM*:
the sync driver loops on `tcp-stream.read()` until the DB answers. So the WASM call
itself (`query()`) is long-blocking and cannot run on the render thread.

---

## The Owner-Thread Model

This is **forced by Wasmtime**, not optional polish: a `Store` is `!Sync` and only one
call can be in flight at a time. The moment `query()` can block, you cannot also call
`render_pane()` / `handle_event()` on the same instance from the UI thread.

So each data-source plugin instance gets a dedicated host **owner thread** that owns
its `Store`:

```text
UI/render thread                 owner thread (owns the wasmtime Store)
     │  send Command::Query(q) ─────────▶ │  loader.query(handle, q)   ← blocks here, fine
     │  (returns immediately)             │     driver ↔ tcp-stream.read/write (blocking)
     │  ...renders spinner...             │
     │  ◀──── QueryResult(json) ──────────┤  send result over channel
     │  feed into handle_event next frame │
```

The render thread sends `connect` / `query` / `close` commands over a channel and
polls for results — reusing the existing `pending_count` + `poll_plugin_http_results`
machinery, generalized from "poll HTTP results" to "poll plugin-call results." This is
`submit()` lifted from *"host does the I/O"* to *"host runs the whole blocking call on
a worker thread."*

The owner thread is also the natural home for the long-lived connection + socket state
across the `connect → query → close` lifecycle — matching the existing handle model.

---

## Why Not Simulate Async Inside WASM

Simulating async properly means: non-blocking host socket reads (return `WouldBlock`),
a guest-side executor, and an incremental `poll()` export the host calls each frame to
step the state machine. That is rebuilding a reactor — and async drivers *still* won't
work without a real waker tied to I/O readiness. You would write a lot of plumbing to
arrive at worse ergonomics than `sync driver + owner thread`, which delivers identical
UX (spinner while loading, no UI freeze) for a fraction of the code.

---

## Cross-Cutting Concerns

- **Timeouts:** Fuel bounds CPU, but a blocking `read()` on a hung connection consumes
  no fuel — it waits forever. Put read/connect timeouts on the host `tcp-client`
  import. This doubles as protection against slow-loris-style hangs and pairs with the
  SSRF guard.
- **Cancellation:** "User closed the tab mid-query." With the owner-thread model, drop
  the stream / close the socket to unblock the read and tear the thread down cleanly.
- **TLS placement:** Terminating TLS host-side keeps certificate handling out of the
  sandbox and lets every plugin share one TLS implementation.
- **Connection lifetime:** DB connections are long-lived and stateful — the owner
  thread holds them for the instance's lifetime, unlike the request/response HTTP path.

---

## Summary

| Decision | Choice |
|---|---|
| Group 1 DBs (Postgres/MySQL/Redis/Mongo/Cassandra) | **Option A** — host `tcp-client` import, sync driver in the plugin |
| Group 2 DBs (Oracle / native-only) | **Option C** — subprocess plugin (later) |
| Dynamic libraries (Option D) | Rejected — sandbox regression + ABI/packaging cost |
| WASI P2 sockets (Option B) | Deferred — A achieves the same on `wasip1` without losing policy enforcement |
| Async | Sync drivers + per-instance **owner thread**; no in-WASM async simulation |
| Migration to `wasm32-wasip2` | Not required for the recommended path |
