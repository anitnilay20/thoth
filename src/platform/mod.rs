/// Platform abstraction layer for cross-platform file operations
///
/// This module provides platform-agnostic interfaces for file I/O operations
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
pub mod file_io;

pub use file_io::FileIO;
