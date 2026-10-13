mod format;
mod json_copy_to_clipboard;
mod lru_cache;

pub use format::{format_date, format_date_static, format_simple_kv, preview_value};
pub use json_copy_to_clipboard::{get_object_string, split_root_rel, walk_rel};
pub use lru_cache::LruCache;

pub const fn default_rate_limit() -> u32 {
    60
}
