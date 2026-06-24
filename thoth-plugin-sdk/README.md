# thoth-plugin-sdk

Helper crate for authoring [Thoth](https://github.com/anitnilay20/thoth) plugins.

Plugins describe their UI as a tree of nodes serialized to JSON. Instead of
hand-writing that JSON, build it with type-safe builders — the host renders the
*same* types, so the SDK is a single source of truth for both sides.

```toml
[dependencies]
thoth-plugin-sdk = { version = "0.1", features = ["plugin"] }
```

```rust
use thoth_plugin_sdk::prelude::*;

#[derive(PluginMeta)]
#[plugin(
    id          = "com.example.my-plugin",
    name        = "My Plugin",
    version     = env!("CARGO_PKG_VERSION"),
    description = "Does useful things",
    capabilities = [DataSource, NewUiComponent],
)]
struct MyPlugin;

#[derive(Default)]
struct State { url: String }
static STATE: PluginState<State> = PluginState::new();

fn build_ui(state: &State) -> RenderNode {
    RenderNode::Column(
        Column::builder()
            .gap(8.0)
            .children(vec![
                RenderNode::text("Endpoint"),
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
```

## What's inside

- **`components`** — every UI widget (layout, display, input, action) as a
  `bon` builder. Fields are private/`#[non_exhaustive]`; construct via builders.
- **`render_node`** — the serializable `RenderNode` DSL the host renders.
- **`state`** — `PluginState<T>`, a lazily-initialised global replacing the
  `thread_local! { RefCell<Option<T>> }` boilerplate.
- **`settings`** — `SettingsMap`, parse/build the `{key,value}` settings payload.
- **`PluginMeta`** derive — generates the `plugin-meta` `get_info()` export.
- **`prelude`** — one glob (`use thoth_plugin_sdk::prelude::*;`) for all of the above.

## Cargo features

- **`default`** — DSL types + builders (everything a wasm plugin needs).
- **`plugin`** — adds the `ToNodeJson` wire trait and the `PluginMeta` derive.
- **`egui`** — the host-only renderer; **do not** enable it in plugins.

See [`docs/PLUGIN_SYSTEM.md`](https://github.com/anitnilay20/thoth/blob/main/docs/PLUGIN_SYSTEM.md)
for the full guide.

## License

MIT OR Apache-2.0
