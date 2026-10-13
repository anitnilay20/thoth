//! egui-owned parts of the plugin system: the renderer for plugin UI trees and
//! the marketplace browser. The runtime, loaders and registry live in
//! `thoth-core` and are re-exported here.

pub use thoth_core::plugin::*;

pub mod marketplace;
pub mod render_node;
