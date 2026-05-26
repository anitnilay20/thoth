/// Platform abstraction layer for cross-platform operations
///
/// This module provides platform-agnostic interfaces for operations
/// that have different implementations on Unix and Windows systems.
///
/// # Example
/// ```no_run
/// use std::fs::File;
/// use thoth::platform::FileIO;
///
/// let file = File::open("data.json")?;
/// let mut buffer = vec![0u8; 1024];
/// file.read_at(&mut buffer, 0)?; // Works on both Unix and Windows
/// # Ok::<(), std::io::Error>(())
/// ```
pub mod archive;
pub mod file_io;
pub mod file_open_channel;
pub mod fonts;
pub mod fs;
#[cfg(target_os = "macos")]
pub mod macos;
pub mod native_menu;
pub mod path_registry;

pub use archive::get_extractor_for_file;
pub use file_io::FileIO;
pub use file_open_channel::{drain_open_requests, enqueue_open_request};
pub use fonts::{find_font_bytes, list_system_font_families};
pub use fs::get_fs_ops;
