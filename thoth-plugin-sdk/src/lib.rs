//! # Thoth Plugin SDK
//!
//! Helper crate for authoring [Thoth](https://github.com/anitnilay20/thoth)
//! plugins. It provides type-safe builders for the UI DSL so plugin authors
//! compose components with code instead of hand-written JSON, and the host
//! renders those same types with egui.
//!
//! ## Cargo features
//!
//! The crate is split so that each consumer compiles only what it needs:
//!
//! - **default** — the data/DSL types and their [builders](bon). This is all a
//!   wasm plugin needs to *describe* its UI, keeping `.wasm` artifacts small.
//! - **`plugin`** — adds the `ToNodeJson` wire-protocol trait for serializing
//!   a node tree into the JSON the host consumes. Enable this in plugins.
//! - **`egui`** — adds the [`theme`] module and the egui rendering
//!   (`egui::Widget`) implementations. Enabled by the **host**, not plugins.
//!
//! Generate the full API docs with all features enabled:
//! `cargo doc -p thoth-plugin-sdk --all-features --open`.
//!
//! ## Who owns what
//!
//! Components never pick their own colours: the application owns the theme and
//! publishes it into egui memory, and widgets read it back via
//! [`theme::ThemeColors::from_ctx`]. See the [`theme`] module for details.

#![deny(missing_docs)]

pub mod components;
pub(crate) mod helpers;
pub mod settings;
pub mod state;
pub mod tokens;

pub use tokens::TextToken;

#[cfg(feature = "plugin")]
mod wire;
#[cfg(feature = "plugin")]
pub use wire::ToNodeJson;

/// Derive the `plugin-meta` `get_info()` export from a `#[plugin(...)]`
/// attribute. See [`thoth_plugin_sdk_macros::PluginMeta`].
#[cfg(feature = "plugin")]
pub use thoth_plugin_sdk_macros::PluginMeta;

#[cfg(feature = "egui")]
pub mod theme;

pub mod render_node;