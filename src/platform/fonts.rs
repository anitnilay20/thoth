/// Cross-platform system font discovery.
///
/// Uses `fontdb` to query fonts via each OS's native font infrastructure:
/// - macOS   — CoreText / system font directories
/// - Linux   — fontconfig (if available) + XDG font directories
/// - Windows — Windows font APIs + %WINDIR%\Fonts
use std::sync::OnceLock;

static FONT_DB: OnceLock<fontdb::Database> = OnceLock::new();

fn font_db() -> &'static fontdb::Database {
    FONT_DB.get_or_init(|| {
        let mut db = fontdb::Database::new();
        db.load_system_fonts();
        db
    })
}

/// Return a sorted, deduplicated list of all installed font family names.
pub fn list_system_font_families() -> Vec<String> {
    let db = font_db();
    let mut families: Vec<String> = db
        .faces()
        .flat_map(|face| face.families.iter().map(|(name, _)| name.clone()))
        .collect::<std::collections::HashSet<_>>()
        .into_iter()
        .collect();
    families.sort();
    families
}

/// Return the raw bytes for the first face whose family name matches `family`.
/// Returns `None` if the font is not installed or its file cannot be read.
pub fn find_font_bytes(family: &str) -> Option<Vec<u8>> {
    let db = font_db();
    let face = db.faces().find(|f| {
        f.families
            .iter()
            .any(|(name, _)| name.eq_ignore_ascii_case(family))
    })?;

    match &face.source {
        fontdb::Source::File(path) => std::fs::read(path).ok(),
        fontdb::Source::Binary(data) => Some(data.as_ref().as_ref().to_vec()),
        fontdb::Source::SharedFile(_, data) => Some(data.as_ref().as_ref().to_vec()),
    }
}
