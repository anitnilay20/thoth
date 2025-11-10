mod format;
mod json_copy_to_clipboard;
mod lru_cache;

use eframe::egui::IconData;
pub use format::{format_date, format_date_static, format_simple_kv, preview_value};
pub use json_copy_to_clipboard::{get_object_string, split_root_rel};
pub use lru_cache::LruCache;

pub fn load_icon(bytes: &[u8]) -> IconData {
    let image = image::load_from_memory(bytes).unwrap().into_rgba8();
    let (w, h) = image.dimensions();
    IconData {
        rgba: image.into_raw(),
        width: w,
        height: h,
    }
}
