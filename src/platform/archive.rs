/// Cross-platform archive extraction
///
/// Provides platform-specific archive extraction for updates
use crate::error::{Result, ThothError};
use std::path::Path;

pub trait ArchiveExtractor {
    /// Extract an archive to a destination directory
    fn extract(&self, archive_path: &Path, dest_dir: &Path) -> Result<()>;
}

pub struct ZipExtractor;
pub struct TarGzExtractor;

impl ArchiveExtractor for ZipExtractor {
    #[cfg(target_os = "windows")]
    fn extract(&self, archive_path: &Path, dest_dir: &Path) -> Result<()> {
        let file = std::fs::File::open(archive_path)?;
        let mut archive =
            zip::ZipArchive::new(file).map_err(|e| ThothError::UpdateInstallError {
                reason: format!("Failed to open ZIP archive: {}", e),
            })?;

        for i in 0..archive.len() {
            let mut file = archive
                .by_index(i)
                .map_err(|e| ThothError::UpdateInstallError {
                    reason: format!("Failed to read ZIP entry: {}", e),
                })?;
            let outpath = dest_dir.join(file.name());

            if file.is_dir() {
                std::fs::create_dir_all(&outpath)?;
            } else {
                if let Some(p) = outpath.parent() {
                    std::fs::create_dir_all(p)?;
                }
                let mut outfile =
                    std::fs::File::create(&outpath).map_err(|e| ThothError::FileWriteError {
                        path: outpath.clone(),
                        reason: format!("Failed to create extracted file: {}", e),
                    })?;
                std::io::copy(&mut file, &mut outfile).map_err(|e| ThothError::FileWriteError {
                    path: outpath.clone(),
                    reason: format!("Failed to write extracted data: {}", e),
                })?;
            }
        }
        Ok(())
    }

    #[cfg(not(target_os = "windows"))]
    fn extract(&self, _archive_path: &Path, _dest_dir: &Path) -> Result<()> {
        Err(ThothError::UpdateInstallError {
            reason: "ZIP extraction not supported on this platform".to_string(),
        })
    }
}

impl ArchiveExtractor for TarGzExtractor {
    #[cfg(not(target_os = "windows"))]
    fn extract(&self, archive_path: &Path, dest_dir: &Path) -> Result<()> {
        use flate2::read::GzDecoder;
        use tar::Archive;

        let file = std::fs::File::open(archive_path)?;
        let gz = GzDecoder::new(file);
        let mut archive = Archive::new(gz);
        archive.unpack(dest_dir)?;

        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn extract(&self, _archive_path: &Path, _dest_dir: &Path) -> Result<()> {
        Err(ThothError::UpdateInstallError {
            reason: "tar.gz extraction not supported on Windows platform".to_string(),
        })
    }
}

/// Get the appropriate archive extractor for the current platform
pub fn get_extractor_for_file(filename: &str) -> Result<Box<dyn ArchiveExtractor>> {
    if filename.ends_with(".zip") {
        Ok(Box::new(ZipExtractor))
    } else if filename.ends_with(".tar.gz") {
        Ok(Box::new(TarGzExtractor))
    } else {
        Err(ThothError::UpdateInstallError {
            reason: format!("Unsupported archive format: {}", filename),
        })
    }
}
