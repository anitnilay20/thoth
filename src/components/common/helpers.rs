use std::path::Path;

use eframe::egui::{self, TextureHandle};

/// Decode a PNG from `path` and upload it to the GPU as an egui texture.
/// The `TextureHandle` is stored in egui's per-frame memory keyed by path,
/// so the file is read and decoded only once per session.
pub fn load_icon_texture(ctx: &egui::Context, path: &Path, name: &str) -> Option<TextureHandle> {
    let key = egui::Id::new((name, path));

    if let Some(cached) = ctx.memory(|mem| mem.data.get_temp::<TextureHandle>(key)) {
        return Some(cached);
    }

    let texture = decode_png_to_texture(ctx, path)?;
    ctx.memory_mut(|mem| mem.data.insert_temp(key, texture.clone()));
    Some(texture)
}

/// Maximum icon file size we're willing to read. Protects against accidentally
/// pointing at a large asset and allocating huge amounts of memory.
const MAX_ICON_SIZE_BYTES: u64 = 5_000_000;

fn decode_png_to_texture(ctx: &egui::Context, path: &Path) -> Option<TextureHandle> {
    match std::fs::metadata(path) {
        Ok(meta) if meta.len() > MAX_ICON_SIZE_BYTES => {
            eprintln!(
                "warn: icon at {} is too large ({} bytes > {MAX_ICON_SIZE_BYTES}), skipping",
                path.display(),
                meta.len()
            );
            return None;
        }
        Err(_) => return None,
        _ => {}
    }

    let bytes = match std::fs::read(path) {
        Err(e) => {
            eprintln!("warn: failed to read icon at {}: {e}", path.display());
            return None;
        }
        Ok(b) => b,
    };

    let img = match image::load_from_memory(&bytes) {
        Err(e) => {
            eprintln!("warn: failed to decode icon at {}: {e}", path.display());
            return None;
        }
        Ok(img) => img,
    };

    let rgba = img.to_rgba8();
    let (w, h) = rgba.dimensions();
    let color_image = egui::ColorImage::from_rgba_unmultiplied([w as usize, h as usize], &rgba);

    Some(ctx.load_texture(
        path.to_string_lossy(),
        color_image,
        egui::TextureOptions::LINEAR,
    ))
}
