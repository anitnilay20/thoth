//! Platform helpers. The display-independent half lives in `thoth-core` and is
//! re-exported here; font rasterisation and the native menu need egui and are
//! owned by this crate.

pub use thoth_core::platform::*;

pub mod fonts;
pub mod native_menu;

pub use fonts::{find_font_bytes, find_font_bytes_weighted, has_weight, list_system_font_families};
