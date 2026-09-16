# Working in this repo

Conventions that are easy to violate by accident, and expensive to unpick later.

## UI: use the SDK's components

**Every widget comes from `thoth-plugin-sdk`.** Do not hand-roll `ui.label`,
`ui.button`, `ui.selectable_label` or a bespoke row when a component exists —
the SDK carries the design system (colours, spacing, hover and selection
states, dark/light behaviour), and a hand-rolled widget silently diverges from
it on one of those axes.

Available under `thoth_plugin_sdk::components`:

| need | component |
|---|---|
| text | `Typography` (`TypographyVariant::Body`, `BodyMuted`, `Mono`, …) |
| a list of rows | `List` + `ListItem` — supports `selected`, `description`, `badge`, per-row actions |
| tabular data | `TableView`, or `DataView` when it is bound to a Papyrus handle |
| a JSON tree | `JsonTree` |
| buttons | `Button`, `IconButton`, `ButtonGroups` |
| inputs | `Input`, `NumberInput`, `Select`, `MultiSelect`, `Checkbox`, `Radio`, `ToggleSwitch`, `Slider` |
| containers | `Card`, `Modal`, `Collapsible`, `Tabs`, `Separator` |
| feedback | `Spinner`, `Progress`, `Badge` |
| code | `Code`, `CodeEditor`, `Markdown` |
| a tree/data row | `DataRow` — handles indent, caret, syntax tokens and highlight ranges |

If a component is missing, add it to the SDK rather than inlining a one-off in
the host: the host and every plugin then get it, and it stays consistent.

**Raw `egui` is still correct for layout and plumbing** — `ui.horizontal`,
`allocate_ui_with_layout`, `ScrollArea`, `Frame`, `vec2`. The SDK's own
components use those internally. The rule is about *widgets*, not geometry.

Note that `Split`/`VSplit`/`Row`/`Column` take `RenderNode` children: they are
for declarative plugin UI trees, not for host panels that hold live state.

See `docs/DESIGN_SYSTEM.md` for tokens and specs, and run the component gallery
to see everything rendered:

```
cargo run -p thoth-plugin-sdk --example gallery --features egui
```

## Nothing expensive on the UI thread

File work is measured in seconds, and a frame is measured in milliseconds.
Anything that scans, parses, counts or copies belongs on a worker
(`file::indexing::IndexJob` is the pattern): report progress through an atomic,
make cancellation cooperative, and show the user something usable meanwhile
rather than a spinner.

Watch for the non-obvious ones — `count(*)` over a JSON file is a full scan, and
asking DuckDB whether it *can* read a document means parsing it.

## Claims about performance need a measurement

This codebase makes size and speed promises. Before asserting one, measure it,
and prefer a test that fails when it regresses over a comment that says it
shouldn't. Debug and release numbers differ enough to mislead — say which.

## Caches must not be able to fail a read

A cache is an optimisation. A miss, a corrupt entry or an unreadable directory
returns "not cached", never an error. Validate against a file's size, mtime
*and* a content fingerprint: the first two both miss an in-place edit of the
same length, and a stale index does not fail loudly — it answers with the wrong
bytes.

## Tests must not touch the user's data

Anything writing under `dirs::config_dir()` needs an override for tests
(`THOTH_INDEX_CACHE_DIR` is the precedent). Tests that share process-global
state — the Papyrus registry, the index cache — take the module's lock rather
than racing.
