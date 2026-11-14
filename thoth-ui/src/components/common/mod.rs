//! Host-side shared utilities for feature components.
//!
//! The reusable UI widgets that used to live here now come from
//! `thoth_plugin_sdk::components`. What remains is host-only glue: the
//! component-trait system (`traits`) and the icon-texture loader (`helpers`).
pub mod helpers;
pub mod traits;
