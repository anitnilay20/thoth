mod format;
mod json_copy_to_clipboard;
mod lru_cache;
mod scroll;

use crate::shortcuts::Shortcut;
use eframe::egui::IconData;
pub use format::{format_date, format_date_static, format_simple_kv, preview_value};
pub use json_copy_to_clipboard::{get_object_string, split_root_rel};
pub use lru_cache::LruCache;
pub use scroll::{scroll_to_search_target, scroll_to_selection};

/// Get formatted shortcut strings for context menu
/// Returns: (copy_key, copy_value, copy_object, copy_path)
pub fn get_context_menu_shortcuts() -> (String, String, String, String) {
    let copy_key = Shortcut::new("C").command().format();
    let copy_value = Shortcut::new("C").command().shift().format();
    let copy_object = Shortcut::new("C").command().alt().format();
    let copy_path = Shortcut::new("P").command().shift().format();
    (copy_key, copy_value, copy_object, copy_path)
}

pub fn load_icon(bytes: &[u8]) -> Option<IconData> {
    let image = match image::load_from_memory(bytes) {
        Ok(img) => img.into_rgba8(),
        Err(e) => {
            eprintln!("Failed to load icon: {}", e);
            return None;
        }
    };
    let (w, h) = image.dimensions();
    Some(IconData {
        rgba: image.into_raw(),
        width: w,
        height: h,
    })
}
