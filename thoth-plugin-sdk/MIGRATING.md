# Migrating between SDK versions

Changes that a plugin built against an older `thoth-plugin-sdk` will notice —
either because it no longer compiles, or, worse, because it still compiles and
renders differently.

The SDK is a single source of truth for both sides of the boundary: the plugin
builds a node tree and the host renders those same types. So a change here can
reach a plugin two ways, and both are listed.

- **Source** — the plugin crate has to be edited and rebuilt.
- **Payload** — an already-built `.wasm` keeps working, but what the host draws
  from its JSON changes. These are the ones worth reading twice: nothing fails,
  the result just looks different.

## Unreleased — squircle redesign (#156)

### `TableView::sort` / `DataView::sort` hold several keys — *source*

`TableView::sort` and `DataView::sort` were `Option<SortBy>` — at most one
column could be marked with a sort arrow. They are now `Vec<SortBy>`, so a
multi-column sort shows an arrow on every key, and the builder method is
`sort(Vec<SortBy>)` rather than `maybe_sort(Option<SortBy>)`.

A plugin that marked one column now pushes it into a vec:

```rust
// before
DataView::builder().handle("h").maybe_sort(Some(sort_by)).build();
// after
DataView::builder().handle("h").sort(vec![sort_by]).build();
```

`SortBy` also gained an `append` flag (see below); it defaults to `false`.

### `SortBy` gained `append` — *payload*

`SortBy` has a new `append: bool` field, written only when it is `true`. A
header Shift-click now emits `{"column":"a","append":true}` instead of a plain
cycle, and the producer is expected to add that column to its order rather than
replace the order with it.

An old payload (or an unchanged `.wasm`) is unaffected — `append` defaults to
`false` — and the old single `"sort": null` / `"sort": {...}` spellings are
still deserialized into the new list, so nothing errors and nothing renders
differently beyond now having room for several arrows.

### `Separator` no longer derives `Copy` — *source*

`Separator` gained a `color: Option<String>` field, and `String` is not `Copy`,
so the derive had to go.

A plugin that passed a `Separator` by value more than once now moves it on the
first use:

```rust
let rule = Separator::builder().build();
row.push(RenderNode::Separator(rule));
row.push(RenderNode::Separator(rule)); // ← no longer compiles
```

Clone it instead:

```rust
row.push(RenderNode::Separator(rule.clone()));
row.push(RenderNode::Separator(rule));
```

`Clone` is still derived, and a `Separator` is small — two `f32`s and three
`Option`s, one of them a short colour string — so cloning one is not something
to design around.

### `TableView` gained `framed`, defaulting to `true` — *payload*

`framed` is a new field that draws the grid's own canvas fill, hairline edge
and rounded corners. The previous `TableView` had no such field and drew none
of them.

It defaults to `true`, and the field is `#[serde(default)]`, so an existing
payload with no `framed` key deserializes as `true` and gains a frame it never
had. Nothing errors — this shows up only as a plugin that suddenly has a box
around its table, or a doubled border where the plugin was already drawing its
own container.

The default suits the common case: a standalone grid owns its container. Set it
explicitly when the grid sits inside something that already draws the fill,
edge and corners:

```rust
TableView::builder()
    .headers(headers)
    .rows(rows)
    .framed(false) // the surrounding card already owns the edge
    .build();
```

That is what the host's own `DataView` does, and why its grid runs flush to the
panel.
