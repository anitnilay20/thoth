use super::json_tree_viewer::JsonTreeViewer;
use super::viewer_trait::FileFormatViewer;
use crate::file::lazy_loader::FileType;

/// Enum representing different file format viewers
///
/// This enum wraps concrete viewer implementations and allows FileViewer
/// to work with different viewer types polymorphically without boxing/dynamic dispatch.
///
/// **IMPORTANT**: All variants MUST contain types that implement `FileFormatViewer` trait.
/// This is enforced at compile-time by the `as_viewer_mut()` method - if a type doesn't
/// implement the trait, the code will not compile.
///
/// # Adding a New Viewer
///
/// 1. Create a new viewer struct (e.g., `CsvTableViewer`)
/// 2. Implement `FileFormatViewer` trait for it (REQUIRED - enforced at compile time)
/// 3. Add a new variant to this enum: `Csv(CsvTableViewer)`
/// 4. Update `from_file_type()` to handle the new file type
/// 5. Update the `as_viewer_mut()` match to include the new variant
///
/// # Example
/// ```
/// // Step 1 & 2: Create and implement trait (REQUIRED)
/// pub struct CsvTableViewer { ... }
/// impl FileFormatViewer for CsvTableViewer { ... }  // Must implement!
///
/// // Step 3: Add variant
/// pub enum ViewerType {
///     Json(JsonTreeViewer),
///     Csv(CsvTableViewer),  // Compiler will verify FileFormatViewer is implemented
/// }
///
/// // Step 4: Update from_file_type
/// FileType::Csv => ViewerType::Csv(CsvTableViewer::new()),
///
/// // Step 5: Update as_viewer_mut - this enforces the trait bound!
/// ViewerType::Csv(viewer) => viewer,  // Won't compile if trait not implemented
/// ```
pub enum ViewerType {
    /// JSON/NDJSON tree viewer (implements FileFormatViewer)
    Json(JsonTreeViewer),
    // Future viewers:
    // Csv(CsvTableViewer),
    // Xml(XmlTreeViewer),
    // Yaml(YamlTreeViewer),
    // Text(TextViewer),
}

impl ViewerType {
    /// Create a viewer based on file type
    pub fn from_file_type(file_type: FileType) -> Self {
        match file_type {
            FileType::Json | FileType::Ndjson => ViewerType::Json(JsonTreeViewer::new()),
            // Future file types:
            // FileType::Csv => ViewerType::Csv(CsvTableViewer::new()),
            // FileType::Xml => ViewerType::Xml(XmlTreeViewer::new()),
            // FileType::Yaml => ViewerType::Yaml(YamlTreeViewer::new()),
            // _ => ViewerType::Text(TextViewer::new()),  // Fallback
        }
    }

    /// Get mutable reference to the underlying viewer as a trait object
    ///
    /// **Trait Enforcement**: This method enforces that all ViewerType variants
    /// must implement `FileFormatViewer`. If you add a new variant with a type
    /// that doesn't implement the trait, Rust will give a compile error here:
    ///
    /// ```text
    /// error[E0277]: the trait bound `YourType: FileFormatViewer` is not satisfied
    /// ```
    ///
    /// This compile-time check ensures type safety without runtime overhead.
    pub fn as_viewer_mut(&mut self) -> &mut dyn FileFormatViewer {
        match self {
            ViewerType::Json(viewer) => viewer,
            // ViewerType::Csv(viewer) => viewer,
            // ViewerType::Xml(viewer) => viewer,
        }
    }
}
