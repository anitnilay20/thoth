/// Cross-platform file I/O trait for position-independent reads
///
/// This trait abstracts platform-specific file operations to provide
/// a unified interface across Unix and Windows systems.
use std::fs::File;
use std::io::Result;

pub trait FileIO {
    /// Read bytes from a specific position without modifying the file cursor.
    ///
    /// This operation is thread-safe and can be called concurrently from multiple threads.
    ///
    /// # Arguments
    /// * `buf` - Buffer to read data into
    /// * `offset` - Byte offset in the file to start reading from
    ///
    /// # Returns
    /// Number of bytes read, or an I/O error
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize>;
}

impl FileIO for File {
    #[cfg(unix)]
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize> {
        use std::os::unix::fs::FileExt;
        FileExt::read_at(self, buf, offset)
    }

    #[cfg(windows)]
    fn read_at(&self, buf: &mut [u8], offset: u64) -> Result<usize> {
        use std::os::windows::fs::FileExt;
        FileExt::seek_read(self, buf, offset)
    }
}
