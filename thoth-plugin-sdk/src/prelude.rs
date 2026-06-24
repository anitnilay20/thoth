//! Everything a plugin author typically needs, in one glob import:
//!
//! ```
//! use thoth_plugin_sdk::prelude::*;
//! ```
//!
//! Brings in [`RenderNode`](crate::render_node::RenderNode), every UI component
//! and its enums (via [`components`](crate::components)), the state and settings
//! helpers ([`PluginState`](crate::state::PluginState),
//! [`SettingsMap`](crate::settings::SettingsMap)), and — with the `plugin`
//! feature — the [`PluginMeta`](crate::PluginMeta) derive and the
//! [`ToNodeJson`](crate::ToNodeJson) trait.
//!
//! Host-only items (the `egui` renderer, the `theme` palette) are intentionally
//! left out; import those directly when building the host.

pub use crate::components::*;
pub use crate::render_node::RenderNode;
pub use crate::settings::SettingsMap;
pub use crate::state::PluginState;
pub use crate::tokens::TextToken;

#[cfg(feature = "plugin")]
pub use crate::{PluginMeta, ToNodeJson};
