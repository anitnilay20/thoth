# Profiling Guide

Thoth includes comprehensive CPU and memory profiling tools for performance optimization, available only in development builds via the `profiling` feature flag.

## Quick Start

1. **Build with profiling:**

   ```bash
   cargo build --features profiling
   cargo run --features profiling
   ```

2. **View live CPU profiling:**
   - Press `Cmd+Alt+P` (macOS) or `Ctrl+Alt+P` (Windows/Linux)
   - The profiler window shows:
     - **Memory Profiling (dhat)**: Instructions for viewing detailed memory analysis
     - **CPU Profiling (puffin)**: Flamegraph with per-component execution time

3. **Analyze memory allocations:**
   - Use the app normally, then **exit cleanly by closing the window** (not Ctrl+C or force-quit)
   - After app exits, `dhat-heap.json` is written to your working directory
   - You'll see "dhat: Total: X bytes in Y blocks" printed to stderr
   - Open https://nnethercote.github.io/dh_view/dh_view.html
   - Click "Load" and select `dhat-heap.json`
   - View per-component memory allocations with full call stacks

## CPU Profiling (Puffin)

### What it shows:

- **Flamegraph**: Hierarchical view of function calls
- **Execution time**: How long each component takes to render
- **Call counts**: How many times each function is called per frame
- **Per-component breakdown**: See which parts of the UI are slow

### Instrumented components:

- `ThothApp::update` - Main update loop
- `ThothApp::render_toolbar` - Top toolbar rendering
- `ThothApp::render_central_panel` - Main content area
- `ThothApp::render_settings_panel` - Settings UI
- `CentralPanel::render` - Central panel component
- `SettingsPanel::render` - Settings panel component
- `JsonTreeViewer::render` - JSON tree rendering
- `JsonTreeViewer::rebuild_rows` - Row list generation
- `JsonTreeViewer::build_rows_from_value` - Recursive tree building
- `DataRow::render` - Individual row rendering

### How to use:

1. Press `Cmd+Alt+P` to open profiler
2. Interact with your app (open files, expand JSON, search, etc.)
3. Watch the flamegraph update in real-time
4. Look for:
   - Wide bars = functions taking a lot of time
   - Tall stacks = deep call hierarchies
   - Red/hot colors = CPU hotspots

## Memory Profiling (dhat)

dhat tracks all heap allocations during the app's lifetime. Unlike puffin (which shows live in-app), dhat analysis happens **after the app exits** via its output file.

### Detailed analysis with dhat viewer:

1. **Run your profiling session:**

   ```bash
   cargo run --features profiling
   # Use the app...
   # Close window normally
   ```

2. **Load the output:**
   - Open https://nnethercote.github.io/dh_view/dh_view.html
   - Click "Load" and select `dhat-heap.json`

3. **What you'll see:**
   - **Call tree**: Functions sorted by total bytes allocated
   - **Stack traces**: Full call stack for each allocation point
   - **Per-component view**: Memory allocated by DataRow, JsonTreeViewer, etc.
   - **Timeline**: Memory usage over time
   - **Peak analysis**: What was allocated at peak memory usage

4. **Finding memory issues:**
   - Sort by "Total bytes" to find biggest allocators
   - Look for:
     - Unexpected large allocations
     - Allocations that should have been freed (memory leaks)
     - Redundant allocations in hot paths
   - Click functions to see their call stacks and source locations

## Performance Tips

### What to look for:

**CPU (Puffin):**

- Functions called too frequently (high count)
- Functions taking too long (wide bars)
- Unnecessary work in render loops

**Memory (dhat):**

- Growing "Current memory" = potential leak
- Large "Peak memory" = optimization opportunity
- High allocation counts in hot paths = consider caching

### Common optimizations:

1. **Cache JSON parsing results** - Avoid re-parsing
2. **Lazy rendering** - Only render visible rows
3. **Reduce allocations** - Reuse buffers, use references
4. **Batch operations** - Reduce per-frame work

## Zero-Cost Abstraction

When profiling is **disabled** (default build):

- ✅ No dhat dependency included
- ✅ No puffin dependency included
- ✅ No runtime overhead
- ✅ No profiling code compiled
- ✅ Smaller binary size

Release builds should **never** include the `profiling` feature.

## Troubleshooting

### dhat-heap.json not generated?

- **Must exit cleanly**: Close the window normally, don't use Ctrl+C or kill the process
- The profiler writes the file when its destructor runs at the end of `main()`
- Check you built with `--features profiling`
- File is written to the directory where you ran `cargo run`
- You should see "dhat: Total: ... bytes" printed to stderr when it writes
- If killed forcefully (SIGKILL), the destructor doesn't run and no file is created

### Profiler window not showing?

- Press `Cmd+Alt+P` / `Ctrl+Alt+P`
- Check you built with `--features profiling`
- Check Settings → Developer → Show Profiler is enabled

### High profiling overhead?

- Normal - profiling has performance cost
- Don't profile in release builds
- Consider profiling specific operations instead of full sessions

## Technical Details

### Tools used:

- **puffin**: Lightweight instrumentation profiler for per-scope CPU time
- **puffin_egui**: In-app UI for puffin flamegraphs
- **dhat**: Heap allocation profiler from Valgrind suite

### Feature flag:

```toml
[features]
profiling = ["puffin", "puffin_egui", "dhat"]
```

All profiling dependencies are optional and only included when the feature is enabled.
