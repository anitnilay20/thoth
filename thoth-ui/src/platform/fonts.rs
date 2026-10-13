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
    find_font_bytes_weighted(family, fontdb::Weight::NORMAL)
}

/// Return the raw bytes for the face of `family` closest to `weight`.
///
/// egui has no weight axis: a `FontId` names a *family*, so rendering text at
/// weight 500 or 600 means registering that weight as its own named family. This
/// picks the nearest available face by absolute weight distance, so asking for
/// Medium on a family that ships only Regular and Bold yields Regular rather
/// than nothing.
pub fn find_font_bytes_weighted(family: &str, weight: fontdb::Weight) -> Option<Vec<u8>> {
    let db = font_db();
    let face = db
        .faces()
        .filter(|f| {
            f.families
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case(family))
        })
        // Prefer upright faces so a Medium *Italic* never wins on weight alone.
        .min_by_key(|f| {
            let dw = f.weight.0.abs_diff(weight.0) as u32;
            let italic = u32::from(f.style != fontdb::Style::Normal);
            (italic, dw)
        })?;

    match &face.source {
        fontdb::Source::File(path) => std::fs::read(path).ok(),
        fontdb::Source::Binary(data) => Some(data.as_ref().as_ref().to_vec()),
        fontdb::Source::SharedFile(_, data) => Some(data.as_ref().as_ref().to_vec()),
    }
}

/// Whether `family` actually ships a face at (or very near) `weight`.
///
/// [`find_font_bytes_weighted`] always falls back to the nearest face, so callers
/// that only want to register a real Medium/SemiBold family — rather than a
/// duplicate of Regular — check this first.
pub fn has_weight(family: &str, weight: fontdb::Weight) -> bool {
    const TOLERANCE: u16 = 25;
    font_db()
        .faces()
        .filter(|f| {
            f.families
                .iter()
                .any(|(name, _)| name.eq_ignore_ascii_case(family))
        })
        .any(|f| f.style == fontdb::Style::Normal && f.weight.0.abs_diff(weight.0) <= TOLERANCE)
}
